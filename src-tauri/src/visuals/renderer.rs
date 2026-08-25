use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
    thread,
    time::{Duration, Instant},
};

use bytemuck::{Pod, Zeroable};
use tauri::Window;
use wgpu::util::DeviceExt;

use crate::analysis::{SharedAnalysis, VisualInputFrame};

use super::{
    intensity_values, palette_for, smooth_palette, FlashEnvelope, SmoothedVisualState,
    VisualSettings,
};

const FRAME_INTERVAL: Duration = Duration::from_nanos(16_666_667);

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct VisualUniforms {
    resolution_time: [f32; 4],
    music: [f32; 4],
    pulse: [f32; 4],
    visual: [f32; 4],
    color_a: [f32; 4],
    color_b: [f32; 4],
    color_c: [f32; 4],
    color_d: [f32; 4],
    style_a: [f32; 4],
    style_b: [f32; 4],
    effects: [f32; 4],
}

pub fn run_renderer(
    window: Window,
    stop_requested: Arc<AtomicBool>,
    settings: Arc<RwLock<VisualSettings>>,
    analysis: SharedAnalysis,
    output_mode: Arc<std::sync::atomic::AtomicU8>,
) -> Result<(), String> {
    pollster::block_on(Renderer::run(
        window,
        stop_requested,
        settings,
        analysis,
        output_mode,
    ))
}

struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl Renderer {
    async fn run(
        window: Window,
        stop_requested: Arc<AtomicBool>,
        settings: Arc<RwLock<VisualSettings>>,
        analysis: SharedAnalysis,
        output_mode: Arc<std::sync::atomic::AtomicU8>,
    ) -> Result<(), String> {
        let size = window.inner_size().map_err(|error| error.to_string())?;
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window.clone())
            .map_err(|error| error.to_string())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .map_err(|error| error.to_string())?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("PulseBridge Visuals GPU"),
                ..Default::default()
            })
            .await
            .map_err(|error| error.to_string())?;
        let width = size.width.max(1);
        let height = size.height.max(1);
        let config = surface
            .get_default_config(&adapter, width, height)
            .ok_or_else(|| "The selected display has no compatible GPU surface".to_string())?;
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("PulseBridge performance shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/performance.wgsl").into()),
        });
        let initial_uniforms = VisualUniforms::zeroed();
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Visual parameter snapshot"),
            contents: bytemuck::bytes_of(&initial_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Visual parameter layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Visual parameter binding"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Performance pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Performance shader pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let mut renderer = Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            uniform_buffer,
            bind_group,
        };
        let started_at = Instant::now();
        let mut last_frame = started_at;
        let mut next_frame = started_at;
        let mut smoothed = SmoothedVisualState::default();
        let mut flash_envelope = FlashEnvelope::default();
        let initial_settings = read_settings(&settings);
        let mut smoothed_palette =
            palette_for(initial_settings.palette, VisualInputFrame::default().state);

        while !stop_requested.load(Ordering::Acquire) {
            let now = Instant::now();
            let elapsed = now.saturating_duration_since(started_at).as_secs_f32();
            let delta = now
                .saturating_duration_since(last_frame)
                .as_secs_f32()
                .clamp(0.001, 0.1);
            last_frame = now;
            let current_settings = read_settings(&settings);
            let output_mode_value = output_mode.load(Ordering::Acquire);
            let mut frame = analysis
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .visual_input(now);
            if output_mode_value == 1 {
                frame = VisualInputFrame::default();
            }
            frame.reactivity =
                (frame.reactivity * current_settings.music_reactivity).clamp(0.0, 1.5);
            smoothed.update(frame, current_settings.style, delta);

            let size = window.inner_size().map_err(|error| error.to_string())?;
            if size.width > 0
                && size.height > 0
                && (size.width != renderer.config.width || size.height != renderer.config.height)
            {
                renderer.config.width = size.width;
                renderer.config.height = size.height;
                renderer
                    .surface
                    .configure(&renderer.device, &renderer.config);
            }

            let intensities = intensity_values(current_settings.intensity);
            let target_palette = palette_for(current_settings.palette, frame.state);
            smooth_palette(&mut smoothed_palette, target_palette, delta);
            let impact_flash = flash_envelope.update(
                elapsed,
                frame.impact,
                current_settings.flash,
                current_settings.flash_strength,
                intensities[3],
                delta,
            );
            let uniforms = VisualUniforms {
                resolution_time: [
                    renderer.config.width as f32,
                    renderer.config.height as f32,
                    elapsed,
                    delta,
                ],
                music: [
                    smoothed.energy,
                    smoothed.bass,
                    smoothed.mids,
                    smoothed.highs,
                ],
                pulse: [
                    frame.beat_phase,
                    smoothed.beat_pulse,
                    smoothed.onset,
                    impact_flash,
                ],
                visual: [
                    (0.18 + smoothed.energy * 0.82) * intensities[0] * current_settings.motion,
                    (0.15 + smoothed.highs * 0.85) * intensities[1],
                    0.9 + smoothed.bass * 0.28,
                    (0.34 + smoothed.energy * 0.72) * intensities[2] * current_settings.brightness,
                ],
                color_a: smoothed_palette[0],
                color_b: smoothed_palette[1],
                color_c: smoothed_palette[2],
                color_d: smoothed_palette[3],
                style_a: [
                    smoothed.style_weights[0],
                    smoothed.style_weights[1],
                    smoothed.style_weights[2],
                    smoothed.style_weights[3],
                ],
                style_b: [
                    smoothed.style_weights[4],
                    0.78 + smoothed.energy * 0.34,
                    smoothed.reactivity,
                    (output_mode_value == 2) as u8 as f32,
                ],
                effects: [
                    smoothed.sub,
                    smoothed.impact * intensities[3],
                    current_settings.color_change,
                    0.0,
                ],
            };
            renderer.render(uniforms)?;

            next_frame += FRAME_INTERVAL;
            let after_render = Instant::now();
            if next_frame > after_render {
                thread::sleep(next_frame - after_render);
            } else {
                next_frame = after_render;
            }
        }

        Ok(())
    }

    fn render(&mut self, uniforms: VisualUniforms) -> Result<(), String> {
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                self.surface.configure(&self.device, &self.config);
                frame
            }
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(())
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("The GPU rejected a performance frame".to_string())
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Performance frame encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Performance frame"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
        Ok(())
    }
}

fn read_settings(settings: &RwLock<VisualSettings>) -> VisualSettings {
    settings
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_performance_shader_is_valid_wgsl() {
        let module = naga::front::wgsl::parse_str(include_str!("../../shaders/performance.wgsl"))
            .expect("performance shader should parse");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("performance shader should validate");
    }
}
