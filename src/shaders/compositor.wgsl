// SPDX-FileCopyrightText: 2026 Frederick Ziola
//
// SPDX-License-Identifier: GPL-3.0-only

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec2<f32>
}

struct FragmentOutput {
    @location(0) frame_buf: vec4<f32>,
}

struct Uniforms {
    time: f32,
    viewport_center: vec2<f32>,
    viewport_size: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

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

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    let normpos = array<vec2<f32>, 4>(
        vec2<f32>( 1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0, -1.0),
    )[vertex_index];

    out.clip_position = vec4<f32>(normpos, 0.0, 1.0);
    out.world_position = normpos * (uniforms.viewport_size / 2.0) + uniforms.viewport_center;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    let gbuffer_sample = sample_gbuffer(in.world_position);
    var col = vec4<f32>(gbuffer_sample.gb_norm.rgb, 1.0);
    col = max(col, vec4<f32>(0.0));

    out.frame_buf = pow(col, vec4<f32>(1.0 / 2.2, 1.0 / 2.2, 1.0 / 2.2, 1.0));
    return out;
}

