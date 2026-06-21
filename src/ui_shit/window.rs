use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::event_loop::ControlFlow;

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use crate::parsing::RenderNode;
use crate::ui_shit::display_renderer;
use crate::ui_shit::layout::{self, LayoutEngine};
use crate::ui_shit::paint;

struct WrappedWindow(Arc<Window>);
unsafe impl Send for WrappedWindow {}
unsafe impl Sync for WrappedWindow {}
impl HasWindowHandle for WrappedWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.0.window_handle()
    }
}
impl HasDisplayHandle for WrappedWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.0.display_handle()
    }
}

struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    display_renderer: Option<display_renderer::DisplayRenderer>,
    limits: wgpu::Limits,
}

impl Renderer {
    async fn new(window: Arc<Window>) -> Result<Self, Box<dyn std::error::Error>> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(WrappedWindow(window.clone()))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| format!("adapter request: {e}"))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("tsfire device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
                ..Default::default()
            })
            .await?;
        let caps = surface.get_capabilities(&adapter);
        let limits = device.limits();
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let alpha_mode = caps.alpha_modes[0];
        let size = window.inner_size();
        let size = PhysicalSize::new(
            size.width.min(limits.max_texture_dimension_2d),
            size.height.min(limits.max_texture_dimension_2d),
        );
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
        };
        surface.configure(&device, &config);

        Ok(Self {
            device,
            queue,
            surface,
            config,
            display_renderer: None,
            limits,
        })
    }

    fn ensure_renderer(&mut self) {
        if self.display_renderer.is_none() {
            self.display_renderer = Some(display_renderer::DisplayRenderer::new(
                self.device.clone(),
                self.queue.clone(),
                self.config.width,
                self.config.height,
                self.config.format,
            ));
        }
    }

    fn render(&mut self, list: Option<&paint::DisplayList>) {
        let current = self.surface.get_current_texture();
        let frame = match current {
            wgpu::CurrentSurfaceTexture::Success(st)
            | wgpu::CurrentSurfaceTexture::Suboptimal(st) => st,
            _ => return,
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.ensure_renderer();

        if let Some(list) = list {
            if let Some(renderer) = &mut self.display_renderer {
                let cmd = renderer.render(&view, list);
                self.queue.submit(Some(cmd));
            }
        } else {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("tsfire encoder"),
                });
            {
                let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("tsfire clear"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.0,
                                g: 0.0,
                                b: 0.0,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
            }
            self.queue.submit(Some(encoder.finish()));
        }

        frame.present();
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width > 0 && size.height > 0 {
            let w = size.width.min(self.limits.max_texture_dimension_2d);
            let h = size.height.min(self.limits.max_texture_dimension_2d);
            self.config.width = w;
            self.config.height = h;
            self.surface.configure(&self.device, &self.config);
            if let Some(renderer) = &mut self.display_renderer {
                renderer.resize(w, h);
            }
        }
    }
}

struct App {
    state: Option<WindowState>,
    display_list: Option<paint::DisplayList>,
    render_tree: Option<RenderNode>,
    decoded_images: Vec<paint::DecodedImage>,
    image_map: HashMap<String, u32>,
}

impl App {
    fn rebuild_display_list(&mut self, width: f32, height: f32) {
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        if let Some(tree) = &self.render_tree {
            let engine = layout::BlockLayout;
            let boxes = engine.layout(tree, layout::Size { width, height });
            let dl =
                paint::build_display_list(&boxes, self.decoded_images.clone(), &self.image_map);
            self.display_list = Some(dl);
        } else {
            self.display_list = None;
        }
    }
}

struct WindowState {
    _window: Arc<Window>,
    renderer: Renderer,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let win = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("tsfire")
                    .with_inner_size(PhysicalSize::new(1024, 768)),
            )
            .unwrap();
        let win = Arc::new(win);
        let renderer = pollster::block_on(Renderer::new(win.clone())).unwrap();

        let size = win.inner_size();
        self.rebuild_display_list(size.width as f32, size.height as f32);

        self.state = Some(WindowState {
            _window: win,
            renderer,
        });
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now()));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let state = match self.state.as_mut() {
            Some(s) => s,
            None => return,
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        logical_key: key,
                        state: winit::event::ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                if matches!(key, Key::Named(NamedKey::Escape)) {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    state.renderer.resize(size);
                    self.rebuild_display_list(size.width as f32, size.height as f32);
                }
            }
            WindowEvent::RedrawRequested => {
                state.renderer.render(self.display_list.as_ref());
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_secs_f64(1.0 / 60.0),
            ));
        }
    }
}

pub fn run(
    tree: Option<RenderNode>,
    decoded_images: Vec<paint::DecodedImage>,
    image_map: HashMap<String, u32>,
) -> Result<(), Box<dyn std::error::Error>> {
    let event_zaloop = EventLoop::new()?;
    let mut app = App {
        state: None,
        display_list: None,
        render_tree: tree,
        decoded_images,
        image_map,
    };
    event_zaloop.run_app(&mut app)?;
    Ok(())
}
