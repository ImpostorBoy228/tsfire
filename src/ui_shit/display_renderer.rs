use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::parsing;
use crate::ui_shit::{layout, paint, paint::TextRange};

// --- Vertex types ---

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct SolidVertex {
    position: [f32; 2],
    color: [f32; 4],
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct TexturedVertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct GradientVert {
    pos_t: [f32; 4],
    from_rgba: [f32; 4],
    to_rgba: [f32; 4],
}

// --- Glyph Atlas ---

#[derive(Debug, Clone, Copy)]
struct GlyphUV {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[derive(Debug)]
struct AtlasAlloc {
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
}

impl AtlasAlloc {
    fn new() -> Self {
        Self {
            cursor_x: 1,
            cursor_y: 1,
            row_height: 0,
        }
    }

    fn place(&mut self, w: u32, h: u32, atlas_size: u32) -> Option<(u32, u32)> {
        let gap = 1u32;
        let ww = w + gap;
        let hh = h + gap;
        if self.cursor_x + ww > atlas_size {
            self.cursor_x = 1;
            self.cursor_y += self.row_height;
            self.row_height = 0;
        }
        if self.cursor_y + hh > atlas_size {
            return None;
        }
        let x = self.cursor_x;
        let y = self.cursor_y;
        self.cursor_x += ww;
        self.row_height = self.row_height.max(hh);
        Some((x, y))
    }
}

#[derive(Debug)]
struct GlyphAtlas {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    cache: HashMap<u32, GlyphUV>,
    atlas_size: u32,
    alloc: AtlasAlloc,
}

impl GlyphAtlas {
    fn new(device: &wgpu::Device, atlas_size: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph_atlas"),
            size: wgpu::Extent3d {
                width: atlas_size,
                height: atlas_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            texture,
            view,
            cache: HashMap::new(),
            atlas_size,
            alloc: AtlasAlloc::new(),
        }
    }

    fn get_glyph(&self, codepoint: u32) -> Option<&GlyphUV> {
        self.cache.get(&codepoint)
    }

    fn upload_glyph(
        &mut self,
        queue: &wgpu::Queue,
        codepoint: u32,
        bitmap: &[u8],
        bm_width: i32,
        bm_rows: i32,
        bm_pitch: i32,
        bm_offset: i32,
    ) -> Option<GlyphUV> {
        let w = bm_width as u32;
        let h = bm_rows as u32;
        let pitch = bm_pitch as u32;

        let (atlas_x, atlas_y) = self.alloc.place(w, h, self.atlas_size)?;

        let src_start = bm_offset as usize;
        let src_len = (pitch * h) as usize;
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: atlas_x,
                    y: atlas_y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &bitmap[src_start..src_start + src_len],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(pitch),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );

        let uv = GlyphUV {
            x: atlas_x as f32 / self.atlas_size as f32,
            y: atlas_y as f32 / self.atlas_size as f32,
            width: w as f32 / self.atlas_size as f32,
            height: h as f32 / self.atlas_size as f32,
        };
        self.cache.insert(codepoint, uv);
        Some(uv)
    }
}

// --- Font cache ---

struct FontCache {
    data: Vec<u8>,
    handles: HashMap<u32, crate::font::FontHandle>,
}

impl FontCache {
    fn load() -> Option<Self> {
        let paths = [
            crate::font::DEFAULT_FONT_PATH,
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/System/Library/Fonts/Helvetica.ttc",
        ];
        for p in &paths {
            if let Ok(data) = std::fs::read(p) {
                return Some(FontCache {
                    data,
                    handles: HashMap::new(),
                });
            }
        }
        None
    }

    fn get(&mut self, px: f32) -> Option<&crate::font::FontHandle> {
        let key = px as u32;
        if !self.handles.contains_key(&key) {
            let b = self.data.clone().into_boxed_slice();
            let handle = crate::font::FontHandle::load(b, px)?;
            self.handles.insert(key, handle);
        }
        self.handles.get(&key)
    }
}

// --- Display Renderer ---

pub struct DisplayRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    width: f32,
    height: f32,

    solid_pipeline: wgpu::RenderPipeline,
    gradient_pipeline: wgpu::RenderPipeline,
    textured_pipeline: wgpu::RenderPipeline,
    textured_bgl: wgpu::BindGroupLayout,
    textured_bind_group: wgpu::BindGroup,

    glyph_atlas: Option<GlyphAtlas>,
    dummy_sampler: wgpu::Sampler,
    font_cache: Option<FontCache>,
    glyph_metrics_cache: crate::cache::GlyphMetricsCache,
    image_textures: Vec<wgpu::Texture>,
    image_views: Vec<wgpu::TextureView>,
    image_bind_groups: Vec<wgpu::BindGroup>,
    image_vb: wgpu::Buffer,
    image_vb_capacity: u64,

    clip_rect: Option<layout::Rect>,
    global_alpha: f32,

    solid_vb: wgpu::Buffer,
    solid_vb_capacity: u64,
    textured_vb: wgpu::Buffer,
    textured_vb_capacity: u64,
    gradient_vb: wgpu::Buffer,
    gradient_vb_capacity: u64,
}

fn vertex_buffer_layout_solid<'a>() -> wgpu::VertexBufferLayout<'a> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<SolidVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 8,
                shader_location: 1,
            },
        ],
    }
}

fn vertex_buffer_layout_gradient<'a>() -> wgpu::VertexBufferLayout<'a> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GradientVert>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 32,
                shader_location: 2,
            },
        ],
    }
}

fn vertex_buffer_layout_textured<'a>() -> wgpu::VertexBufferLayout<'a> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<TexturedVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 8,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 2,
            },
        ],
    }
}

impl DisplayRenderer {
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tsfire_shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "shaders/pipeline.wgsl"
            ))),
        });

        let solid_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("solid_bgl"),
            entries: &[],
        });

        let textured_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("textured_bgl"),
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

        // Dummy 1x1 white texture for initial bind group
        let dummy_texture = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("dummy_texture"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            Default::default(),
            &[255u8, 255, 255, 255],
        );
        let dummy_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("dummy_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let dummy_tex_view = dummy_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let dummy_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("dummy_bg"),
            layout: &textured_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&dummy_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&dummy_tex_view),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("solid_pipeline_layout"),
            bind_group_layouts: &[Some(&solid_bgl)],
            immediate_size: 0,
        });

        let solid_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("solid_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_solid"),
                buffers: &[vertex_buffer_layout_solid()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_solid"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
                unclipped_depth: false,
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

        let gradient_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gradient_bgl"),
            entries: &[],
        });

        let gradient_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("gradient_pipeline_layout"),
                bind_group_layouts: &[Some(&gradient_bgl)],
                immediate_size: 0,
            });

        let gradient_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gradient_pipeline"),
            layout: Some(&gradient_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_gradient"),
                buffers: &[vertex_buffer_layout_gradient()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_gradient"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
                unclipped_depth: false,
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

        let textured_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("textured_pipeline_layout"),
                bind_group_layouts: &[Some(&textured_bgl)],
                immediate_size: 0,
            });

        let textured_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("textured_pipeline"),
            layout: Some(&textured_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_textured"),
                buffers: &[vertex_buffer_layout_textured()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_textured"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
                unclipped_depth: false,
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

        let font_cache = FontCache::load();
        let glyph_metrics_cache = crate::cache::GlyphMetricsCache::new();

        let initial_vb_size: u64 = 64 * 1024;
        let solid_vb = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("solid_vb"),
            size: initial_vb_size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let textured_vb = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("textured_vb"),
            size: initial_vb_size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let gradient_vb = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gradient_vb"),
            size: initial_vb_size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let image_vb = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("image_vb"),
            size: initial_vb_size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            device,
            queue,
            width: width as f32,
            height: height as f32,
            solid_pipeline,
            gradient_pipeline,
            textured_pipeline,
            textured_bgl,
            textured_bind_group: dummy_bind_group,
            glyph_atlas: None,
            dummy_sampler,
            font_cache,
            glyph_metrics_cache,
            image_textures: vec![],
            image_views: vec![],
            image_bind_groups: vec![],
            image_vb,
            image_vb_capacity: initial_vb_size,
            clip_rect: None,
            global_alpha: 1.0,
            solid_vb,
            solid_vb_capacity: initial_vb_size,
            textured_vb,
            textured_vb_capacity: initial_vb_size,
            gradient_vb,
            gradient_vb_capacity: initial_vb_size,
        }
    }

    fn init_glyph_atlas(&mut self) {
        if self.glyph_atlas.is_none() {
            let atlas = GlyphAtlas::new(&self.device, 512);
            self.glyph_atlas = Some(atlas);
        }
        // Recreate bind group with atlas texture
        if let Some(atlas) = &self.glyph_atlas {
            self.textured_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("atlas_bg"),
                layout: &self.textured_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Sampler(&self.dummy_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&atlas.view),
                    },
                ],
            });
        }
    }

    pub fn render(
        &mut self,
        target: &wgpu::TextureView,
        list: &paint::DisplayList,
    ) -> wgpu::CommandBuffer {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("tsfire encoder"),
            });

        let mut solid_vertices: Vec<SolidVertex> = Vec::new();
        let mut gradient_vertices: Vec<GradientVert> = Vec::new();
        let mut textured_vertices: Vec<TexturedVertex> = Vec::new();
        let mut image_vertices: Vec<TexturedVertex> = Vec::new();
        let mut image_draws: Vec<(u32, u32)> = Vec::new(); // (img_idx, num_vertices)

        for cmd in &list.items {
            match cmd {
                paint::DisplayCommand::FillRect(rect, color) => {
                    self.add_fill_rect(rect, color, &mut solid_vertices);
                }
                paint::DisplayCommand::FillGradient(rect, grad) => {
                    self.add_fill_gradient(rect, grad, &mut gradient_vertices);
                }
                paint::DisplayCommand::Border(rect, sides) => {
                    let r = *rect;
                    // top
                    if sides[0].width > 0.0 && sides[0].style != paint::BorderStyle::None {
                        self.add_fill_rect(
                            &layout::Rect {
                                x: r.x,
                                y: r.y,
                                width: r.width,
                                height: sides[0].width,
                            },
                            &sides[0].color,
                            &mut solid_vertices,
                        );
                    }
                    // right
                    if sides[1].width > 0.0 && sides[1].style != paint::BorderStyle::None {
                        self.add_fill_rect(
                            &layout::Rect {
                                x: r.x + r.width - sides[1].width,
                                y: r.y,
                                width: sides[1].width,
                                height: r.height,
                            },
                            &sides[1].color,
                            &mut solid_vertices,
                        );
                    }
                    // bottom
                    if sides[2].width > 0.0 && sides[2].style != paint::BorderStyle::None {
                        self.add_fill_rect(
                            &layout::Rect {
                                x: r.x,
                                y: r.y + r.height - sides[2].width,
                                width: r.width,
                                height: sides[2].width,
                            },
                            &sides[2].color,
                            &mut solid_vertices,
                        );
                    }
                    // left
                    if sides[3].width > 0.0 && sides[3].style != paint::BorderStyle::None {
                        self.add_fill_rect(
                            &layout::Rect {
                                x: r.x,
                                y: r.y,
                                width: sides[3].width,
                                height: r.height,
                            },
                            &sides[3].color,
                            &mut solid_vertices,
                        );
                    }
                }
                paint::DisplayCommand::DrawBoxShadow(rect, color, _ox, _oy, _blur) => {
                    let shadow_color = crate::parsing::Color(
                        color.0,
                        color.1,
                        color.2,
                        (color.3 as f32 * 0.5) as u8,
                    );
                    self.add_fill_rect(rect, &shadow_color, &mut solid_vertices);
                }
                paint::DisplayCommand::TextRun(rect, color, font_size, font_family, range) => {
                    self.add_text_run(
                        list,
                        rect,
                        color,
                        *font_size,
                        *font_family,
                        range,
                        &mut textured_vertices,
                    );
                }
                paint::DisplayCommand::SetClip(rect) => {
                    self.clip_rect = Some(*rect);
                }
                paint::DisplayCommand::PopClip => {
                    self.clip_rect = None;
                }
                paint::DisplayCommand::SetOpacity(val) => {
                    self.global_alpha = *val;
                }
                paint::DisplayCommand::PopOpacity => {
                    self.global_alpha = 1.0;
                }
                paint::DisplayCommand::DrawImage(rect, idx) => {
                    let n = image_vertices.len() as u32;
                    self.add_draw_image(rect, *idx, &mut image_vertices);
                    let added = image_vertices.len() as u32 - n;
                    if added > 0 {
                        image_draws.push((*idx, added));
                    }
                }
            }
        }

        // write vertex data to dynamic gpu buffers
        if !solid_vertices.is_empty() {
            let needed = (solid_vertices.len() * std::mem::size_of::<SolidVertex>()) as u64;
            if needed > self.solid_vb_capacity {
                self.solid_vb_capacity = (needed as f64 * 1.5) as u64;
                self.solid_vb = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("solid_vb"),
                    size: self.solid_vb_capacity,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }
            self.queue
                .write_buffer(&self.solid_vb, 0, bytemuck::cast_slice(&solid_vertices));
        }
        if !textured_vertices.is_empty() {
            let needed = (textured_vertices.len() * std::mem::size_of::<TexturedVertex>()) as u64;
            if needed > self.textured_vb_capacity {
                self.textured_vb_capacity = (needed as f64 * 1.5) as u64;
                self.textured_vb = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("textured_vb"),
                    size: self.textured_vb_capacity,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }
            self.queue.write_buffer(
                &self.textured_vb,
                0,
                bytemuck::cast_slice(&textured_vertices),
            );
        }
        if !gradient_vertices.is_empty() {
            let needed = (gradient_vertices.len() * std::mem::size_of::<GradientVert>()) as u64;
            if needed > self.gradient_vb_capacity {
                self.gradient_vb_capacity = (needed as f64 * 1.5) as u64;
                self.gradient_vb = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("gradient_vb"),
                    size: self.gradient_vb_capacity,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }
            self.queue.write_buffer(
                &self.gradient_vb,
                0,
                bytemuck::cast_slice(&gradient_vertices),
            );
        }
        self.ensure_image_textures(list);
        if !image_vertices.is_empty() {
            let needed = (image_vertices.len() * std::mem::size_of::<TexturedVertex>()) as u64;
            if needed > self.image_vb_capacity {
                self.image_vb_capacity = (needed as f64 * 1.5) as u64;
                self.image_vb = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("image_vb"),
                    size: self.image_vb_capacity,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }
            self.queue.write_buffer(&self.image_vb, 0, bytemuck::cast_slice(&image_vertices));
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tsfire pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
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

            if !solid_vertices.is_empty() {
                pass.set_pipeline(&self.solid_pipeline);
                pass.set_vertex_buffer(0, self.solid_vb.slice(..));
                pass.draw(0..solid_vertices.len() as u32, 0..1);
            }

            if !gradient_vertices.is_empty() {
                pass.set_pipeline(&self.gradient_pipeline);
                pass.set_vertex_buffer(0, self.gradient_vb.slice(..));
                pass.draw(0..gradient_vertices.len() as u32, 0..1);
            }

            if !textured_vertices.is_empty() {
                pass.set_pipeline(&self.textured_pipeline);
                pass.set_bind_group(0, &self.textured_bind_group, &[]);
                pass.set_vertex_buffer(0, self.textured_vb.slice(..));
                pass.draw(0..textured_vertices.len() as u32, 0..1);
            }

            if !image_draws.is_empty() {
                pass.set_pipeline(&self.textured_pipeline);
                let mut vb_offset: u64 = 0;
                for &(img_idx, vcount) in &image_draws {
                    if let Some(bg) = self.image_bind_groups.get(img_idx as usize) {
                        let size = (vcount as u64) * std::mem::size_of::<TexturedVertex>() as u64;
                        pass.set_bind_group(0, bg, &[]);
                        pass.set_vertex_buffer(0, self.image_vb.slice(vb_offset..vb_offset + size));
                        pass.draw(0..vcount, 0..1);
                        vb_offset += size;
                    }
                }
            }
        }

        encoder.finish()
    }

    fn add_fill_rect(
        &self,
        rect: &layout::Rect,
        color: &parsing::Color,
        out: &mut Vec<SolidVertex>,
    ) {
        // clip to current clip_rect
        let r = match self.clip_rect {
            Some(cr) => {
                let x = rect.x.max(cr.x);
                let y = rect.y.max(cr.y);
                let right = (rect.x + rect.width).min(cr.x + cr.width);
                let bottom = (rect.y + rect.height).min(cr.y + cr.height);
                if x >= right || y >= bottom {
                    return;
                }
                layout::Rect {
                    x,
                    y,
                    width: right - x,
                    height: bottom - y,
                }
            }
            None => *rect,
        };

        let ndc_left = -1.0 + 2.0 * r.x / self.width;
        let ndc_right = -1.0 + 2.0 * (r.x + r.width) / self.width;
        let ndc_top = 1.0 - 2.0 * r.y / self.height;
        let ndc_bottom = 1.0 - 2.0 * (r.y + r.height) / self.height;

        let c = [
            color.0 as f32 / 255.0,
            color.1 as f32 / 255.0,
            color.2 as f32 / 255.0,
            color.3 as f32 / 255.0 * self.global_alpha,
        ];

        out.push(SolidVertex {
            position: [ndc_left, ndc_top],
            color: c,
        });
        out.push(SolidVertex {
            position: [ndc_left, ndc_bottom],
            color: c,
        });
        out.push(SolidVertex {
            position: [ndc_right, ndc_bottom],
            color: c,
        });
        out.push(SolidVertex {
            position: [ndc_right, ndc_bottom],
            color: c,
        });
        out.push(SolidVertex {
            position: [ndc_right, ndc_top],
            color: c,
        });
        out.push(SolidVertex {
            position: [ndc_left, ndc_top],
            color: c,
        });
    }

    fn add_fill_gradient(
        &self,
        rect: &layout::Rect,
        gradient: &paint::Gradient,
        out: &mut Vec<GradientVert>,
    ) {
        let r = match self.clip_rect {
            Some(cr) => {
                let x = rect.x.max(cr.x);
                let y = rect.y.max(cr.y);
                let right = (rect.x + rect.width).min(cr.x + cr.width);
                let bottom = (rect.y + rect.height).min(cr.y + cr.height);
                if x >= right || y >= bottom { return; }
                layout::Rect { x, y, width: right - x, height: bottom - y }
            }
            None => *rect,
        };

        let ndc_left = -1.0 + 2.0 * r.x / self.width;
        let ndc_right = -1.0 + 2.0 * (r.x + r.width) / self.width;
        let ndc_top = 1.0 - 2.0 * r.y / self.height;
        let ndc_bottom = 1.0 - 2.0 * (r.y + r.height) / self.height;

        let from = [
            gradient.from.0 as f32 / 255.0,
            gradient.from.1 as f32 / 255.0,
            gradient.from.2 as f32 / 255.0,
            gradient.from.3 as f32 / 255.0 * self.global_alpha,
        ];
        let to = [
            gradient.to.0 as f32 / 255.0,
            gradient.to.1 as f32 / 255.0,
            gradient.to.2 as f32 / 255.0,
            gradient.to.3 as f32 / 255.0 * self.global_alpha,
        ];

        let (t_top, t_bottom) = if gradient.vertical { (0.0, 1.0) } else { (0.0, 0.0) };

        // triangle 1
        out.push(GradientVert { pos_t: [ndc_left, ndc_top, t_top, 0.0], from_rgba: from, to_rgba: to });
        out.push(GradientVert { pos_t: [ndc_left, ndc_bottom, t_bottom, 0.0], from_rgba: from, to_rgba: to });
        out.push(GradientVert { pos_t: [ndc_right, ndc_bottom, t_bottom, 0.0], from_rgba: from, to_rgba: to });
        // triangle 2
        out.push(GradientVert { pos_t: [ndc_right, ndc_bottom, t_bottom, 0.0], from_rgba: from, to_rgba: to });
        out.push(GradientVert { pos_t: [ndc_right, ndc_top, t_top, 0.0], from_rgba: from, to_rgba: to });
        out.push(GradientVert { pos_t: [ndc_left, ndc_top, t_top, 0.0], from_rgba: from, to_rgba: to });
    }

    fn ensure_image_textures(&mut self, list: &paint::DisplayList) {
        if list.decoded_images.len() <= self.image_textures.len() {
            return;
        }
        for i in self.image_textures.len()..list.decoded_images.len() {
            let img = &list.decoded_images[i];
            let extent = wgpu::Extent3d { width: img.width, height: img.height, depth_or_array_layers: 1 };
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("img_{i}")),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &img.rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(img.width * 4),
                    rows_per_image: Some(img.height),
                },
                extent,
            );
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("img_bg_{i}")),
                layout: &self.textured_bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Sampler(&self.dummy_sampler) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                ],
            });
            self.image_textures.push(texture);
            self.image_views.push(view);
            self.image_bind_groups.push(bind_group);
        }
    }

    fn add_draw_image(
        &self,
        rect: &layout::Rect,
        _img_idx: u32,
        out: &mut Vec<TexturedVertex>,
    ) {
        let r = match self.clip_rect {
            Some(cr) => {
                let x = rect.x.max(cr.x);
                let y = rect.y.max(cr.y);
                let right = (rect.x + rect.width).min(cr.x + cr.width);
                let bottom = (rect.y + rect.height).min(cr.y + cr.height);
                if x >= right || y >= bottom { return; }
                layout::Rect { x, y, width: right - x, height: bottom - y }
            }
            None => *rect,
        };

        let ndc_left = -1.0 + 2.0 * r.x / self.width;
        let ndc_right = -1.0 + 2.0 * (r.x + r.width) / self.width;
        let ndc_top = 1.0 - 2.0 * r.y / self.height;
        let ndc_bottom = 1.0 - 2.0 * (r.y + r.height) / self.height;

        let white = [1.0, 1.0, 1.0, 1.0 * self.global_alpha];
        // full-texture UV
        out.push(TexturedVertex { position: [ndc_left, ndc_top],    uv: [0.0, 0.0], color: white });
        out.push(TexturedVertex { position: [ndc_left, ndc_bottom], uv: [0.0, 1.0], color: white });
        out.push(TexturedVertex { position: [ndc_right, ndc_bottom], uv: [1.0, 1.0], color: white });
        out.push(TexturedVertex { position: [ndc_right, ndc_bottom], uv: [1.0, 1.0], color: white });
        out.push(TexturedVertex { position: [ndc_right, ndc_top],    uv: [1.0, 0.0], color: white });
        out.push(TexturedVertex { position: [ndc_left, ndc_top],    uv: [0.0, 0.0], color: white });
    }

    fn add_text_run(
        &mut self,
        list: &paint::DisplayList,
        rect: &layout::Rect,
        color: &parsing::Color,
        font_size: f32,
        _font_family: u8,
        range: &TextRange,
        out: &mut Vec<TexturedVertex>,
    ) {
        let font_cache = match &mut self.font_cache {
            Some(f) => f,
            None => return,
        };
        let font = match font_cache.get(font_size) {
            Some(f) => f,
            None => return,
        };

        let end = (range.start + range.len) as usize;
        let text = if end <= list.text_arena.len() {
            &list.text_arena[range.start as usize..end]
        } else {
            return;
        };
        let (glyphs, bitmap) = match self.glyph_metrics_cache.get(font, font_size, text) {
            Some(cached) => (cached.clone(), Vec::new()),
            None => {
                let (g, b) = match font.fill_glyphs(text) {
                    Some(v) => v,
                    None => return,
                };
                self.glyph_metrics_cache.insert(font, font_size, text, g.clone());
                (g, b)
            }
        };

        self.init_glyph_atlas();
        let atlas = match &mut self.glyph_atlas {
            Some(a) => a,
            None => return,
        };

        let cr = color.0 as f32 / 255.0;
        let cg = color.1 as f32 / 255.0;
        let cb = color.2 as f32 / 255.0;
        let ca = color.3 as f32 / 255.0 * self.global_alpha;
        let color_arr = [cr, cg, cb, ca];

        let baseline_y = (rect.y + rect.height * 0.8).round();
        let mut cursor_x = rect.x;

        for info in &glyphs {
            cursor_x += info.ker_x;

            let uv = match atlas.get_glyph(info.codepoint) {
                Some(uv) => *uv,
                None => match atlas.upload_glyph(
                    &self.queue,
                    info.codepoint,
                    &bitmap,
                    info.bm_width,
                    info.bm_rows,
                    info.bm_pitch,
                    info.bm_offset,
                ) {
                    Some(uv) => uv,
                    None => {
                        cursor_x = (cursor_x + info.adv_x).round();
                        continue;
                    }
                },
            };

            let gx = (cursor_x + info.br_x).round();
            let gy = (baseline_y - info.br_y).round();

            let ndc_left = -1.0 + 2.0 * gx / self.width;
            let ndc_right = -1.0 + 2.0 * (gx + info.bm_width as f32) / self.width;
            let ndc_top = 1.0 - 2.0 * gy / self.height;
            let ndc_bottom = 1.0 - 2.0 * (gy + info.bm_rows as f32) / self.height;

            out.push(TexturedVertex {
                position: [ndc_left, ndc_top],
                uv: [uv.x, uv.y],
                color: color_arr,
            });
            out.push(TexturedVertex {
                position: [ndc_left, ndc_bottom],
                uv: [uv.x, uv.y + uv.height],
                color: color_arr,
            });
            out.push(TexturedVertex {
                position: [ndc_right, ndc_bottom],
                uv: [uv.x + uv.width, uv.y + uv.height],
                color: color_arr,
            });
            out.push(TexturedVertex {
                position: [ndc_right, ndc_bottom],
                uv: [uv.x + uv.width, uv.y + uv.height],
                color: color_arr,
            });
            out.push(TexturedVertex {
                position: [ndc_right, ndc_top],
                uv: [uv.x + uv.width, uv.y],
                color: color_arr,
            });
            out.push(TexturedVertex {
                position: [ndc_left, ndc_top],
                uv: [uv.x, uv.y],
                color: color_arr,
            });

            cursor_x = (cursor_x + info.adv_x).round();
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width as f32;
        self.height = height as f32;
    }
}
