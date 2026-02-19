#!/usr/bin/env python3

from pathlib import Path
import re
import sys

# =========================
# CONFIG
# =========================

REPO_ROOT = Path(__file__).resolve().parents[1]
SPEC_FILE = REPO_ROOT / "docs" / "SYSTEM_SPEC.md"

SOURCE_EXTENSIONS = {".rs", ".wgsl"}

# =========================
# REGEX DEFINITIONS
# =========================

FN_RE = re.compile(
    r"^\s*(pub\s+)?fn\s+([a-zA-Z0-9_]+)\s*\(([^)]*)\)\s*(?:->\s*([^{\n]+))?",
    re.MULTILINE,
)

STATIC_RE = re.compile(
    r"^\s*(pub\s+)?static\s+(mut\s+)?([A-Z0-9_]+)\s*:\s*([^=;]+)",
    re.MULTILINE,
)

CONST_RE = re.compile(
    r"^\s*(pub\s+)?const\s+([A-Z0-9_]+)\s*:\s*([^=;]+)",
    re.MULTILINE,
)

LAZY_RE = re.compile(r"lazy_static!|once_cell|thread_local!", re.MULTILINE)

UNSAFE_RE = re.compile(r"\bunsafe\b")

# =========================
# SCANNING
# =========================

def scan_files():
    return [
        p for p in REPO_ROOT.rglob("*")
        if p.is_file()
        and p.suffix in SOURCE_EXTENSIONS
        and ".git" not in p.parts
        and "target" not in p.parts
    ]


# =========================
# FILE TREE
# =========================

def build_tree(files):
    tree = {}
    for f in files:
        rel = f.relative_to(REPO_ROOT)
        node = tree
        for part in rel.parts:
            node = node.setdefault(part, {})
    return tree


def render_tree(node, indent=0):
    lines = []
    for k in sorted(node):
        lines.append("  " * indent + f"- {k}")
        lines.extend(render_tree(node[k], indent + 1))
    return lines


# =========================
# FUNCTION EXTRACTION
# =========================

def extract_functions(file: Path):
    text = file.read_text(encoding="utf-8", errors="ignore")
    functions = []

    for pub, name, params, ret in FN_RE.findall(text):
        functions.append({
            "visibility": "pub" if pub else "private",
            "name": name,
            "params": params.strip(),
            "return": ret.strip() if ret else "()",
        })

    return functions


# =========================
# GLOBAL SYMBOL EXTRACTION
# =========================

def extract_globals(file: Path):
    text = file.read_text(encoding="utf-8", errors="ignore")
    globals_ = []

    for pub, mut, name, typ in STATIC_RE.findall(text):
        globals_.append({
            "kind": "static",
            "visibility": "pub" if pub else "private",
            "mutable": bool(mut),
            "name": name,
            "type": typ.strip(),
        })

    for pub, name, typ in CONST_RE.findall(text):
        globals_.append({
            "kind": "const",
            "visibility": "pub" if pub else "private",
            "name": name,
            "type": typ.strip(),
        })

    if LAZY_RE.search(text):
        globals_.append({
            "kind": "lazy/static-like",
            "visibility": "unknown",
            "name": "macro-based",
            "type": "lazy_static / once_cell / thread_local",
        })

    return globals_


# =========================
# UNSAFE DETECTION
# =========================

def has_unsafe(file: Path):
    text = file.read_text(encoding="utf-8", errors="ignore")
    return bool(UNSAFE_RE.search(text))


# =========================
# SPEC PATCHING
# =========================

def replace_block(text, tag, content):
    start = f"<!-- AUTO:{tag}:START -->"
    end = f"<!-- AUTO:{tag}:END -->"

    if start not in text or end not in text:
        raise RuntimeError(f"Missing AUTO block for {tag}")

    before, _, rest = text.partition(start)
    _, _, after = rest.partition(end)

    return f"{before}{start}\n{content}\n{end}{after}"


# =========================
# MAIN
# =========================

def main():
    if not SPEC_FILE.exists():
        print("SYSTEM_SPEC.md not found", file=sys.stderr)
        sys.exit(1)

    files = scan_files()

    # FILE TREE
    tree_md = "\n".join(render_tree(build_tree(files)))

    # FUNCTIONS
    fn_lines = []
    for f in files:
        if f.suffix == ".rs":
            funcs = extract_functions(f)
            if funcs:
                fn_lines.append(f"### `{f.relative_to(REPO_ROOT)}`")
                for fn in funcs:
                    fn_lines.append(
                        f"- `{fn['visibility']} fn {fn['name']}({fn['params']}) -> {fn['return']}`"
                    )
                fn_lines.append("")

    fn_md = "\n".join(fn_lines) or "_No functions detected._"

    # GLOBALS
    global_lines = []
    for f in files:
        if f.suffix == ".rs":
            globals_ = extract_globals(f)
            if globals_:
                global_lines.append(f"### `{f.relative_to(REPO_ROOT)}`")
                for g in globals_:
                    line = f"- `{g['kind']} {g.get('name')} : {g.get('type')}`"
                    line += f" ({g['visibility']})"
                    if g.get("mutable"):
                        line += " [mutable]"
                    global_lines.append(line)
                global_lines.append("")

    globals_md = "\n".join(global_lines) or "_No global symbols detected._"

    # UNSAFE
    unsafe_md = "\n".join(
        f"- `{f.relative_to(REPO_ROOT)}`"
        for f in files
        if f.suffix == ".rs" and has_unsafe(f)
    ) or "_No unsafe usage detected._"

    # PATCH
    spec = SPEC_FILE.read_text(encoding="utf-8")
    spec = replace_block(spec, "FILE_TREE", tree_md)
    spec = replace_block(spec, "FUNCTIONS", fn_md)
    spec = replace_block(spec, "GLOBALS", globals_md)
    spec = replace_block(spec, "UNSAFE", unsafe_md)

    SPEC_FILE.write_text(spec, encoding="utf-8")
    print("SYSTEM_SPEC.md synchronized successfully")


if __name__ == "__main__":
    main()

