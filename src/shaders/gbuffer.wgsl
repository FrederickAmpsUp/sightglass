struct VertexInput {
    @builtin(vertex_index) vertex_index: u32
}

struct InstanceInput {
    size: vec2<f32>,
    position: vec2<f32>,
    rotation: f32
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>
}

struct FragmentOutput {
    @location(0) gb_type: u32,
    @location(1) gb_albd: vec4<f32>,
    @location(2) gb_norm: vec4<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let normpos = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 2.0,  1.0)
    )[in.vertex_index];

    out.clip_position = vec4<f32>(normpos / 2.0, 0.0, 1.0);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    out.gb_type = 1;
    out.gb_albd = vec4<f32>(1.0, 0.0, 0.0, 1.0);
    out.gb_norm = vec4<f32>(1.0, 0.0, 0.0, 1.0);

    return out;
}
