// SPDX-FileCopyrightText: 2026 Frederick Ziola
//
// SPDX-License-Identifier: GPL-3.0-only

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

struct DrawIndirectArgs {
    vertex_count: atomic<u32>,
    instance_count: u32,
    first_vertex: u32,
    first_instance: u32,
}

struct SegmentVertex {
    x: f32,
    y: f32, // if these are in a vec2 the struct aligns to 16 bytes and everything explodes
    color: u32, // rgba8unorm packed
}

@group(2) @binding(0)
var<storage, read_write> indirect_args: DrawIndirectArgs;
@group(2) @binding(1)
var<storage, read_write> vertices: array<SegmentVertex>;

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

const MAX_SEGMENTS_PER_THREAD = 10u;

var<private> local_vertices: array<SegmentVertex, 2*MAX_SEGMENTS_PER_THREAD>;
var<private> local_vertex_count = 0u;

fn push_vertex(position: vec2<f32>, color: vec4<f32>) {
    let vert = SegmentVertex(position.x, position.y, pack4x8unorm(color));

    if (local_vertex_count < 2*MAX_SEGMENTS_PER_THREAD) {
      local_vertices[local_vertex_count] = vert;
      local_vertex_count++;
    }
}

@compute @workgroup_size(64)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let index = gid.x;

    let theta = 6.28 * f32(index) / 256.0;

    let center = vec2<f32>(512.0, 512.0);
    let r = 1024.0;

    let color = vec4<f32>(0.01, 0.01, 0.01, 1.0);//vec4<f32>(vec3<f32>(f32(index) / 256.0), 1.0);

    push_vertex(center, color);
    push_vertex(center + vec2<f32>(cos(theta), sin(theta)) * r, color);

    let base_idx = atomicAdd(&indirect_args.vertex_count, local_vertex_count);

    for (var i = 0u; i < local_vertex_count; i++) {
        vertices[base_idx + i] = local_vertices[i];
    }
}
