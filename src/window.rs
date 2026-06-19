use std::sync::Arc;

use raw_window_handle::{DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use wgpu::util::DeviceExt;

use crate::paint::{DisplayCommand, DisplayList};

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

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 4],
}
unsafe impl bytemuck::Pod for Vertex {}
unsafe impl bytemuck::Zeroable for Vertex {}

struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
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

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tsfire shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(SHADER)),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tsfire pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
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
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        Ok(Self {
            device,
            queue,
            surface,
            config,
            pipeline,
        })
    }

    fn render(&self, list: &DisplayList) {
        let current = self.surface.get_current_texture();
        let frame = match current {
            wgpu::CurrentSurfaceTexture::Success(st)
            | wgpu::CurrentSurfaceTexture::Suboptimal(st) => st,
            _ => return,
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let vertices = build_vertices(list, self.config.width as f32, self.config.height as f32);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("tsfire encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tsfire pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
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

            pass.set_pipeline(&self.pipeline);
            if !vertices.is_empty() {
                let buffer = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: bytemuck::cast_slice(&vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..vertices.len() as u32, 0..1);
            }
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width > 0 && size.height > 0 {
            self.config.width = size.width;
            self.config.height = size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }
}

fn build_vertices(list: &DisplayList, win_w: f32, win_h: f32) -> Vec<Vertex> {
    let cw = win_w.max(list.content_size.width).max(1.0);
    let ch = win_h.max(list.content_size.height).max(1.0);
    let mut out = Vec::new();

    for cmd in &list.items {
        match *cmd {
            DisplayCommand::FillRect(rect, color) => {
                push_rect(&mut out, &rect, 1.0, color, cw, ch);
            }
            DisplayCommand::FillGradient(rect, ref grad) => {
                push_rect(&mut out, &rect, 1.0, grad.from, cw, ch);
            }
            DisplayCommand::Border(rect, ref sides) => {
                let _w = sides.iter().map(|s| s.width).sum::<f32>().max(1.0);
                let bw = sides[0].width.max(1.0);
                let bc = sides[0].color;
                // top
                push_rect(
                    &mut out,
                    &crate::layout::Rect {
                        x: rect.x,
                        y: rect.y,
                        width: rect.width,
                        height: bw,
                    },
                    1.0,
                    bc,
                    cw,
                    ch,
                );
                // bottom
                push_rect(
                    &mut out,
                    &crate::layout::Rect {
                        x: rect.x,
                        y: rect.y + rect.height - bw,
                        width: rect.width,
                        height: bw,
                    },
                    1.0,
                    bc,
                    cw,
                    ch,
                );
                // left
                push_rect(
                    &mut out,
                    &crate::layout::Rect {
                        x: rect.x,
                        y: rect.y,
                        width: bw,
                        height: rect.height,
                    },
                    1.0,
                    bc,
                    cw,
                    ch,
                );
                // right
                push_rect(
                    &mut out,
                    &crate::layout::Rect {
                        x: rect.x + rect.width - bw,
                        y: rect.y,
                        width: bw,
                        height: rect.height,
                    },
                    1.0,
                    bc,
                    cw,
                    ch,
                );
            }
            DisplayCommand::TextRun(rect, color, ..) => {
                // approximate text as semi-transparent fill with text color
                let mut c = color;
                c.3 = 40;
                push_rect(&mut out, &rect, 1.0, c, cw, ch);
            }
            _ => {}
        }
    }

    if out.is_empty() {
        push_vertex(
            &mut out,
            [-1.0, -1.0, 0.94, 0.94, 0.94, 1.0],
            [3.0, -1.0, 0.94, 0.94, 0.94, 1.0],
            [-1.0, 3.0, 0.94, 0.94, 0.94, 1.0],
        );
    }

    out
}

fn push_rect(
    out: &mut Vec<Vertex>,
    rect: &crate::layout::Rect,
    alpha: f32,
    color: crate::style::Color,
    cw: f32,
    ch: f32,
) {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return;
    }

    let x0 = (rect.x / cw) * 2.0 - 1.0;
    let x1 = ((rect.x + rect.width) / cw) * 2.0 - 1.0;
    let top = 1.0 - (rect.y / ch) * 2.0;
    let bottom = 1.0 - ((rect.y + rect.height) / ch) * 2.0;

    let r = color.0 as f32 / 255.0;
    let g = color.1 as f32 / 255.0;
    let b = color.2 as f32 / 255.0;
    let a = color.3 as f32 / 255.0 * alpha;

    push_vertex(
        out,
        [x0, top, r, g, b, a],
        [x1, top, r, g, b, a],
        [x0, bottom, r, g, b, a],
    );
    push_vertex(
        out,
        [x1, top, r, g, b, a],
        [x1, bottom, r, g, b, a],
        [x0, bottom, r, g, b, a],
    );
}

#[inline]
fn push_vertex(out: &mut Vec<Vertex>, a: [f32; 6], b: [f32; 6], c: [f32; 6]) {
    out.push(Vertex {
        position: [a[0], a[1]],
        color: [a[2], a[3], a[4], a[5]],
    });
    out.push(Vertex {
        position: [b[0], b[1]],
        color: [b[2], b[3], b[4], b[5]],
    });
    out.push(Vertex {
        position: [c[0], c[1]],
        color: [c[2], c[3], c[4], c[5]],
    });
}

const SHADER: &str = r"
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
};
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    return VertexOutput(vec4(input.position, 0.0, 1.0), input.color);
}

@fragment
fn fs_main(@location(0) color: vec4<f32>) -> @location(0) vec4<f32> {
    return color;
}
";

struct App {
    state: Option<WindowState>,
    list: DisplayList,
}

struct WindowState {
    window: Arc<Window>,
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
        self.state = Some(WindowState { window: win, renderer });
        self.state.as_ref().unwrap().window.request_redraw();
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        let state = match self.state.as_mut() {
            Some(s) => s,
            None => return,
        };
        match event {
            WindowEvent::CloseRequested => _event_loop.exit(),
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
                    _event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                state.renderer.resize(size);
            }
            WindowEvent::RedrawRequested => {
                state.renderer.render(&self.list)
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}

pub fn run(list: DisplayList) -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    let mut app = App {
        state: None,
        list,
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}
