// SPDX-FileCopyrightText: 2026 Frederick Ziola
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::{game_object::GameObject, sprite_atlas::SpriteAtlas, viewport::Viewport};
use bytemuck::{offset_of, Pod, Zeroable};
use std::sync::Arc;
use wesl::include_wesl;
use winit::window::Window;

#[repr(C)]
#[derive(Copy, Clone, Default, Pod, Zeroable)]
struct GBufferInstanceInput {
    pub size: glam::Vec2,
    pub position: glam::Vec2,
    pub rotation: f32,
    pub texture_offset: [u32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Default, Pod, Zeroable)]
struct Uniforms {
    pub time: f32,

    pub light_segment_workgroups: u32,

    pub viewport_center: glam::Vec2,
    pub viewport_size: glam::Vec2,
}

#[repr(C)]
#[derive(Copy, Clone, Default, Pod, Zeroable)]
struct LightSegmentVertex {
    pub position: glam::Vec2,
    pub color: u32, // packed RGBA8unorm
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    is_surface_configured: bool,
    device: wgpu::Device,
    pub queue: wgpu::Queue,

    uniforms: Uniforms,
    uniforms_dirty: bool,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,

    pub sprite_atlas: SpriteAtlas,
    sprite_atlas_type_view: wgpu::TextureView,
    sprite_atlas_albd_view: wgpu::TextureView,
    sprite_atlas_norm_view: wgpu::TextureView,
    sprite_atlas_bind_group: wgpu::BindGroup,

    gbuffer_instance_count: u64,
    gbuffer_instance_capacity: u64,
    gbuffer_instance_buffer: wgpu::Buffer,

    gbuffer_type: wgpu::Texture,
    gbuffer_albd: wgpu::Texture,
    gbuffer_norm: wgpu::Texture,
    gbuffer_type_view: wgpu::TextureView,
    gbuffer_albd_view: wgpu::TextureView,
    gbuffer_norm_view: wgpu::TextureView,

    gbuffer_bind_group: wgpu::BindGroup,

    light_segment_vertices: wgpu::Buffer,
    light_segment_indirect_args: wgpu::Buffer,

    light_segment_bind_group: wgpu::BindGroup,

    light_map: wgpu::Texture,
    light_map_msaa: wgpu::Texture,

    light_map_sampler: wgpu::Sampler,
    light_map_view: wgpu::TextureView,
    light_map_msaa_view: wgpu::TextureView,

    light_map_bind_group: wgpu::BindGroup,

    frame_buffer: wgpu::Texture,

    // see render_pipeline.excalidraw
    gbuffer_pipeline: wgpu::RenderPipeline,
    light_segment_pipeline: wgpu::ComputePipeline,
    light_draw_pipeline: wgpu::RenderPipeline,
    compositor_pipeline: wgpu::RenderPipeline,
}

impl Renderer {
    const SPRITE_TYPE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Uint;
    const SPRITE_ALBD_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
    const SPRITE_NORM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

    const SPRITE_ATLAS_WIDTH: u32 = 1024;
    const SPRITE_ATLAS_HEIGHT: u32 = 1024;

    const GBUFFER_TYPE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Uint;
    const GBUFFER_ALBD_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
    const GBUFFER_NORM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

    const LIGHT_WORKGROUPS: u64 = 18;
    const LIGHT_WORKGROUP_SIZE: u64 = 64;
    const LIGHT_MAX_SEGMENTS_PER_INVOC: u64 = 10;

    const LIGHT_SEGMENT_VERTICES_SIZE: u64 = 2
        * Self::LIGHT_WORKGROUPS
        * Self::LIGHT_WORKGROUP_SIZE
        * Self::LIGHT_MAX_SEGMENTS_PER_INVOC;

    const LIGHT_MAP_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    const FRAME_BUFFER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

    const WORLD_BUFFER_WIDTH: u32 = 1024;
    const WORLD_BUFFER_HEIGHT: u32 = 1024;

    fn create_world_buffer(device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gbuffer"),
            size: wgpu::Extent3d {
                width: Self::WORLD_BUFFER_WIDTH,
                height: Self::WORLD_BUFFER_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 11,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[format],
        })
    }

    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: None,
        });

        let surface = instance.create_surface(window.clone())?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: true,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("sightglass main rendering device"),
                required_features: wgpu::Features::empty(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| !f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        log::info!("{:#?}", surface_format);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Srgb,
        };

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniform buffer"),
            size: size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("uniform bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZero::new(size_of::<Uniforms>() as u64),
                    },
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    count: None,
                }],
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniform bind group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform_buffer,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        let gbuffer_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gbuffer instance buffer"),
            size: 0,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sprite_atlas_type = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sprite atlas type"),
            size: wgpu::Extent3d {
                width: Self::SPRITE_ATLAS_WIDTH,
                height: Self::SPRITE_ATLAS_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::SPRITE_TYPE_FORMAT,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[Self::SPRITE_TYPE_FORMAT],
        });

        let sprite_atlas_albd = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sprite atlas albd"),
            size: wgpu::Extent3d {
                width: Self::SPRITE_ATLAS_WIDTH,
                height: Self::SPRITE_ATLAS_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::SPRITE_ALBD_FORMAT,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[Self::SPRITE_ALBD_FORMAT],
        });

        let sprite_atlas_norm = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sprite altas norm"),
            size: wgpu::Extent3d {
                width: Self::SPRITE_ATLAS_WIDTH,
                height: Self::SPRITE_ATLAS_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::SPRITE_NORM_FORMAT,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[Self::SPRITE_NORM_FORMAT],
        });

        let sprite_atlas_type_view =
            sprite_atlas_type.create_view(&wgpu::TextureViewDescriptor::default());
        let sprite_atlas_albd_view =
            sprite_atlas_albd.create_view(&wgpu::TextureViewDescriptor::default());
        let sprite_atlas_norm_view =
            sprite_atlas_norm.create_view(&wgpu::TextureViewDescriptor::default());

        let sprite_atlas_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("sprite atlas bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        // type
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Uint,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        // albd
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        // norm
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        let sprite_atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprite atlas bind group"),
            layout: &sprite_atlas_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    // type
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&sprite_atlas_type_view),
                },
                wgpu::BindGroupEntry {
                    // albd
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&sprite_atlas_albd_view),
                },
                wgpu::BindGroupEntry {
                    // norm
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&sprite_atlas_norm_view),
                },
            ],
        });

        let gbuffer_type = Self::create_world_buffer(&device, Self::GBUFFER_TYPE_FORMAT);
        let gbuffer_albd = Self::create_world_buffer(&device, Self::GBUFFER_ALBD_FORMAT);
        let gbuffer_norm = Self::create_world_buffer(&device, Self::GBUFFER_NORM_FORMAT);

        let gbuffer_type_view = gbuffer_type.create_view(&wgpu::TextureViewDescriptor {
            base_mip_level: 0,
            mip_level_count: Some(1),
            ..Default::default()
        });
        let gbuffer_albd_view = gbuffer_albd.create_view(&wgpu::TextureViewDescriptor {
            base_mip_level: 0,
            mip_level_count: Some(1),
            ..Default::default()
        });
        let gbuffer_norm_view = gbuffer_norm.create_view(&wgpu::TextureViewDescriptor {
            base_mip_level: 0,
            mip_level_count: Some(1),
            ..Default::default()
        });

        let gbuffer_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("gbuffer bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        // type
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Uint,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        // albd
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        // norm
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        let gbuffer_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gbuffer bind group"),
            layout: &gbuffer_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&gbuffer_type_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&gbuffer_albd_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&gbuffer_norm_view),
                },
            ],
        });

        let gbuffer_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gbuffer"),
            source: wgpu::ShaderSource::Wgsl(include_wesl!("gbuffer").into()),
        });

        let gbuffer_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("gbuffer pipeline layout"),
                bind_group_layouts: &[
                    Some(&uniform_bind_group_layout),
                    Some(&sprite_atlas_bind_group_layout),
                ],
                immediate_size: 0,
            });

        let gbuffer_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gbuffer pipeline"),
            layout: Some(&gbuffer_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &gbuffer_shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<GBufferInstanceInput>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            // size
                            format: wgpu::VertexFormat::Float32x2,
                            offset: offset_of!(GBufferInstanceInput, size) as u64,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            // position
                            format: wgpu::VertexFormat::Float32x2,
                            offset: offset_of!(GBufferInstanceInput, position) as u64,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            // rotation
                            format: wgpu::VertexFormat::Float32,
                            offset: offset_of!(GBufferInstanceInput, rotation) as u64,
                            shader_location: 2,
                        },
                        wgpu::VertexAttribute {
                            // texture_offset
                            format: wgpu::VertexFormat::Uint32x2,
                            offset: offset_of!(GBufferInstanceInput, texture_offset) as u64,
                            shader_location: 3,
                        },
                    ],
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &gbuffer_shader,
                entry_point: Some("fs_main"),
                targets: &[
                    Some(wgpu::ColorTargetState {
                        // type
                        format: Self::GBUFFER_TYPE_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                    Some(wgpu::ColorTargetState {
                        // albd
                        format: Self::GBUFFER_ALBD_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                    Some(wgpu::ColorTargetState {
                        // norm
                        format: Self::GBUFFER_NORM_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                ],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        let light_segment_vertices_size =
            size_of::<LightSegmentVertex>() as u64 * Self::LIGHT_SEGMENT_VERTICES_SIZE;
        let light_segment_vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("light segment vertices"),
            size: light_segment_vertices_size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let light_segment_indirect_args = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("light segment indirect args"),
            size: size_of::<wgpu::util::DrawIndirectArgs>() as u64,
            usage: wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let light_segment_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("light segment bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            // indirect args
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: std::num::NonZero::new(size_of::<
                                wgpu::util::DrawIndirectArgs,
                            >()
                                as u64),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            // vertices
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: std::num::NonZero::new(light_segment_vertices_size),
                        },
                        count: None,
                    },
                ],
            });

        let light_segment_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("light segment bind group"),
            layout: &light_segment_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    // indirect args
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &light_segment_indirect_args,
                        offset: 0,
                        size: std::num::NonZero::new(
                            size_of::<wgpu::util::DrawIndirectArgs>() as u64
                        ),
                    }),
                },
                wgpu::BindGroupEntry {
                    // vertices
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &light_segment_vertices,
                        offset: 0,
                        size: std::num::NonZero::new(light_segment_vertices_size),
                    }),
                },
            ],
        });

        let light_map = Self::create_world_buffer(&device, Self::LIGHT_MAP_FORMAT);
        let light_map_msaa = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("light map msaa"),
            size: wgpu::Extent3d {
                width: Self::WORLD_BUFFER_WIDTH,
                height: Self::WORLD_BUFFER_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 4,
            dimension: wgpu::TextureDimension::D2,
            format: Self::LIGHT_MAP_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let light_map_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("light map sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });

        let light_map_view = light_map.create_view(&wgpu::TextureViewDescriptor {
            mip_level_count: Some(1),
            ..Default::default()
        });
        let light_map_msaa_view =
            light_map_msaa.create_view(&wgpu::TextureViewDescriptor::default());

        let light_map_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("light map bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        let light_map_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("light map bind group"),
            layout: &light_map_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&light_map_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&light_map_view),
                },
            ],
        });

        let light_segment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("light segment"),
            source: wgpu::ShaderSource::Wgsl(include_wesl!("light_segment").into()),
        });

        let light_segment_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("light segment pipeline layout"),
                bind_group_layouts: &[
                    Some(&uniform_bind_group_layout),
                    Some(&gbuffer_bind_group_layout),
                    Some(&light_segment_bind_group_layout),
                ],
                immediate_size: 0,
            });

        let light_segment_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("light segment pipeline"),
                layout: Some(&light_segment_pipeline_layout),
                module: &light_segment_shader,
                entry_point: Some("cs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });

        let light_draw_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("light draw"),
            source: wgpu::ShaderSource::Wgsl(include_wesl!("light_draw").into()),
        });

        let light_draw_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("light draw pipeline layout"),
                bind_group_layouts: &[],
                immediate_size: 0,
            });

        let light_draw_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("light draw pipeline"),
            layout: Some(&light_draw_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &light_draw_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<LightSegmentVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: offset_of!(LightSegmentVertex, position) as u64,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Uint32,
                            offset: offset_of!(LightSegmentVertex, color) as u64,
                            shader_location: 1,
                        },
                    ],
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                mask: !0,
                count: 4,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &light_draw_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: Self::LIGHT_MAP_FORMAT,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::all(),
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let frame_buffer = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("frame buffer"),
            size: wgpu::Extent3d {
                width: surface_config.width,
                height: surface_config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::FRAME_BUFFER_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[Self::FRAME_BUFFER_FORMAT],
        });

        let compositor_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("compositor"),
            source: wgpu::ShaderSource::Wgsl(include_wesl!("compositor").into()),
        });

        let compositor_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("compositor pipeline layout"),
                bind_group_layouts: &[
                    Some(&uniform_bind_group_layout),
                    Some(&gbuffer_bind_group_layout),
                    Some(&light_map_bind_group_layout),
                ],
                immediate_size: 0,
            });

        let compositor_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("compositor pipeline"),
            layout: Some(&compositor_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &compositor_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &compositor_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format, //Self::FRAME_BUFFER_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        Ok(Self {
            surface,
            surface_config,
            is_surface_configured: false,
            device,
            queue,

            uniforms: Uniforms::default(),
            uniforms_dirty: true,
            uniform_buffer,
            uniform_bind_group,

            sprite_atlas: SpriteAtlas::new(sprite_atlas_type, sprite_atlas_albd, sprite_atlas_norm),

            sprite_atlas_type_view,
            sprite_atlas_albd_view,
            sprite_atlas_norm_view,

            sprite_atlas_bind_group,

            gbuffer_instance_count: 0,
            gbuffer_instance_capacity: 0,
            gbuffer_instance_buffer,

            gbuffer_type,
            gbuffer_albd,
            gbuffer_norm,
            gbuffer_type_view,
            gbuffer_albd_view,
            gbuffer_norm_view,

            gbuffer_bind_group,

            light_segment_vertices,
            light_segment_indirect_args,
            light_segment_bind_group,

            light_map_msaa,
            light_map,

            light_map_sampler,
            light_map_view,
            light_map_msaa_view,

            light_map_bind_group,

            frame_buffer,

            gbuffer_pipeline,
            light_segment_pipeline,
            light_draw_pipeline,
            compositor_pipeline,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
            self.is_surface_configured = true;
        }
    }

    pub fn render(&mut self) -> anyhow::Result<()> {
        if !self.is_surface_configured {
            return Ok(());
        }

        self.uniforms.light_segment_workgroups = Self::LIGHT_WORKGROUPS as u32;
        if self.uniforms_dirty {
            self.upload_uniforms();
        }

        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => {
                // Skip this frame
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.surface_config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                // where did it go
                anyhow::bail!("Lost surface/device");
            }
        };

        let frame_buffer_view = self
            .frame_buffer
            .create_view(&wgpu::TextureViewDescriptor::default());

        let output_view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("main render encoder"),
            });

        {
            let mut gbuffer_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gbuffer pass"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.gbuffer_type_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.0,
                                g: 0.0,
                                b: 0.0,
                                a: 0.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.gbuffer_albd_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.0,
                                g: 0.0,
                                b: 0.0,
                                a: 0.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.gbuffer_norm_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.0,
                                g: 0.0,
                                b: 0.0,
                                a: 0.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: None,
                multiview_mask: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if self.gbuffer_instance_count > 0 {
                gbuffer_pass.set_pipeline(&self.gbuffer_pipeline);
                gbuffer_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                gbuffer_pass.set_bind_group(1, &self.sprite_atlas_bind_group, &[]);
                gbuffer_pass.set_vertex_buffer(0, self.gbuffer_instance_buffer.slice(..));
                gbuffer_pass.draw(0..4, 0..(self.gbuffer_instance_count as u32));
            }
        }

        // reset the indirect args (specifically vertex count)
        self.queue.write_buffer(
            &self.light_segment_indirect_args,
            0,
            bytemuck::cast_slice(&[wgpu::util::DrawIndirectArgs {
                first_vertex: 0,
                vertex_count: 0,
                first_instance: 0,
                instance_count: 1,
            }]),
        );

        {
            let mut light_segment_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("light segment pass"),
                timestamp_writes: None,
            });

            light_segment_pass.set_pipeline(&self.light_segment_pipeline);
            light_segment_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            light_segment_pass.set_bind_group(1, &self.gbuffer_bind_group, &[]);
            light_segment_pass.set_bind_group(2, &self.light_segment_bind_group, &[]);

            light_segment_pass.dispatch_workgroups(Self::LIGHT_WORKGROUPS as u32, 1, 1);
        }

        {
            let mut light_draw_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("light draw pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.light_map_msaa_view,
                    depth_slice: None,
                    resolve_target: Some(&self.light_map_view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                multiview_mask: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            light_draw_pass.set_pipeline(&self.light_draw_pipeline);
            light_draw_pass.set_vertex_buffer(0, self.light_segment_vertices.slice(..));
            light_draw_pass.draw_indirect(&self.light_segment_indirect_args, 0);
        }

        // TODO: light accumulation

        {
            let mut compositor_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("compositor pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &output_view, //&frame_buffer_view
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                multiview_mask: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            compositor_pass.set_pipeline(&self.compositor_pipeline);
            compositor_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            compositor_pass.set_bind_group(1, &self.gbuffer_bind_group, &[]);
            compositor_pass.set_bind_group(2, &self.light_map_bind_group, &[]);
            compositor_pass.draw(0..4, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        self.queue.present(output);

        Ok(())
    }

    pub fn set_viewport(&mut self, viewport: &Viewport) {
        self.uniforms.viewport_center = viewport.center;
        self.uniforms.viewport_size.x = viewport.width;
        self.uniforms.viewport_size.y =
            viewport.width * self.surface_config.height as f32 / self.surface_config.width as f32;

        self.uniforms_dirty = true;
    }

    pub fn set_time(&mut self, time: f32) {
        self.uniforms.time = time;
        self.uniforms_dirty = true;
    }

    fn upload_uniforms(&mut self) {
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.uniforms]),
        );
        self.uniforms_dirty = false;
    }

    fn grow_gbuffer_instance_buffer(&mut self, min_capacity: u64) {
        let instance_size = size_of::<GBufferInstanceInput>() as u64;

        // Double the instance count capacity, or use min_capacity (with a sensible default minimum like 64)
        let new_capacity = std::cmp::max(
            min_capacity,
            std::cmp::max(self.gbuffer_instance_capacity * 2, 64),
        );

        let new_size = new_capacity * instance_size;

        self.gbuffer_instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gbuffer instance buffer"),
            size: new_size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.gbuffer_instance_capacity = new_capacity;
    }

    fn write_gbuffer_instances(
        &mut self,
        start_pos: u64,
        instances: &[GBufferInstanceInput],
    ) -> anyhow::Result<()> {
        if instances.len() == 0 {
            return Ok(());
        }

        let required_capacity: u64 = start_pos + instances.len() as u64;

        if required_capacity >= self.gbuffer_instance_capacity {
            self.grow_gbuffer_instance_buffer(required_capacity);
        }

        self.gbuffer_instance_count = std::cmp::max(required_capacity, self.gbuffer_instance_count);

        let instance_size = size_of::<GBufferInstanceInput>() as u64;

        self.queue
            .write_buffer_with(
                &self.gbuffer_instance_buffer,
                instance_size * start_pos,
                std::num::NonZero::new(instance_size * instances.len() as u64)
                    .ok_or(anyhow::anyhow!("instance must not be zero size"))?,
            )
            .ok_or(anyhow::anyhow!("failed to write buffer"))?
            .copy_from_slice(bytemuck::cast_slice(instances));

        Ok(())
    }

    pub fn update_gameobjects(&mut self, game_objects: &[GameObject]) -> anyhow::Result<()> {
        let instances: Vec<GBufferInstanceInput> = game_objects
            .iter()
            .map(|object| GBufferInstanceInput {
                position: object.transform.position,
                rotation: object.transform.rotation,
                size: glam::Vec2::new(object.size.0 as f32, object.size.1 as f32),
                texture_offset: self
                    .sprite_atlas
                    .get_offset(object.sprite_id.clone())
                    .unwrap()
                    .into(),
            })
            .collect();

        self.write_gbuffer_instances(0, &instances)?;

        Ok(())
    }
}
