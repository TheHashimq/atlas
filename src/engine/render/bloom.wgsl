struct VSOut {
    @builtin(position) pos : vec4<f32>,
    @location(0) uv : vec2<f32>,
};

@vertex
fn vs_fullscreen(@builtin(vertex_index) index : u32) -> VSOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );

    var out : VSOut;
    out.pos = vec4<f32>(positions[index], 0.0, 1.0);

    // Proper normalized UV
    out.uv = positions[index] * 0.5 + vec2<f32>(0.5);

    return out;
}

@group(0) @binding(0) var t_src : texture_2d<f32>;
@group(0) @binding(1) var s_src : sampler;


// ---- Threshold ----
@fragment
fn fs_threshold(in: VSOut) -> @location(0) vec4<f32> {
    let col  = textureSample(t_src, s_src, in.uv).rgb;

    let lum       = dot(col, vec3<f32>(0.2126, 0.7152, 0.0722));
    let threshold = 0.35;
    let knee      = 0.20;
    let rq        = clamp(lum - threshold + knee, 0.0, 2.0 * knee);
    let weight    = (rq * rq) / (4.0 * knee + 0.00001);
    let factor    = max(weight, lum - threshold) / max(lum, 0.00001);

    return vec4<f32>(col * factor, 1.0);
}


// ---- Horizontal blur ----
@fragment
fn fs_blur_h(in: VSOut) -> @location(0) vec4<f32> {
    let dims  = vec2<f32>(textureDimensions(t_src));
    let texel = vec2<f32>(1.0 / dims.x, 0.0);

    let w0 = 0.2270270270;
    let w1 = 0.1945945946;
    let w2 = 0.1216216216;
    let w3 = 0.0540540541;
    let w4 = 0.0162162162;

    var col = textureSample(t_src, s_src, in.uv).rgb * w0;
    col += textureSample(t_src, s_src, in.uv + texel * 1.0).rgb * w1;
    col += textureSample(t_src, s_src, in.uv - texel * 1.0).rgb * w1;
    col += textureSample(t_src, s_src, in.uv + texel * 2.0).rgb * w2;
    col += textureSample(t_src, s_src, in.uv - texel * 2.0).rgb * w2;
    col += textureSample(t_src, s_src, in.uv + texel * 3.0).rgb * w3;
    col += textureSample(t_src, s_src, in.uv - texel * 3.0).rgb * w3;
    col += textureSample(t_src, s_src, in.uv + texel * 4.0).rgb * w4;
    col += textureSample(t_src, s_src, in.uv - texel * 4.0).rgb * w4;

    return vec4<f32>(col, 1.0);
}


// ---- Vertical blur ----
@fragment
fn fs_blur_v(in: VSOut) -> @location(0) vec4<f32> {
    let dims  = vec2<f32>(textureDimensions(t_src));
    let texel = vec2<f32>(0.0, 1.0 / dims.y);

    let w0 = 0.2270270270;
    let w1 = 0.1945945946;
    let w2 = 0.1216216216;
    let w3 = 0.0540540541;
    let w4 = 0.0162162162;

    var col = textureSample(t_src, s_src, in.uv).rgb * w0;
    col += textureSample(t_src, s_src, in.uv + texel * 1.0).rgb * w1;
    col += textureSample(t_src, s_src, in.uv - texel * 1.0).rgb * w1;
    col += textureSample(t_src, s_src, in.uv + texel * 2.0).rgb * w2;
    col += textureSample(t_src, s_src, in.uv - texel * 2.0).rgb * w2;
    col += textureSample(t_src, s_src, in.uv + texel * 3.0).rgb * w3;
    col += textureSample(t_src, s_src, in.uv - texel * 3.0).rgb * w3;
    col += textureSample(t_src, s_src, in.uv + texel * 4.0).rgb * w4;
    col += textureSample(t_src, s_src, in.uv - texel * 4.0).rgb * w4;

    return vec4<f32>(col, 1.0);
}


// ---- Composite ----
@fragment
fn fs_composite(in: VSOut) -> @location(0) vec4<f32> {
    let bloom = textureSample(t_src, s_src, in.uv).rgb;
    return vec4<f32>(bloom * 2.5, 1.0);
}

