// SPDX-FileCopyrightText: 2026 Frederick Ziola
//
// SPDX-License-Identifier: GPL-3.0-only

import package::{raymarch::march, uniforms::uniforms, gbuffer_sample::sample_gbuffer};
import package::lygia::generative::random::random::random;

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

    let total_invocations = (f32(uniforms.light_segment_workgroups) * 64.0);

    let theta = 6.28318 * f32(index) / total_invocations
        + (random(f32(index) + uniforms.time) * 2.0 - 1.0) * (3.14159 / total_invocations);

    let origin = vec2<f32>(512.0, 512.0 + sin(uniforms.time) * 16.0);
    var hit = origin;

    march(&hit, normalize(vec2<f32>(cos(theta), sin(theta))));

    let channel = u32(random(f32(index) + uniforms.time + 0.5) * 3.0) % 3;
    //let color = vec4<f32>(0.05 * f32(channel == 0), 0.05 * f32(channel == 1), 0.05 * f32(channel == 2), 1.0);//vec4<f32>(vec3<f32>(f32(index) / 256.0), 1.0);
    let color = vec4<f32>(0.015, 0.015, 0.015, 1.0);

    push_vertex(origin, color);
    push_vertex(hit, color);

    let base_idx = atomicAdd(&indirect_args.vertex_count, local_vertex_count);

    for (var i = 0u; i < local_vertex_count; i++) {
        vertices[base_idx + i] = local_vertices[i];
    }
}
