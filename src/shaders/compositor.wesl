// SPDX-FileCopyrightText: 2026 Frederick Ziola
//
// SPDX-License-Identifier: GPL-3.0-only

import package::{uniforms::uniforms, gbuffer_sample::sample_gbuffer};

@group(2) @binding(0)
var light_map_sampler: sampler;
@group(2) @binding(1)
var light_map: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec2<f32>
}

struct FragmentOutput {
    @location(0) frame_buf: vec4<f32>,
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
    let light_map_sample = textureSample(light_map, light_map_sampler, in.world_position / 1024.0);
    var col = light_map_sample;//vec4<f32>(gbuffer_sample.gb_norm.rgb, 1.0);
    col = max(col, vec4<f32>(0.0)); // remove negatives

    out.frame_buf = pow(col, vec4<f32>(1.0 / 2.2, 1.0 / 2.2, 1.0 / 2.2, 1.0));
    return out;
}

