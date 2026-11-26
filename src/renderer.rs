use crate::{game_object::GameObject, sprite_atlas::SpriteAtlas};
use bytemuck::{offset_of, Pod, Zeroable};
use std::sync::Arc;
use winit::window::Window;

#[repr(C)]
#[derive(Copy, Clone, Default, Pod, Zeroable)]
pub struct GBufferInstanceInput {
    size: glam::Vec2,
    position: glam::Vec2,
    rotation: f32,
    texture_offset: [u32; 2],
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    is_surface_configured: bool,
    device: wgpu::Device,
    pub queue: wgpu::Queue,

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

    light_map: wgpu::Texture,

    frame_buffer: wgpu::Texture,

    // see render_pipeline.excalidraw
    gbuffer_pipeline: wgpu::RenderPipeline,
    lighting_pipeline: wgpu::ComputePipeline,
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

    const LIGHT_MAP_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;

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
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };

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

        let gbuffer_shader =
            device.create_shader_module(wgpu::include_wgsl!("shaders/gbuffer.wgsl"));

        let gbuffer_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("gbuffer pipeline layout"),
                bind_group_layouts: &[
                    None, // reserved for uniforms
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

        let light_map = Self::create_world_buffer(&device, Self::LIGHT_MAP_FORMAT);

        let lighting_shader =
            device.create_shader_module(wgpu::include_wgsl!("shaders/lighting.wgsl"));

        let lighting_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("lighting pipeline layout"),
                bind_group_layouts: &[],
                immediate_size: 0,
            });

        let lighting_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("lighting pipeline"),
            layout: Some(&lighting_pipeline_layout),
            module: &lighting_shader,
            entry_point: Some("cs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
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

        let compositor_shader =
            device.create_shader_module(wgpu::include_wgsl!("shaders/compositor.wgsl"));

        let compositor_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("compositor pipeline layout"),
                bind_group_layouts: &[
                    None, // reserved for uniforms
                    Some(&gbuffer_bind_group_layout),
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

            light_map,

            frame_buffer,

            gbuffer_pipeline,
            lighting_pipeline,
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
                anyhow::bail!("Lost device");
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
                gbuffer_pass.set_bind_group(1, &self.sprite_atlas_bind_group, &[]);
                gbuffer_pass.set_vertex_buffer(0, self.gbuffer_instance_buffer.slice(..));
                gbuffer_pass.draw(0..4, 0..(self.gbuffer_instance_count as u32));
            }
        }

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
            compositor_pass.set_bind_group(1, &self.gbuffer_bind_group, &[]);
            compositor_pass.draw(0..4, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        self.queue.present(output);

        Ok(())
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

    pub fn write_gbuffer_instances(
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
