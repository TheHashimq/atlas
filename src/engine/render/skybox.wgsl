struct SkyUniforms {
    view_proj  : mat4x4<f32>,
    camera_pos : vec4<f32>,
    time       : vec4<f32>,
};

@group(0) @binding(0) var<uniform> sky : SkyUniforms;

struct VSOut {
    @builtin(position) clip_pos : vec4<f32>,
    @location(0)       ray_dir  : vec3<f32>,
};

// Fullscreen triangle, ray direction reconstructed from clip pos
@vertex
fn vs_sky(@builtin(vertex_index) vi: u32) -> VSOut {
    let x = f32((vi << 1u) & 2u) * 2.0 - 1.0;
    let y = f32( vi         & 2u) * 2.0 - 1.0;

    // Unproject clip-space corner to world-space ray
    let inv_vp  = transpose(sky.view_proj); // approximation — good enough for sky
    let clip    = vec4<f32>(x, y, 1.0, 1.0);

    var out : VSOut;
    out.clip_pos = vec4<f32>(x, y, 1.0, 1.0);  // z=1 → far plane
    out.ray_dir  = vec3<f32>(x, y, -1.0);       // refined in fragment
    return out;
}

fn hash(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(123.4, 456.7));
    q += dot(q, q + 45.32);
    return fract(q.x * q.y);
}

fn stars(dir: vec3<f32>, t: f32) -> f32 {
    // Project direction onto a grid for twinkling stars
    let uv    = vec2<f32>(atan2(dir.z, dir.x), asin(dir.y)) * 8.0;
    let cell  = floor(uv);
    let f     = fract(uv);
    let h     = hash(cell);
    let blink = 0.5 + 0.5 * sin(t * (2.0 + h * 4.0) + h * 6.28);
    let dist  = length(f - vec2(0.5));
    return smoothstep(0.12, 0.0, dist) * step(0.85, h) * blink;
}

@fragment
fn fs_sky(in: VSOut) -> @location(0) vec4<f32> {
    let t = sky.time.x;

    // Reconstruct view ray from camera
    let ray = normalize(in.ray_dir);

    // Horizon blend
    let h = ray.y;  // -1=down, 1=up

    // Sky gradient: deep navy → midnight purple → near-black at zenith
    let zenith   = vec3<f32>(0.02, 0.02, 0.08);
    let horizon  = vec3<f32>(0.08, 0.06, 0.18);
    let ground   = vec3<f32>(0.02, 0.015, 0.01);

    var sky_col : vec3<f32>;
    if h > 0.0 {
        sky_col = mix(horizon, zenith, pow(h, 0.4));
    } else {
        sky_col = mix(horizon, ground, pow(-h, 0.5));
    }

    // Subtle animated nebula wisps
    let wisp_uv = vec2<f32>(ray.x + t * 0.008, ray.z + t * 0.005);
    let wisp    = hash(wisp_uv * 3.0) * 0.04 * smoothstep(0.0, 0.5, h);
    sky_col    += vec3<f32>(0.3, 0.1, 0.5) * wisp;

    // Stars (only above horizon)
    let star = stars(ray, t) * smoothstep(-0.05, 0.1, h);
    sky_col  += vec3<f32>(0.9, 0.95, 1.0) * star;

    // Subtle warm glow near horizon (light source direction)
    let sun_dir  = normalize(vec3<f32>(1.0, 0.1, 0.5));
    let sun_dot  = max(dot(ray, sun_dir), 0.0);
    sky_col     += vec3<f32>(0.4, 0.2, 0.05) * pow(sun_dot, 6.0) * 0.3;

    return vec4<f32>(sky_col, 1.0);
}
