use std::sync::{ mpsc::channel, Arc };
use portable_pty::{ CommandBuilder, MasterPty, PtySize };
use rt::{
    Attrs,
    Buffer,
    Cache,
    Color,
    Family,
    FontSystem,
    Metrics,
    Resolution,
    Shaping,
    SwashCache,
    TextArea,
    TextAtlas,
    TextBounds,
    TextRenderer,
    Viewport,
};
use wgpu::{
    CommandEncoderDescriptor,
    CompositeAlphaMode,
    DeviceDescriptor,
    Instance,
    InstanceDescriptor,
    LoadOp,
    MultisampleState,
    Operations,
    PresentMode,
    RenderPassColorAttachment,
    RenderPassDescriptor,
    RequestAdapterOptions,
    SurfaceConfiguration,
    TextureFormat,
    TextureUsages,
    TextureViewDescriptor,
};
use winit::{
    dpi::LogicalSize,
    event::{ ElementState, KeyEvent, WindowEvent },
    event_loop::EventLoop,
    keyboard::Key,
    window::Window,
};

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.run_app(&mut (Application { window_state: None, text: None })).unwrap();
}

struct WindowState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: SurfaceConfiguration,

    font_system: FontSystem,
    swash_cache: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    text_renderer: TextRenderer,
    text_buffer: Buffer,

    // Make sure that the winit window is last in the struct so that
    // it is dropped after the wgpu surface is dropped, otherwise the
    // program may crash when closed. This is probably a bug in wgpu.
    window: Arc<Window>,
    pty_master: Box<dyn MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send>,
    output_rx: std::sync::mpsc::Receiver<String>,
    pty_writer: Box<dyn std::io::Write + Send>,
}

impl WindowState {
    async fn new(window: Arc<Window>) -> Self {
        let physical_size = window.inner_size();
        let scale_factor = window.scale_factor();

        // Set up surface
        let instance = Instance::new(InstanceDescriptor::default());
        let adapter = instance.request_adapter(&RequestAdapterOptions::default()).await.unwrap();
        let (device, queue) = adapter
            .request_device(&DeviceDescriptor::default(), None).await
            .unwrap();

        let surface = instance.create_surface(window.clone()).expect("Create surface");
        let swapchain_format = TextureFormat::Bgra8UnormSrgb;
        let surface_config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: physical_size.width,
            height: physical_size.height,
            present_mode: PresentMode::Fifo,
            alpha_mode: CompositeAlphaMode::Opaque,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Set up text renderer
        let mut font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let cache = Cache::new(&device);
        let viewport = Viewport::new(&device, &cache);
        let mut atlas = TextAtlas::new(&device, &queue, &cache, swapchain_format);
        let text_renderer = TextRenderer::new(
            &mut atlas,
            &device,
            MultisampleState::default(),
            None
        );
        let mut text_buffer = Buffer::new(&mut font_system, Metrics::new(30.0, 42.0));

        let physical_width = ((physical_size.width as f64) * scale_factor) as f32;
        let physical_height = ((physical_size.height as f64) * scale_factor) as f32;

        text_buffer.set_size(&mut font_system, Some(physical_width), Some(physical_height));
        text_buffer.shape_until_scroll(&mut font_system, false);

        let pty_system = portable_pty::native_pty_system();
        let mut pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();

        let cmd = CommandBuilder::new("whoami");
        let mut child = pair.slave.spawn_command(cmd).unwrap();
        drop(pair.slave);

        // Setting up the channel and spawning a thread to read the output
        let (tx, rx) = std::sync::mpsc::channel();
        let mut reader = pair.master.try_clone_reader().unwrap();
        std::thread::spawn(move || {
            let mut s = String::new();
            reader.read_to_string(&mut s).unwrap();
            tx.send(s).unwrap();
        });
        let pty_writer = pair.master.take_writer().unwrap();

        Self {
            device,
            queue,
            surface,
            surface_config,
            font_system,
            swash_cache,
            viewport,
            atlas,
            text_renderer,
            text_buffer,
            window,
            pty_master: pair.master,
            child,
            output_rx: rx,
            pty_writer,
        }
    }
}

struct Application {
    window_state: Option<WindowState>,
    text: Option<String>,
}

impl winit::application::ApplicationHandler for Application {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.window_state.is_some() {
            return;
        }

        // Set up window
        let (width, height) = (800, 600);
        let window_attributes = Window::default_attributes()
            .with_inner_size(LogicalSize::new(width as f64, height as f64))
            .with_title("rt");
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        self.window_state = Some(pollster::block_on(WindowState::new(window)));

        self.text = Some(String::new());
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent
    ) {
        let Some(state) = &mut self.window_state else {
            return;
        };

        let WindowState {
            window,
            device,
            queue,
            surface,
            surface_config,
            font_system,
            swash_cache,
            viewport,
            atlas,
            text_renderer,
            text_buffer,
            pty_master,
            output_rx,
            pty_writer,
            ..
        } = state;

        match event {
            WindowEvent::RedrawRequested => {
                if let Ok(output) = output_rx.try_recv() {
                    self.text.as_mut().unwrap().push_str(&output);
                }

                text_buffer.set_text(
                    font_system,
                    &self.text.as_ref().unwrap().as_str(),
                    Attrs::new().family(Family::Monospace),
                    Shaping::Advanced
                );
                viewport.update(&queue, Resolution {
                    width: surface_config.width,
                    height: surface_config.height,
                });

                text_renderer
                    .prepare(
                        device,
                        queue,
                        font_system,
                        atlas,
                        viewport,
                        [
                            TextArea {
                                buffer: text_buffer,
                                left: 0.0,
                                top: 0.0,
                                scale: 1.0,
                                bounds: TextBounds::default(),
                                default_color: Color::rgb(255, 255, 255),
                            },
                        ],
                        swash_cache
                    )
                    .unwrap();

                let frame = surface.get_current_texture().unwrap();
                let view = frame.texture.create_view(&TextureViewDescriptor::default());
                let mut encoder = device.create_command_encoder(
                    &(CommandEncoderDescriptor { label: None })
                );
                {
                    let mut pass = encoder.begin_render_pass(
                        &(RenderPassDescriptor {
                            label: None,
                            color_attachments: &[
                                Some(RenderPassColorAttachment {
                                    view: &view,
                                    resolve_target: None,
                                    ops: Operations {
                                        load: LoadOp::Clear(wgpu::Color::BLACK),
                                        store: wgpu::StoreOp::Store,
                                    },
                                }),
                            ],
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        })
                    );

                    text_renderer.render(&atlas, &viewport, &mut pass).unwrap();
                }

                queue.submit(Some(encoder.finish()));
                frame.present();

                atlas.trim();
            }
            WindowEvent::KeyboardInput {
                event: KeyEvent { logical_key: key, state: ElementState::Pressed, .. },
                ..
            } =>
                match key.as_ref() {
                    Key::Character(character) => {
                        // self.text.as_mut().unwrap().push_str(character);
                        pty_writer.write_all(character.clone().as_bytes()).unwrap();
                        window.request_redraw();
                    }
                    _ => (),
                }
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                surface_config.width = size.width;
                surface_config.height = size.height;
                surface.configure(&device, &surface_config);

                pty_master
                    .resize(PtySize {
                        rows: (size.height / 16) as u16, // Adjust based on your font size
                        cols: (size.width / 8) as u16,
                        pixel_width: size.width as u16,
                        pixel_height: size.height as u16,
                    })
                    .unwrap();

                window.request_redraw();
            }

            _ => {}
        }
    }
}
