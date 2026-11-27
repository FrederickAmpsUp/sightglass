struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: u32, // rgba8unorm packed
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    
    @location(0) color: vec3<f32>
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    out.clip_position = vec4<f32>((in.position / 1024.0) * 2.0 - 1.0, 0.0, 1.0);
    out.color = unpack4x8unorm(in.color).rgb;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0); // might do brightness comp. here
}
