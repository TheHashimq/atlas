struct ShadowUniforms {
    light_view_proj : mat4x4<f32>,
    model           : mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> shadow : ShadowUniforms;

@vertex
fn vs_shadow(
    @location(0) position : vec3<f32>,
    @location(1) normal   : vec3<f32>,
    @location(2) uv       : vec2<f32>,
) -> @builtin(position) vec4<f32> {
    return shadow.light_view_proj * shadow.model * vec4<f32>(position, 1.0);
}

// No fragment shader needed — depth is written automatically
