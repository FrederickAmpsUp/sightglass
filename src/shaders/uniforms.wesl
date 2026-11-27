struct Uniforms {
    time: f32,
    viewport_center: vec2<f32>,
    viewport_size: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;
