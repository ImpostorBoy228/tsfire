use std::sync::Arc;

use raw_window_handle::{DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use crate::ui_shit::display_renderer;
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
}

impl Renderer {
    async fn new(window: Arc<Window>) -> Result<Self, Box<dyn std::error::Error>> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(WrappedWindow(window.clone()))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| format!("adapter request: {e}"))?;
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("tsfire device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    memory_hints: wgpu::MemoryHints::Performance,
                    trace: wgpu::Trace::Off,
                    ..Default::default()
                },
            )
            .await?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let alpha_mode = caps.alpha_modes[0];
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        Ok(Self { device, queue, surface, config, display_renderer: None })
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
                                r: 0.1,
                                g: 0.05,
                                b: 0.06,
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
            self.config.width = size.width;
            self.config.height = size.height;
            self.surface.configure(&self.device, &self.config);
            if let Some(renderer) = &mut self.display_renderer {
                renderer.resize(size.width, size.height);
            }
        }
    }
}

struct App {
    state: Option<WindowState>,
    display_list: Option<paint::DisplayList>,
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
        self.state = Some(WindowState { _window: win, renderer });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
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
                state.renderer.resize(size);
            }
            WindowEvent::RedrawRequested => {
                state.renderer.render(self.display_list.as_ref());
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state._window.request_redraw();
        }
    }
}

pub fn run(list: paint::DisplayList) -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    let mut app = App {
        state: None,
        display_list: Some(list),
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}
