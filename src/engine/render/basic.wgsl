const PI         : f32 = 3.14159265358979;
const TAU        : f32 = 6.28318530717959;
const MAX_LIGHTS : u32 = 4u;

struct SceneUniforms {
    view_proj        : mat4x4<f32>,
    light_view_proj  : mat4x4<f32>,
    camera_pos       : vec4<f32>,
    time             : vec4<f32>,
    light_pos        : array<vec4<f32>, 4>,
    light_color      : array<vec4<f32>, 4>,
    fog_params       : vec4<f32>,   // x=density
    fog_color        : vec4<f32>,   // rgb=linear fog colour
};

struct Material {
    base_color_factor : vec4<f32>,
    emissive_factor   : vec3<f32>,
    roughness_factor  : f32,
    metallic_factor   : f32,
    occlusion_factor  : f32,
    is_light          : f32,
    _pad              : f32,
};

struct ObjectUniforms {
    model    : mat4x4<f32>,
    material : Material,
    _pad     : vec4<f32>,
};

// --- Group 0: Global ---
@group(0) @binding(0) var<uniform> scene    : SceneUniforms;
@group(0) @binding(1) var          t_shadow : texture_depth_2d;
@group(0) @binding(2) var          s_shadow : sampler_comparison;

// --- Group 1: Material ---
@group(1) @binding(0) var<uniform> object : ObjectUniforms;
@group(1) @binding(1) var          t_base_color : texture_2d<f32>;
@group(1) @binding(2) var          s_material   : sampler;
@group(1) @binding(3) var          t_mr         : texture_2d<f32>;
@group(1) @binding(4) var          t_normal     : texture_2d<f32>;
@group(1) @binding(5) var          t_emissive   : texture_2d<f32>;
@group(1) @binding(6) var          t_occlusion  : texture_2d<f32>;

struct VSOut {
    @builtin(position) clip_pos   : vec4<f32>,
    @location(0)       world_pos  : vec3<f32>,
    @location(1)       normal     : vec3<f32>,
    @location(2)       uv         : vec2<f32>,
    @location(3)       tangent    : vec3<f32>,
    @location(4)       bitangent  : vec3<f32>,
    @location(5)       view_dist  : f32,
    @location(6)       shadow_pos : vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) position : vec3<f32>,
    @location(1) normal   : vec3<f32>,
    @location(2) uv       : vec2<f32>,
    @location(3) tangent  : vec4<f32>,
) -> VSOut {
    var out : VSOut;

    let world_pos  = object.model * vec4<f32>(position, 1.0);
    out.clip_pos   = scene.view_proj * world_pos;
    out.world_pos  = world_pos.xyz;
    out.uv         = uv;
    out.view_dist  = length(scene.camera_pos.xyz - world_pos.xyz);
    out.shadow_pos = scene.light_view_proj * world_pos;

    let nm = mat3x3<f32>(
        object.model[0].xyz,
        object.model[1].xyz,
        object.model[2].xyz,
    );
    
    let N = normalize(nm * normal);
    let T = normalize(nm * tangent.xyz);
    let B = cross(N, T) * tangent.w; 
    
    out.normal    = N;
    out.tangent   = T;
    out.bitangent = B;
    
    return out;
}

// ================================================================
//  BRDF
// ================================================================

fn D_GGX(NdH: f32, roughness: f32) -> f32 {
    let a  = roughness * roughness;
    let a2 = a * a;
    let d  = NdH * NdH * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d + 0.00001);
}

fn G_schlick_ggx(NdV: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return NdV / (NdV * (1.0 - k) + k);
}

fn G_smith(NdV: f32, NdL: f32, roughness: f32) -> f32 {
    return G_schlick_ggx(NdV, roughness) * G_schlick_ggx(NdL, roughness);
}

fn fresnel_schlick(cos_theta: f32, F0: vec3<f32>) -> vec3<f32> {
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

fn fresnel_schlick_rough(cos_theta: f32, F0: vec3<f32>, roughness: f32) -> vec3<f32> {
    return F0 + (max(vec3(1.0 - roughness), F0) - F0)
             * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

// ================================================================
//  ATMOSPHERE
// ================================================================

fn hemisphere_ambient(N: vec3<f32>) -> vec3<f32> {
    let sky    = vec3<f32>(0.04, 0.05, 0.07);
    let ground = vec3<f32>(0.01, 0.01, 0.01);
    return mix(ground, sky, N.y * 0.5 + 0.5);
}

fn apply_fog_linear(color: vec3<f32>, dist: f32) -> vec3<f32> {
    let density = scene.fog_params.x;
    let f       = exp(-pow(density * dist, 2.0));
    let factor  = clamp(f, 0.0, 1.0);
    return mix(scene.fog_color.rgb, color, factor);
}

fn checker(uv: vec2<f32>, scale: f32) -> f32 {
    let p = floor(uv * scale);
    return fract((p.x + p.y) * 0.5) * 2.0;
}

fn micro_normal(N: vec3<f32>, T: vec3<f32>, uv: vec2<f32>, strength: f32) -> vec3<f32> {
    let B  = normalize(cross(N, T));
    let s  = uv * 8.0;
    let dx = sin(s.x * 13.7 + s.y * 5.3) * strength;
    let dy = cos(s.x * 7.1  + s.y * 11.9) * strength;
    return normalize(N + T * dx + B * dy);
}

// ================================================================
//  PCF SHADOW
// ================================================================

fn sample_shadow(shadow_pos: vec4<f32>, NdL: f32) -> f32 {
    let proj  = shadow_pos.xyz / shadow_pos.w;
    let uv    = vec2<f32>(proj.x * 0.5 + 0.5, -proj.y * 0.5 + 0.5);
    let depth = proj.z;
    let bias  = max(0.005 * (1.0 - NdL), 0.001);
    let texel = 1.0 / 2048.0;

    let in_frustum = f32(
        uv.x >= 0.0 && uv.x <= 1.0 &&
        uv.y >= 0.0 && uv.y <= 1.0 &&
        depth >= 0.0 && depth <= 1.0
    );

    var shadow = 0.0;
    shadow += textureSampleCompare(t_shadow, s_shadow, uv + vec2(-texel, -texel), depth - bias);
    shadow += textureSampleCompare(t_shadow, s_shadow, uv + vec2(  0.0,  -texel), depth - bias);
    shadow += textureSampleCompare(t_shadow, s_shadow, uv + vec2( texel, -texel), depth - bias);
    shadow += textureSampleCompare(t_shadow, s_shadow, uv + vec2(-texel,    0.0), depth - bias);
    shadow += textureSampleCompare(t_shadow, s_shadow, uv + vec2(  0.0,     0.0), depth - bias);
    shadow += textureSampleCompare(t_shadow, s_shadow, uv + vec2( texel,    0.0), depth - bias);
    shadow += textureSampleCompare(t_shadow, s_shadow, uv + vec2(-texel,  texel), depth - bias);
    shadow += textureSampleCompare(t_shadow, s_shadow, uv + vec2(  0.0,   texel), depth - bias);
    shadow += textureSampleCompare(t_shadow, s_shadow, uv + vec2( texel,  texel), depth - bias);
    shadow /= 9.0;

    return mix(1.0, shadow, in_frustum);
}

// ================================================================
//  PER-LIGHT BRDF
// ================================================================

fn point_light_brdf(
    N         : vec3<f32>,
    V         : vec3<f32>,
    world_pos : vec3<f32>,
    F0        : vec3<f32>,
    albedo    : vec3<f32>,
    roughness : f32,
    metallic  : f32,
    light_pos : vec3<f32>,
    light_col : vec3<f32>,
    intensity : f32,
    shadow    : f32,
) -> vec3<f32> {
    let lvec = light_pos - world_pos;
    let dist = length(lvec);
    let L    = normalize(lvec);
    let H    = normalize(V + L);

    let NdL = max(dot(N, L), 0.0);
    if NdL <= 0.0 { return vec3(0.0); }

    let NdV = max(dot(N, V), 0.0001);
    let NdH = max(dot(N, H), 0.0);
    let HdV = max(dot(H, V), 0.0);

    let atten = intensity / (1.0 + 0.14 * dist + 0.07 * dist * dist);

    let D    = D_GGX(NdH, roughness);
    let G    = G_smith(NdV, NdL, roughness);
    let F    = fresnel_schlick(HdV, F0);
    let spec = (D * G * F) / (4.0 * NdV * NdL + 0.0001);
    let kD   = (1.0 - F) * (1.0 - metallic);
    let diff = kD * albedo / PI;
    let rim  = pow(1.0 - NdV, 4.0) * 0.15;

    return (diff + spec) * light_col * NdL * atten * shadow
         + light_col * rim * atten * 0.15;
}

// ================================================================
//  SUN SURFACE — animated granules + corona
// ================================================================

fn hash21(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(127.1, 311.7));
    q += dot(q, q + 19.19);
    return fract(q.x * q.y);
}

// Smooth value noise
fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);   // smoothstep
    return mix(
        mix(hash21(i + vec2(0.0, 0.0)), hash21(i + vec2(1.0, 0.0)), u.x),
        mix(hash21(i + vec2(0.0, 1.0)), hash21(i + vec2(1.0, 1.0)), u.x),
        u.y,
    );
}

// Layered turbulence
fn fbm(p: vec2<f32>) -> f32 {
    var val  = 0.0;
    var amp  = 0.5;
    var freq = 1.0;
    for (var i = 0; i < 5; i++) {
        val  += amp * vnoise(p * freq);
        amp  *= 0.5;
        freq *= 2.1;
    }
    return val;
}

fn sun_color(
    N       : vec3<f32>,   // surface normal (sphere → same as pos on unit sphere)
    V       : vec3<f32>,   // view direction
    t       : f32,
    albedo  : vec3<f32>,
    emissive: vec3<f32>,
) -> vec3<f32> {

    // ---- UV from normal (spherical) ----
    let uv = vec2<f32>(
        atan2(N.z, N.x) / TAU + 0.5,
        asin(clamp(N.y, -1.0, 1.0)) / PI + 0.5,
    );

    // ---- Animated surface noise (granules / convection cells) ----
    let slow_t  = t * 0.06;
    let fast_t  = t * 0.18;
    let uv_spin = uv + vec2<f32>(slow_t, 0.0);   // rotate with time

    let granule = fbm(uv_spin * 6.0);
    let surface = fbm(uv_spin * 12.0 + vec2<f32>(granule * 2.0, fast_t));

    // ---- Hot-core colour palette ----
    //   0.0 = corona orange-red, 0.5 = yellow, 1.0 = white hot core
    let t_col   = clamp(surface * 1.4, 0.0, 1.0);
    let corona  = vec3<f32>(1.0, 0.25, 0.02);     // deep red-orange
    let mid     = vec3<f32>(1.0, 0.72, 0.08);     // golden yellow
    let core    = vec3<f32>(1.0, 0.96, 0.82);     // near-white
    let base_col = mix(mix(corona, mid, t_col), core, t_col * t_col);

    // ---- Rim / corona glow effect ----
    let NdV     = max(dot(N, V), 0.0001);
    let rim     = pow(1.0 - NdV, 2.5);       // bright ring at silhouette
    let corona_col = vec3<f32>(1.0, 0.35, 0.0) * (rim * 5.0);

    // ---- Solar flare spikes (4 angular spikes radiating at rim) ----
    let angle     = atan2(N.z, N.x) + t * 0.12;  // rotate spikes slowly
    let spike_raw = abs(sin(angle * 4.0));         // 4-fold symmetry
    let spike_mask= pow(rim, 0.8) * pow(spike_raw, 6.0);
    let flares    = vec3<f32>(1.0, 0.55, 0.05) * spike_mask * 8.0;

    // ---- Chromatic pulsing — subtle hue drift ----
    let pulse     = 0.85 + 0.15 * sin(t * 2.3);
    let tinted    = base_col * vec3<f32>(pulse, 0.95, 0.85 - pulse * 0.1);

    // ---- Combine ----
    let surface_hdr = tinted * emissive * 1.8;
    return surface_hdr + corona_col + flares;
}

// ================================================================
//  FRAGMENT
// ================================================================

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {

    let t      = scene.time.x;
    let mat    = object.material;
    
    // 1. Sample Textures or use factors
    let base_color_tex = textureSample(t_base_color, s_material, in.uv);
    let albedo = mat.base_color_factor.rgb * base_color_tex.rgb;
    
    let mr_tex = textureSample(t_mr, s_material, in.uv);
    let roughness = clamp(mat.roughness_factor * mr_tex.g, 0.04, 1.0);
    let metallic  = clamp(mat.metallic_factor  * mr_tex.b, 0.0, 1.0);

    let emissive_tex = textureSample(t_emissive, s_material, in.uv);
    let emissive = mat.emissive_factor * emissive_tex.rgb;

    // 2. Normal Mapping (Tangent Space)
    let normal_map = textureSample(t_normal, s_material, in.uv).rgb * 2.0 - 1.0;
    let TBN = mat3x3<f32>(normalize(in.tangent), normalize(in.bitangent), normalize(in.normal));
    let N_surface = normalize(TBN * normal_map);

    let V = normalize(scene.camera_pos.xyz - in.world_pos);

    // 3. Emissive / Light source handling
    if mat.is_light > 0.5 {
        let col = sun_color(N_surface, V, t, albedo, mat.emissive_factor);
        return vec4<f32>(apply_fog_linear(col, in.view_dist), 1.0);
    }

    // 4. Lighting Calculation
    let NdV = max(dot(N_surface, V), 0.0001);
    let F0  = mix(vec3<f32>(0.04), albedo, metallic);

    var Lo = vec3<f32>(0.0);
    
    // Hardcoded simple shadow for the first light
    let L0   = normalize(scene.light_pos[0].xyz - in.world_pos);
    let NdL0 = max(dot(N_surface, L0), 0.0);
    let shadow = sample_shadow(in.shadow_pos, NdL0);

    for (var i = 0u; i < MAX_LIGHTS; i++) {
        let lpos    = scene.light_pos[i].xyz;
        let lcol    = scene.light_color[i].rgb;
        let lintens = scene.light_color[i].w;
        if lintens < 0.001 { continue; }

        let s = select(1.0, shadow, i == 0u);

        Lo += point_light_brdf(
            N_surface, V, in.world_pos, F0, albedo,
            roughness, metallic, lpos, lcol, lintens, s,
        );
    }

    // 5. Ambient / IBL baseline — Boosted
    let occlusion = textureSample(t_occlusion, s_material, in.uv).r * object.material.occlusion_factor;
    
    // Virtual Hemisphere / Environment term
    let env_ambient = hemisphere_ambient(N_surface) * albedo * 0.15; // Increased boost
    let base_ambient = albedo * 0.1;
    let ambient = (base_ambient + env_ambient) * occlusion;
    
    // Cinematic Metallic Sheen (Pseudo-IBL)
    // Highly reflective metals get a boost in ambient to look "shiny" without real reflections
    let refl_vec = reflect(-V, N_surface);
    let view_dot_refl = max(dot(V, refl_vec), 0.0);
    let metallic_spec = pow(view_dot_refl, 32.0) * metallic * F0 * 0.5;
    
    // 6. Combine Lo + Ambient + Boosted Emissive
    var color = Lo + ambient + metallic_spec + (emissive * 3.5); // Boosted emissive factor
    
    // Subtle chromatic fringe/fog
    color = apply_fog_linear(color, in.view_dist);
    
    return vec4<f32>(color, 1.0);
}
