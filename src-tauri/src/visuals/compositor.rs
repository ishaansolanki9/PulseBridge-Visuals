//! Cache two independent scene images for a true dissolve after all effects.
use super::*;

pub(super) struct SceneCompositor {
    textures: [wgpu::Texture; 2],
    views: [wgpu::TextureView; 2],
    uniforms: [wgpu::Buffer; 2],
    bindings: [wgpu::BindGroup; 2],
    blend_buffer: wgpu::Buffer,
    layout: wgpu::BindGroupLayout,
    binding: wgpu::BindGroup,
    sampler: wgpu::Sampler,
    pipeline: wgpu::RenderPipeline,
    previous: Option<VisualUniforms>,
    outgoing: Option<VisualUniforms>,
}

impl SceneCompositor {
    pub fn new(device: &wgpu::Device, scene_layout: &wgpu::BindGroupLayout) -> Self {
        let uniforms = std::array::from_fn(|_| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Isolated scene parameters"),
                contents: bytemuck::bytes_of(&VisualUniforms::zeroed()),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            })
        });
        let bindings = std::array::from_fn(|index| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Isolated scene binding"),
                layout: scene_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniforms[index].as_entire_binding(),
                }],
            })
        });
        let (a, av) = create_render_target(device, 1, 1);
        let (b, bv) = create_render_target(device, 1, 1);
        let views = [av, bv];
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
        let blend_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Scene dissolve weights"),
            contents: bytemuck::cast_slice(&[1.0f32, 0.0, 0.0, 0.0]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let texture_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Scene dissolve layout"),
            entries: &[
                texture_entry(0),
                texture_entry(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let binding = composite_binding(device, &layout, &views, &sampler, &blend_buffer);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Completed scene dissolve"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/composite.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Completed scene dissolve"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: INTERNAL_RENDER_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            textures: [a, b],
            views,
            uniforms,
            bindings,
            blend_buffer,
            layout,
            binding,
            sampler,
            pipeline,
            previous: None,
            outgoing: None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        base: &wgpu::RenderPipeline,
        lines: &wgpu::RenderPipeline,
        uniforms: VisualUniforms,
    ) -> bool {
        if uniforms.style_a[0] == uniforms.style_a[1] {
            self.previous = Some(uniforms);
            self.outgoing = None;
            return false;
        }
        if self.outgoing.is_none() {
            self.outgoing = Some(
                self.previous
                    .filter(|old| old.style_a[0] == uniforms.style_a[0])
                    .unwrap_or(uniforms),
            );
        }
        let (width, height) = (
            uniforms.resolution_time[0] as u32,
            uniforms.resolution_time[1] as u32,
        );
        if self.textures[0].width() != width || self.textures[0].height() != height {
            let (a, av) = create_render_target(device, width, height);
            let (b, bv) = create_render_target(device, width, height);
            self.textures = [a, b];
            self.views = [av, bv];
            self.binding = composite_binding(
                device,
                &self.layout,
                &self.views,
                &self.sampler,
                &self.blend_buffer,
            );
        }
        let outgoing = isolated_scene(uniforms, uniforms.style_a[0], self.outgoing);
        let incoming = isolated_scene(uniforms, uniforms.style_a[1], None);
        for (index, scene) in [outgoing, incoming].iter().enumerate() {
            queue.write_buffer(&self.uniforms[index], 0, bytemuck::bytes_of(scene));
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Isolated transition scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.views[index],
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(base);
            pass.set_bind_group(0, &self.bindings[index], &[]);
            pass.draw(0..3, 0..1);
            draw_line_scenes(&mut pass, lines, scene);
        }
        let total = (uniforms.style_a[2] + uniforms.style_a[3]).max(0.001);
        let weights = [
            uniforms.style_a[2] / total,
            uniforms.style_a[3] / total,
            0.0,
            0.0,
        ];
        queue.write_buffer(&self.blend_buffer, 0, bytemuck::cast_slice(&weights));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Scene dissolve"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.binding, &[]);
        pass.draw(0..3, 0..1);
        true
    }
}

fn composite_binding(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    views: &[wgpu::TextureView; 2],
    sampler: &wgpu::Sampler,
    weights: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Scene dissolve images"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&views[0]),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&views[1]),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: weights.as_entire_binding(),
            },
        ],
    })
}

fn isolated_scene(
    mut live: VisualUniforms,
    family: f32,
    appearance: Option<VisualUniforms>,
) -> VisualUniforms {
    if let Some(old) = appearance {
        // Keep outgoing identity, color, budgets, and effects. Live audio lanes,
        // time, color travel, and geometry history continue throughout the fade.
        for (visual_lane, budget_lane) in [(0, 0), (1, 1), (3, 3)] {
            live.visual[visual_lane] *= old.scene[budget_lane] / live.scene[budget_lane].max(0.01);
        }
        live.scene = old.scene;
        live.style_b[2] = old.style_b[2];
        live.modifiers = old.modifiers;
        live.color_a = old.color_a;
        live.color_b = old.color_b;
        live.color_c = old.color_c;
        live.color_d = old.color_d;
    }
    live.style_a = [family, family, 1.0, 0.0];
    live
}
