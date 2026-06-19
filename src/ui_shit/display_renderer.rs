use std::collections::HashMap;

use wgpu::util::DeviceExt;
use bytemuck::{Pod, Zeroable};

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
struct GlyphAtlas {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    cache: HashMap<u32, GlyphUV>,
    atlas_size: u32,
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
        }
    }

    fn get_glyph(&self, codepoint: u32) -> Option<&GlyphUV> {
        self.cache.get(&codepoint)
    }

    fn add_glyph(&mut self, codepoint: u32, uv: GlyphUV) {
        self.cache.insert(codepoint, uv);
    }
}

// --- Display Renderer ---

pub struct DisplayRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    width: f32,
    height: f32,

    solid_pipeline: wgpu::RenderPipeline,
    textured_pipeline: wgpu::RenderPipeline,
    textured_bind_group: wgpu::BindGroup,

    glyph_atlas: Option<GlyphAtlas>,
    dummy_texture: wgpu::Texture,
    dummy_sampler: wgpu::Sampler,
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
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!("shaders/pipeline.wgsl"))),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });

        // Dummy 1x1 white texture for fallback
        let dummy_texture = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("dummy_texture"),
                size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
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
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let dummy_tex_view = dummy_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let textured_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let textured_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("textured_pipeline_layout"),
            bind_group_layouts: &[Some(&textured_bind_group_layout)],
            immediate_size: 0,
        });

        let textured_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("textured_bg"),
            layout: &textured_bind_group_layout,
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

        Self {
            device,
            queue,
            width: width as f32,
            height: height as f32,
            solid_pipeline,
            textured_pipeline,
            textured_bind_group,
            glyph_atlas: None,
            dummy_texture,
            dummy_sampler,
        }
    }

    fn init_glyph_atlas(&mut self) {
        if self.glyph_atlas.is_none() {
            self.glyph_atlas = Some(GlyphAtlas::new(&self.device, 512));
        }
    }

    pub fn render(
        &mut self,
        target: &wgpu::TextureView,
        list: &paint::DisplayList,
    ) -> wgpu::CommandBuffer {
        self.init_glyph_atlas();

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("tsfire encoder"),
            });

        let mut solid_vertices: Vec<SolidVertex> = Vec::new();
        let mut textured_vertices: Vec<TexturedVertex> = Vec::new();

        for cmd in &list.items {
            match cmd {
                paint::DisplayCommand::FillRect(rect, color) => {
                    self.add_fill_rect(rect, color, &mut solid_vertices);
                }
                paint::DisplayCommand::TextRun(rect, _color, font_size, _font_family, range) => {
                    self.add_text_run(list, rect, *font_size, range, &mut textured_vertices);
                }
                _ => {}
            }
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tsfire pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            if !solid_vertices.is_empty() {
                let vb = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("solid_vertices"),
                        usage: wgpu::BufferUsages::VERTEX,
                        contents: bytemuck::cast_slice(&solid_vertices),
                    });

                pass.set_pipeline(&self.solid_pipeline);
                pass.set_vertex_buffer(0, vb.slice(..));
                pass.draw(0..solid_vertices.len() as u32, 0..1);
            }

            if !textured_vertices.is_empty() {
                let vb = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("textured_vertices"),
                        usage: wgpu::BufferUsages::VERTEX,
                        contents: bytemuck::cast_slice(&textured_vertices),
                    });

                pass.set_pipeline(&self.textured_pipeline);
                pass.set_bind_group(0, &self.textured_bind_group, &[]);
                pass.set_vertex_buffer(0, vb.slice(..));
                pass.draw(0..textured_vertices.len() as u32, 0..1);
            }
        }

        encoder.finish()
    }

    fn add_fill_rect(&self, rect: &layout::Rect, color: &parsing::Color, out: &mut Vec<SolidVertex>) {
        let ndc_left = -1.0 + 2.0 * rect.x / self.width;
        let ndc_right = -1.0 + 2.0 * (rect.x + rect.width) / self.width;
        let ndc_top = 1.0 - 2.0 * rect.y / self.height;
        let ndc_bottom = 1.0 - 2.0 * (rect.y + rect.height) / self.height;

        let c = [
            color.0 as f32 / 255.0,
            color.1 as f32 / 255.0,
            color.2 as f32 / 255.0,
            color.3 as f32 / 255.0,
        ];

        out.push(SolidVertex { position: [ndc_left, ndc_top], color: c });
        out.push(SolidVertex { position: [ndc_left, ndc_bottom], color: c });
        out.push(SolidVertex { position: [ndc_right, ndc_bottom], color: c });

        out.push(SolidVertex { position: [ndc_right, ndc_bottom], color: c });
        out.push(SolidVertex { position: [ndc_right, ndc_top], color: c });
        out.push(SolidVertex { position: [ndc_left, ndc_top], color: c });
    }

    fn add_text_run(
        &mut self,
        list: &paint::DisplayList,
        rect: &layout::Rect,
        font_size: f32,
        range: &TextRange,
        out: &mut Vec<TexturedVertex>,
    ) {
        let text: String = list.text_arena[range.start as usize..][..range.len as usize]
            .chars()
            .collect();

        let char_w = font_size * 0.6;

        let mut x = rect.x;

        for ch in text.chars() {
            let codepoint = ch as u32;

            let uv = if let Some(atlas) = &self.glyph_atlas {
                if let Some(uv) = atlas.get_glyph(codepoint) {
                    *uv
                } else {
                    let uv = GlyphUV {
                        x: 0.0,
                        y: 0.0,
                        width: char_w / atlas.atlas_size as f32,
                        height: font_size * 1.2 / atlas.atlas_size as f32,
                    };
                    if let Some(atlas) = &mut self.glyph_atlas {
                        atlas.add_glyph(codepoint, uv);
                    }
                    uv
                }
            } else {
                GlyphUV {
                    x: 0.0,
                    y: 0.0,
                    width: char_w / 512.0,
                    height: font_size * 1.2 / 512.0,
                }
            };

            let ndc_left = -1.0 + 2.0 * x / self.width;
            let ndc_right = -1.0 + 2.0 * (x + char_w) / self.width;
            let ndc_top = 1.0 - 2.0 * rect.y / self.height;
            let ndc_bottom = 1.0 - 2.0 * (rect.y + font_size * 1.2) / self.height;

            out.push(TexturedVertex {
                position: [ndc_left, ndc_top],
                uv: [uv.x, uv.y],
            });
            out.push(TexturedVertex {
                position: [ndc_left, ndc_bottom],
                uv: [uv.x, uv.y + uv.height],
            });
            out.push(TexturedVertex {
                position: [ndc_right, ndc_bottom],
                uv: [uv.x + uv.width, uv.y + uv.height],
            });
            out.push(TexturedVertex {
                position: [ndc_right, ndc_bottom],
                uv: [uv.x + uv.width, uv.y + uv.height],
            });
            out.push(TexturedVertex {
                position: [ndc_right, ndc_top],
                uv: [uv.x + uv.width, uv.y],
            });
            out.push(TexturedVertex {
                position: [ndc_left, ndc_top],
                uv: [uv.x, uv.y],
            });

            x += char_w;
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width as f32;
        self.height = height as f32;
    }
}
