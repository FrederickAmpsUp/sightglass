// SPDX-FileCopyrightText: 2026 Frederick Ziola
//
// SPDX-License-Identifier: GPL-3.0-only

import package::uniforms::uniforms;

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,

    @location(0) size: vec2<f32>,
    @location(1) position: vec2<f32>,
    @location(2) rotation: f32,
    @location(3) texture_offset: vec2<u32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,

    @location(1) @interpolate(flat) size: vec2<f32>,
    @location(2) @interpolate(flat) texture_offset: vec2<u32>,
    @location(3) @interpolate(flat) rotation: f32,
}

struct FragmentOutput {
    @location(0) gb_type: u32,
    @location(1) gb_albd: vec4<f32>,
    @location(2) gb_norm: vec4<f32>,
}

@group(1) @binding(0) var sprite_type: texture_2d<u32>;
@group(1) @binding(1) var sprite_albd: texture_2d<f32>;
@group(1) @binding(2) var sprite_norm: texture_2d<f32>;

struct SpriteSample {
    s_type: u32,
    s_albd: vec4<f32>,
    s_norm: vec4<f32>,
}

fn sample_sprite(vo: VertexOutput) -> SpriteSample {
    let pxl: vec2<u32> = vec2<u32>((vo.uv * vo.size)) + vo.texture_offset;

    var out: SpriteSample;

    out.s_type = textureLoad(sprite_type, pxl, 0).r;
    out.s_albd = textureLoad(sprite_albd, pxl, 0).rgba;
    out.s_norm = textureLoad(sprite_norm, pxl, 0).rgba;

    return out;
}

fn rotate2d(theta: f32) -> mat2x2<f32> {
    let c = cos(theta);
    let s = sin(theta);
    return mat2x2<f32>(
        vec2<f32>(c, s),
        vec2<f32>(-s, c)
    );
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let norm_pos = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0,  1.0)
    )[in.vertex_index];

    let world_pos = rotate2d(in.rotation) * (norm_pos * in.size / 2.0) + in.position;

    out.clip_position = vec4<f32>((world_pos / 1024.0) * 2.0 - 1.0, 0.0, 1.0);
    out.uv = norm_pos * 0.5 + 0.5;

    out.size = in.size;
    out.texture_offset = in.texture_offset;
    out.rotation = in.rotation;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    let sprite = sample_sprite(in);

    out.gb_type = sprite.s_type;
    out.gb_albd = sprite.s_albd;
    // rotate the normals
    out.gb_norm = vec4<f32>(normalize(rotate2d(-in.rotation) * (sprite.s_norm.xy * 2.0 - 1.0)), 0.0, 0.0);
    out.gb_norm *= f32(sprite.s_type != 0); // zero the normals where the sprite does not exist

    return out;
}
