@group(1) @binding(0)
var gb_type: texture_2d<u32>;
@group(1) @binding(1)
var gb_albd: texture_2d<f32>;
@group(1) @binding(2)
var gb_norm: texture_2d<f32>;

struct GBufferSample {
    gb_type: u32,
    gb_albd: vec4<f32>,
    gb_norm: vec4<f32>
}

fn sample_gbuffer(wpos: vec2<f32>) -> GBufferSample {
    let pxl = vec2<u32>(wpos);

    let s_type = textureLoad(gb_type, pxl, 0).r;
    let s_albd = textureLoad(gb_albd, pxl, 0);
    let s_norm = textureLoad(gb_norm, pxl, 0) * 2.0 - 1.0;

    return GBufferSample(s_type, s_albd, s_norm);
}
