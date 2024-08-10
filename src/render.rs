use std::{ cell, iter };
use std::sync::Arc;

use crate::atlas::InnerAtlas;
use crate::config::Config;
use crate::text::Text;
use log::error;
use pollster::FutureExt;
use wgpu::{ Adapter, Device, Instance, PresentMode, Queue, Surface, SurfaceCapabilities };
use winit::dpi::PhysicalSize;
use winit::window::{ Window };
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

pub struct State<'a> {
    surface: Surface<'a>,
    device: Device,
    queue: Queue,
    config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    atlas: InnerAtlas,
    pub user_config: Config,

    size: PhysicalSize<u32>,
    window: Arc<Window>,
}

impl<'a> State<'a> {
    pub fn new(window: Window) -> Self {
        let window_arc = Arc::new(window);
        let size = window_arc.inner_size();
        let instance = Self::create_gpu_instance();
        let surface = instance.create_surface(window_arc.clone()).unwrap();
        let adapter = Self::create_adapter(instance, &surface);
        let (device, queue) = Self::create_device(&adapter);
        let surface_caps = surface.get_capabilities(&adapter);
        let config = Self::create_surface_config(size, surface_caps);
        surface.configure(&device, &config);

        let texture_bind_group_layout = device.create_bind_group_layout(
            &(wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
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
                label: Some("texture_bind_group_layout"),
            })
        );

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let render_pipeline_layout = device.create_pipeline_layout(
            &(wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&texture_bind_group_layout],
                push_constant_ranges: &[],
            })
        );

        let render_pipeline = device.create_render_pipeline(
            &(wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[Vertex::desc()],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[
                        Some(wgpu::ColorTargetState {
                            format: config.format,
                            blend: Some(wgpu::BlendState {
                                color: wgpu::BlendComponent {
                                    src_factor: wgpu::BlendFactor::SrcAlpha,
                                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                    operation: wgpu::BlendOperation::Add,
                                },
                                alpha: wgpu::BlendComponent {
                                    src_factor: wgpu::BlendFactor::One,
                                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                    operation: wgpu::BlendOperation::Add,
                                },
                            }),
                            write_mask: wgpu::ColorWrites::ALL,
                        }),
                    ],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    // Setting this to anything other than Fill requires Features::POLYGON_MODE_LINE
                    // or Features::POLYGON_MODE_POINT
                    polygon_mode: wgpu::PolygonMode::Fill,
                    // Requires Features::DEPTH_CLIP_CONTROL
                    unclipped_depth: false,
                    // Requires Features::CONSERVATIVE_RASTERIZATION
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                // If the pipeline will be used with a multiview render pass, this
                // indicates how many array layers the attachments will have.
                multiview: None,
                // Useful for optimizing shader compilation on Android
                cache: None,
            })
        );

        let sampler = device.create_sampler(
            &(wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            })
        );

        let atlas = InnerAtlas::new(&device);

        let user_config = Config::new(size.width, size.height);

        Self {
            surface,
            device,
            queue,
            config,
            size,
            window: window_arc,
            render_pipeline,
            sampler,
            texture_bind_group_layout,
            atlas,
            user_config,
        }
    }

    fn create_surface_config(
        size: PhysicalSize<u32>,
        capabilities: SurfaceCapabilities
    ) -> wgpu::SurfaceConfiguration {
        let surface_format = capabilities.formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(capabilities.formats[0]);

        wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: PresentMode::AutoNoVsync,
            alpha_mode: capabilities.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        }
    }

    fn create_device(adapter: &Adapter) -> (Device, Queue) {
        adapter
            .request_device(
                &(wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    label: None,
                    ..Default::default()
                }),
                None
            )
            .block_on()
            .unwrap()
    }

    fn create_adapter(instance: Instance, surface: &Surface) -> Adapter {
        instance
            .request_adapter(
                &(wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::default(),
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
            )
            .block_on()
            .unwrap()
    }

    fn create_gpu_instance() -> Instance {
        Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        })
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.size = new_size;

        self.config.width = new_size.width;
        self.config.height = new_size.height;

        self.surface.configure(&self.device, &self.config);

        println!("Resized to {:?} from state!", new_size);
    }

    pub fn render(&mut self, text: &Text) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(
            &(wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            })
        );

        {
            let mut render_pass = encoder.begin_render_pass(
                &(wgpu::RenderPassDescriptor {
                    label: Some("Render Pass"),
                    color_attachments: &[
                        Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.0,
                                    g: 0.0,
                                    b: 0.0,
                                    a: 0.8,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        }),
                    ],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                })
            );

            for (row, col, cell) in text {
                if cell.character == '\0' {
                    continue;
                }
                self.render_char(&mut render_pass, cell.character, row, col);
            }
        }

        self.queue.submit(iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    pub fn render_char(
        &mut self,
        render_pass: &mut wgpu::RenderPass,
        character: char,
        row: usize,
        col: usize
    ) {
        let glyph_details = self.atlas
            .get_or_create_glyph(character, &self.queue, &self.device)
            .unwrap();

        // Calculate texture coordinates based on atlas
        let tex_coords_top_left = [
            (glyph_details.x as f32) / (self.atlas.size as f32),
            (glyph_details.y as f32) / (self.atlas.size as f32),
        ];
        let tex_coords_bottom_right = [
            ((glyph_details.x + glyph_details.width) as f32) / (self.atlas.size as f32),
            ((glyph_details.y + glyph_details.height) as f32) / (self.atlas.size as f32),
        ];

        // Assume a screen size for normalization
        let screen_width: f32 = self.size.width as f32;
        let screen_height: f32 = self.size.height as f32;

        // Calculate the size of each grid cell
        let cell_width = screen_width / (self.user_config.font_size as f32);
        let cell_height = screen_height / (self.user_config.font_size as f32);
        let cell_width = normalize_position(cell_width, screen_width);
        let cell_height = normalize_position(cell_height, screen_height);

        // Calculate the position of the character in screen space
        let x_position = (col as f32) * cell_width - 1.0;
        let y_position = (row as f32) * cell_height - 1.0;
        error!("x_position: {}, cell_height: {}", x_position, cell_height);

        // Calculate the normalized width and height of the glyph
        let glyph_width = (glyph_details.width as f32) / screen_width;
        let glyph_height = (glyph_details.height as f32) / screen_height;

        // Calculate vertex positions based on row and column
        let vertex_buffer = self.device.create_buffer_init(
            &(wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(
                    &[
                        // Bottom-left
                        Vertex {
                            position: [x_position, y_position + glyph_height, 0.0],
                            tex_coords: tex_coords_top_left,
                        },
                        // Top-left
                        Vertex {
                            position: [x_position, y_position, 0.0],
                            tex_coords: [tex_coords_top_left[0], tex_coords_bottom_right[1]],
                        },
                        // Top-right
                        Vertex {
                            position: [x_position + glyph_width, y_position, 0.0],
                            tex_coords: tex_coords_bottom_right,
                        },
                        // Bottom-right
                        Vertex {
                            position: [x_position + glyph_width, y_position + glyph_height, 0.0],
                            tex_coords: [tex_coords_bottom_right[0], tex_coords_top_left[1]],
                        },
                    ]
                ),
                usage: wgpu::BufferUsages::VERTEX,
            })
        );

        // quad
        // 0----3
        // | \  |
        // |  \ |
        // 1----2
        let index_buffer = self.device.create_buffer_init(
            &(wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(
                    &[
                        0u16,
                        1,
                        2, // First triangle
                        0,
                        2,
                        3, // Second triangle
                    ]
                ),
                usage: wgpu::BufferUsages::INDEX,
            })
        );

        let num_indices = 6;

        let diffuse_bind_group = self.device.create_bind_group(
            &(wgpu::BindGroupDescriptor {
                layout: &self.texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.atlas.texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
                label: Some("diffuse_bind_group"),
            })
        );

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &diffuse_bind_group, &[]);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..num_indices, 0, 0..1);
    }

    pub fn window(&self) -> &Window {
        &self.window
    }
}

fn normalize_position(position: f32, screen_size: f32) -> f32 {
    (position / screen_size) * 2.0
}
