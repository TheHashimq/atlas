// ================================================================
//  Fullscreen Triangle with Proper UV Generation
// ================================================================

struct VSOut {
    @builtin(position) pos : vec4<f32>,
    @location(0) uv : vec2<f32>,
};

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> VSOut {
    // Single fullscreen triangle
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );

    let p = positions[vi];

    var out : VSOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.uv  = vec2<f32>(p.x * 0.5 + 0.5, p.y * -0.5 + 0.5);

    return out;
}

// ================================================================
//  Bindings
// ================================================================

@group(0) @binding(0) var t_src : texture_2d<f32>;
@group(0) @binding(1) var s_src : sampler;

// ================================================================
//  ACES Filmic Tone Mapping
// ================================================================

fn aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3(0.0), vec3(1.0));
}

fn srgb_gamma(c: vec3<f32>) -> vec3<f32> {
    return pow(c, vec3(1.0 / 2.2));
}

// ================================================================
//  fs_blit — HDR → LDR Tonemap + Gamma
//  Used for:
//    • Low quality tier
//    • Base scene copy before bloom composite
// ================================================================

@fragment
fn fs_blit(in: VSOut) -> @location(0) vec4<f32> {
    let uv      = in.uv;
    let hdr     = textureSample(t_src, s_src, uv).rgb;
    let exposed = hdr * 1.6;
    let mapped  = aces(exposed);
    let gamma   = srgb_gamma(mapped);
    return vec4<f32>(gamma, 1.0);
}

// ================================================================
//  fs_threshold — Extract bright pixels (HDR space)
// ================================================================

@fragment
fn fs_threshold(in: VSOut) -> @location(0) vec4<f32> {
    let uv  = in.uv;
    let col = textureSample(t_src, s_src, uv).rgb;

    let lum       = dot(col, vec3<f32>(0.2126, 0.7152, 0.0722));
    let threshold = 1.1;
    let knee      = 0.4;

    let rq     = clamp(lum - threshold + knee, 0.0, 2.0 * knee);
    let weight = (rq * rq) / (4.0 * knee + 0.00001);
    let factor = max(weight, lum - threshold) / max(lum, 0.00001);

    return vec4<f32>(col * factor, 1.0);
}

// ================================================================
//  Horizontal Gaussian Blur (9-tap)
// ================================================================

@fragment
fn fs_blur_h(in: VSOut) -> @location(0) vec4<f32> {
    let uv   = in.uv;
    let dims = vec2<f32>(textureDimensions(t_src));
    let texel = vec2<f32>(1.0 / dims.x, 0.0);

    let w0 = 0.2270270270;
    let w1 = 0.1945945946;
    let w2 = 0.1216216216;
    let w3 = 0.0540540541;
    let w4 = 0.0162162162;

    var col = textureSample(t_src, s_src, uv).rgb * w0;
    col += textureSample(t_src, s_src, uv + texel * 1.0).rgb * w1;
    col += textureSample(t_src, s_src, uv - texel * 1.0).rgb * w1;
    col += textureSample(t_src, s_src, uv + texel * 2.0).rgb * w2;
    col += textureSample(t_src, s_src, uv - texel * 2.0).rgb * w2;
    col += textureSample(t_src, s_src, uv + texel * 3.0).rgb * w3;
    col += textureSample(t_src, s_src, uv - texel * 3.0).rgb * w3;
    col += textureSample(t_src, s_src, uv + texel * 4.0).rgb * w4;
    col += textureSample(t_src, s_src, uv - texel * 4.0).rgb * w4;

    return vec4<f32>(col, 1.0);
}

// ================================================================
//  Vertical Gaussian Blur (9-tap)
// ================================================================

@fragment
fn fs_blur_v(in: VSOut) -> @location(0) vec4<f32> {
    let uv   = in.uv;
    let dims = vec2<f32>(textureDimensions(t_src));
    let texel = vec2<f32>(0.0, 1.0 / dims.y);

    let w0 = 0.2270270270;
    let w1 = 0.1945945946;
    let w2 = 0.1216216216;
    let w3 = 0.0540540541;
    let w4 = 0.0162162162;

    var col = textureSample(t_src, s_src, uv).rgb * w0;
    col += textureSample(t_src, s_src, uv + texel * 1.0).rgb * w1;
    col += textureSample(t_src, s_src, uv - texel * 1.0).rgb * w1;
    col += textureSample(t_src, s_src, uv + texel * 2.0).rgb * w2;
    col += textureSample(t_src, s_src, uv - texel * 2.0).rgb * w2;
    col += textureSample(t_src, s_src, uv + texel * 3.0).rgb * w3;
    col += textureSample(t_src, s_src, uv - texel * 3.0).rgb * w3;
    col += textureSample(t_src, s_src, uv + texel * 4.0).rgb * w4;
    col += textureSample(t_src, s_src, uv - texel * 4.0).rgb * w4;

    return vec4<f32>(col, 1.0);
}

// ================================================================
//  fs_composite — Tonemapped Bloom Additive
// ================================================================

@fragment
fn fs_composite(in: VSOut) -> @location(0) vec4<f32> {
    let uv    = in.uv;
    let bloom = textureSample(t_src, s_src, uv).rgb;

    // Soft filmic bloom
    let mapped = aces(bloom * 0.20);
    let gamma  = srgb_gamma(mapped);

    return vec4<f32>(gamma, 1.0);
}
