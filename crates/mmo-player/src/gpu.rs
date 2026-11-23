use std::sync::Arc;

use mmo_renderer::raster3d::SceneVertex;
use wgpu::util::DeviceExt;
use wgpu::{
    Backends, BindGroup, BindGroupLayout, Color, CommandEncoderDescriptor, Device,
    DeviceDescriptor, Features, Instance, Limits, LoadOp, Operations, PowerPreference, PresentMode,
    Queue, RenderPassColorAttachment, RenderPassDescriptor, RequestAdapterOptions, Surface,
    SurfaceConfiguration, Texture, TextureDimension, TextureFormat, TextureUsages, TextureView,
    TextureViewDescriptor,
};
use winit::window::Window;

const SCENE_VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 2] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];

pub struct GpuSurface {
    surface: Surface<'static>,
    device: Device,
    queue: Queue,
    config: SurfaceConfiguration,
    texture: Texture,
    texture_view: TextureView,
    bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
    ui_texture: Texture,
    ui_texture_view: TextureView,
    ui_bind_group: BindGroup,
    pipeline: wgpu::RenderPipeline,
    scene_pipeline: wgpu::RenderPipeline,
    depth_texture: Texture,
    depth_view: TextureView,
}

impl GpuSurface {
    pub fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();
        let width = size.width.max(320);
        let height = size.height.max(240);

        let instance = Instance::new(&wgpu::InstanceDescriptor {
            backends: Backends::all(),
            ..Default::default()
        });
        let surface = instance
            .create_surface(window)
            .expect("create wgpu surface");
        let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("find a compatible wgpu adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(&DeviceDescriptor {
            label: Some("WFO GPU"),
            required_features: Features::empty(),
            required_limits: Limits::downlevel_webgl2_defaults(),
            memory_hints: wgpu::MemoryHints::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            trace: wgpu::Trace::Off,
        }))
        .expect("create wgpu device");

        let surface_format = surface
            .get_capabilities(&adapter)
            .formats
            .into_iter()
            .find(|format| format.is_srgb())
            .unwrap_or(TextureFormat::Bgra8UnormSrgb);
        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("WFO framebuffer shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
@group(0) @binding(0) var framebuffer: texture_2d<f32>;
@group(0) @binding(1) var framebuffer_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var uvs = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(2.0, 1.0),
        vec2<f32>(0.0, -1.0),
    );
    var out: VertexOutput;
    out.position = vec4<f32>(positions[index], 0.0, 1.0);
    out.uv = uvs[index];
    return out;
}

@fragment
fn fs(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(framebuffer, framebuffer_sampler, in.uv);
    return vec4<f32>(
        srgb_to_linear(color.r),
        srgb_to_linear(color.g),
        srgb_to_linear(color.b),
        color.a
    );
}

fn srgb_to_linear(color: f32) -> f32 {
    if color <= 0.04045 {
        return color / 12.92;
    }
    return pow((color + 0.055) / 1.055, 2.4);
}
"#
                .into(),
            ),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("WFO framebuffer bind layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("WFO pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("WFO framebuffer pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let scene_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("WFO scene shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

@vertex
fn vs(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(input.position, 1.0);
    out.color = input.color;
    return out;
}

@fragment
fn fs(input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(
        srgb_to_linear(input.color.r),
        srgb_to_linear(input.color.g),
        srgb_to_linear(input.color.b),
        1.0
    );
}

fn srgb_to_linear(color: f32) -> f32 {
    if color <= 0.04045 {
        return color / 12.92;
    }
    return pow((color + 0.055) / 1.055, 2.4);
}
"#
                .into(),
            ),
        });
        let scene_vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<[f32; 6]>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &SCENE_VERTEX_ATTRIBUTES,
        };
        let scene_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("WFO 3D scene pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &scene_shader,
                entry_point: Some("vs"),
                buffers: &[scene_vertex_layout],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &scene_shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let texture = create_framebuffer_texture(&device, width as usize, height as usize);
        let texture_view = texture.create_view(&TextureViewDescriptor::default());
        let ui_texture = create_framebuffer_texture(&device, width as usize, height as usize);
        let ui_texture_view = ui_texture.create_view(&TextureViewDescriptor::default());
        let depth_texture = create_depth_texture(&device, width, height);
        let depth_view = depth_texture.create_view(&TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("WFO nearest sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("WFO background bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let ui_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("WFO UI overlay bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&ui_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Self {
            surface,
            device,
            queue,
            config,
            texture,
            texture_view,
            bind_group_layout,
            bind_group,
            ui_texture,
            ui_texture_view,
            ui_bind_group,
            pipeline,
            scene_pipeline,
            depth_texture,
            depth_view,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if self.config.width == width && self.config.height == height {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.depth_texture = create_depth_texture(&self.device, width, height);
        self.depth_view = self
            .depth_texture
            .create_view(&TextureViewDescriptor::default());
    }

    pub fn present(
        &mut self,
        scene_vertices: &[SceneVertex],
        background_pixels: &[u32],
        ui_pixels: &[u32],
        width: usize,
        height: usize,
    ) {
        if background_pixels.len() != width * height
            || ui_pixels.len() != width * height
            || scene_vertices.len() % 3 != 0
        {
            return;
        }

        let bytes_per_row = ((width * 4 + 255) / 256) * 256;
        let background_rgba = rgba_bytes(background_pixels, width, height, bytes_per_row, true);
        let ui_rgba = rgba_bytes(ui_pixels, width, height, bytes_per_row, false);

        let packed_vertices = scene_vertices
            .iter()
            .map(|vertex| {
                [
                    vertex.position[0],
                    vertex.position[1],
                    vertex.position[2],
                    vertex.color[0],
                    vertex.color[1],
                    vertex.color[2],
                ]
            })
            .collect::<Vec<_>>();
        let scene_buffer = if packed_vertices.is_empty() {
            None
        } else {
            Some(
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("WFO scene vertices"),
                        contents: bytemuck::cast_slice(&packed_vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
            )
        };

        let texture_size = wgpu::Extent3d {
            width: width as u32,
            height: height as u32,
            depth_or_array_layers: 1,
        };
        if self.texture.size() != texture_size || self.ui_texture.size() != texture_size {
            self.texture = create_framebuffer_texture(&self.device, width, height);
            self.texture_view = self.texture.create_view(&TextureViewDescriptor::default());
            self.ui_texture = create_framebuffer_texture(&self.device, width, height);
            self.ui_texture_view = self
                .ui_texture
                .create_view(&TextureViewDescriptor::default());
            let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("WFO nearest sampler"),
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            });
            self.bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("WFO background bind group"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });
            self.ui_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("WFO UI overlay bind group"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.ui_texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });
        }

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &background_rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row as u32),
                rows_per_image: Some(height as u32),
            },
            texture_size,
        );
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.ui_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &ui_rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row as u32),
                rows_per_image: Some(height as u32),
            },
            texture_size,
        );

        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(_) => return,
        };
        let view = frame.texture.create_view(&TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("WFO frame encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("WFO sky background pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color {
                            r: 0.27,
                            g: 0.39,
                            b: 0.54,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("WFO GPU scene pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(Operations {
                        load: LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.scene_pipeline);
            if let Some(scene_buffer) = &scene_buffer {
                pass.set_vertex_buffer(0, scene_buffer.slice(..));
                pass.draw(0..packed_vertices.len() as u32, 0..1);
            }
        }
        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("WFO HUD overlay pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.ui_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }
}

fn rgba_bytes(pixels: &[u32], width: usize, height: usize, stride: usize, opaque: bool) -> Vec<u8> {
    let mut rgba = vec![0u8; stride * height];
    for y in 0..height {
        let source = y * width;
        let destination = y * stride;
        for x in 0..width {
            let pixel = pixels[source + x];
            let offset = destination + x * 4;
            rgba[offset] = ((pixel >> 16) & 0xff) as u8;
            rgba[offset + 1] = ((pixel >> 8) & 0xff) as u8;
            rgba[offset + 2] = (pixel & 0xff) as u8;
            rgba[offset + 3] = if opaque { 255 } else { (pixel >> 24) as u8 };
        }
    }
    rgba
}

fn create_framebuffer_texture(device: &Device, width: usize, height: usize) -> Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("WFO CPU framebuffer"),
        size: wgpu::Extent3d {
            width: width as u32,
            height: height as u32,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn create_depth_texture(device: &Device, width: u32, height: u32) -> Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("WFO scene depth"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Depth32Float,
        usage: TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    })
}
