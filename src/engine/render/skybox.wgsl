struct SkyUniforms {
    view_proj  : mat4x4<f32>,
    camera_pos : vec4<f32>,
    time       : vec4<f32>,
    sun_dir    : vec4<f32>,   // xyz = normalized direction *toward* sun
};

@group(0) @binding(0) var<uniform> sky : SkyUniforms;

struct VSOut {
    @builtin(position) clip_pos : vec4<f32>,
    @location(0)       ray_dir  : vec3<f32>,
};

// Fullscreen triangle — reconstruct world-space ray direction
@vertex
fn vs_sky(@builtin(vertex_index) vi: u32) -> VSOut {
    let x = f32((vi << 1u) & 2u) * 2.0 - 1.0;
    let y = f32( vi         & 2u) * 2.0 - 1.0;

    var out : VSOut;
    out.clip_pos = vec4<f32>(x, y, 1.0, 1.0);
    out.ray_dir  = vec3<f32>(x, y, -1.0);
    return out;
}

// ================================================================
//  Utilities
// ================================================================

fn hash3(p: vec3<f32>) -> f32 {
    var q = fract(p * 0.1031);
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn hash21(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(127.1, 311.7));
    q += dot(q, q + 19.19);
    return fract(q.x * q.y);
}

// ================================================================
//  Procedural Starfield
// ================================================================

fn stars(dir: vec3<f32>, offset: vec3<f32>, grid_size: f32, threshold: f32) -> f32 {
    let cell      = floor((dir + offset) * grid_size);
    let h         = hash3(cell);
    let is_star   = step(threshold, h);
    let brightness = fract(h * 123.456) * 0.6 + 0.15;
    return is_star * brightness;
}

// ================================================================
//  Sun disc + halo rings
// ================================================================

fn sun_disc(ray: vec3<f32>, sun_dir: vec3<f32>, t: f32) -> vec3<f32> {
    let cos_angle  = dot(ray, sun_dir);
    let angle      = acos(clamp(cos_angle, -1.0, 1.0));

    // ---- Disc core ----
    let disc_radius = 0.038;       // angular radius of sun
    let edge_soft   = 0.004;
    let disc_mask   = 1.0 - smoothstep(disc_radius - edge_soft, disc_radius + edge_soft, angle);

    // ---- Hot core gradient ----
    let core_uv   = angle / disc_radius;
    let core_t    = clamp(1.0 - core_uv, 0.0, 1.0);
    let disc_core_col = mix(
        vec3<f32>(1.0, 0.55, 0.1),   // orange edge
        vec3<f32>(1.2, 1.1, 0.95),   // white-hot centre (HDR > 1)
        core_t * core_t,
    );

    // ---- Corona halo — 3 concentric rings ----
    let halo1 = exp(-pow(angle / 0.08, 2.0)) * 0.25;
    let halo2 = exp(-pow(angle / 0.20, 2.0)) * 0.10;
    let halo3 = exp(-pow(angle / 0.50, 2.0)) * 0.04;
    let halo  = halo1 + halo2 + halo3;
    let halo_col = vec3<f32>(1.0, 0.65, 0.2) * halo;

    // ---- Animated lens flare chromatic rings ----
    //  project ray onto a 2D plane perpendicular to sun_dir
    let up      = normalize(vec3<f32>(0.0, 1.0, 0.001));   // avoid degenerate
    let right   = normalize(cross(sun_dir, up));
    let screen_y= normalize(cross(right, sun_dir));
    let px      = dot(ray - sun_dir * cos_angle, right);
    let py      = dot(ray - sun_dir * cos_angle, screen_y);
    let ring_r  = sqrt(px * px + py * py);

    // rotating shimmer
    let theta   = atan2(py, px) + t * 0.05;
    let shimmer = 0.5 + 0.5 * sin(theta * 6.0 + t * 0.3);

    let flare_ring = exp(-pow((ring_r - 0.13) / 0.012, 2.0)) * shimmer * 0.18
                   + exp(-pow((ring_r - 0.22) / 0.010, 2.0)) * shimmer * 0.10
                   + exp(-pow((ring_r - 0.35) / 0.018, 2.0)) * shimmer * 0.06;
    let flare_col = vec3<f32>(0.8, 0.5, 1.0) * flare_ring;  // slight violet

    // ---- Combine ----
    let disc_out = disc_core_col * disc_mask * 6.0;
    return disc_out + halo_col + flare_col;
}

// ================================================================
//  Sky gradient
// ================================================================

fn sky_gradient(ray_dir: vec3<f32>) -> vec3<f32> {
    // Subtle deep-space gradient — brighter near the sun horizon
    let horizon = exp(-max(ray_dir.y, 0.0) * 3.5);
    let zenith  = vec3<f32>(0.004, 0.006, 0.016);
    let horiz   = vec3<f32>(0.012, 0.008, 0.020);
    return mix(zenith, horiz, horizon);
}

// ================================================================
//  Fragment
// ================================================================

@fragment
fn fs_sky(in: VSOut) -> @location(0) vec4<f32> {
    // Proper ray reconstruction via inverse view-projection
    let inv_vp  = transpose(sky.view_proj);
    let clip    = vec4<f32>(in.ray_dir.xy, 1.0, 1.0);
    // Simple: use the clip.xy as view direction (works for perspective sky)
    let ray = normalize(in.ray_dir);

    let t = sky.time.x;

    // ---- Background ----
    let bg_col = sky_gradient(ray);

    // ---- Stars (Multi-Layer Parallax) ----
    let sun_dot   = max(dot(ray, sky.sun_dir.xyz), 0.0);
    let star_mask = 1.0 - smoothstep(0.2, 0.8, sun_dot);
    
    // Layer 1: Far stars (tiny, static)
    let star1 = stars(ray, vec3(0.0), 500.0, 0.992) * 0.6;
    
    // Layer 2: Near stars (larger, parallax)
    // Offset based on view direction to create depth
    let parallax = ray * 0.02; 
    let stars1 = stars(ray, parallax, 25.0, 0.98); // Top Layer
    let stars2 = stars(ray, parallax * 0.4, 45.0, 0.99); // Mid Layer
    let stars3 = stars(ray, parallax * 0.1, 75.0, 0.995); // Deepest Layer (Tiny)
    let star_val = (star1 + stars1 + stars2 + stars3) * star_mask;
    
    // Twinkle (based on far-star cell)
    let cell     = floor(ray * 500.0);
    let twinkle  = 0.7 + 0.3 * sin(t * (hash3(cell) * 3.0 + 1.0) + hash3(cell + 1.0) * 6.28);
    let star_col = vec3<f32>(0.85, 0.95, 1.0) * star_val * twinkle;

    // ---- Sun ----
    let sun_col = sun_disc(ray, sky.sun_dir.xyz, t);

    let final_col = bg_col + star_col + sun_col;
    return vec4<f32>(final_col, 1.0);
}
