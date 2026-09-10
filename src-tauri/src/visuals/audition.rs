//! Explicit, ignored development harness. No audio device is opened and none of
//! this module is compiled into the application. Captures use the production WGSL.
use super::*;
use crate::analysis::MusicState;
use crate::visuals::director::SpectacleEnvelope;
use std::{fs, io::Write, path::Path};

struct Stage {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    binding: wgpu::BindGroup,
    uniform: wgpu::Buffer,
}
impl Stage {
    async fn new() -> Self {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .expect("native GPU required");
        eprintln!("SYNTHETIC AUDITION — {:?}", adapter.get_info());
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .unwrap();
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Production audition shader"),
            source: wgpu::ShaderSource::Wgsl(PERFORMANCE_SHADER.into()),
        });
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::bytes_of(&VisualUniforms::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
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
        let line_pipeline =
            create_line_pipeline(&device, &shader, &pipeline_layout, INTERNAL_RENDER_FORMAT);
        Self {
            line_pipeline,
            device,
            queue,
            pipeline,
            binding,
            uniform,
        }
    }
    fn target(&self, width: u32, height: u32) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: INTERNAL_RENDER_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }
    fn draw(
        &self,
        uniforms: VisualUniforms,
        target: &wgpu::Texture,
        capture: Option<&Path>,
    ) -> f64 {
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&uniforms));
        let view = target.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
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
            pass.set_bind_group(0, &self.binding, &[]);
            pass.draw(0..3, 0..1);
            draw_line_scenes(&mut pass, &self.line_pipeline, &uniforms);
        }
        let start = Instant::now();
        self.queue.submit(Some(encoder.finish()));
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(Duration::from_secs(30)),
            })
            .unwrap();
        let milliseconds = start.elapsed().as_secs_f64() * 1000.0;
        if let Some(path) = capture {
            let width = target.width();
            let height = target.height();
            let stride = (width * 4).div_ceil(256) * 256;
            let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: u64::from(stride * height),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut copy = self.device.create_command_encoder(&Default::default());
            copy.copy_texture_to_buffer(
                target.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(stride),
                        rows_per_image: Some(height),
                    },
                },
                target.size(),
            );
            self.queue.submit(Some(copy.finish()));
            readback.map_async(wgpu::MapMode::Read, .., |result| result.unwrap());
            self.device
                .poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: Some(Duration::from_secs(30)),
                })
                .unwrap();
            let data = readback.get_mapped_range(..).unwrap();
            let mut file = fs::File::create(path).unwrap();
            write!(file, "P6\n{width} {height}\n255\n").unwrap();
            for row in data.chunks(stride as usize).take(height as usize) {
                let rgb: Vec<u8> = row[..width as usize * 4]
                    .chunks(4)
                    .flat_map(|p| p[..3].iter().copied())
                    .collect();
                file.write_all(&rgb).unwrap();
            }
            drop(data);
            readback.unmap();
        }
        milliseconds
    }
}
fn synthetic_frame(t: f32) -> VisualInputFrame {
    let (state, energy) = if t < 2.0 {
        (MusicState::Quiet, 0.2)
    } else if t < 5.0 {
        (MusicState::Groove, 0.55)
    } else if t < 8.0 {
        (MusicState::Build, 0.55 + (t - 5.0) * 0.13)
    } else if t < 8.2 {
        (MusicState::Impact, 0.95)
    } else if t < 10.0 {
        (MusicState::Peak, 0.85)
    } else {
        (MusicState::Breakdown, 0.25)
    };
    let phase = (t * 2.0).fract();
    let beat = (-phase * 14.0).exp();
    VisualInputFrame {
        state,
        energy,
        sub: energy * 0.75,
        bass: energy * (0.55 + beat * 0.45),
        mids: energy * (0.7 + 0.2 * (t * 3.0).sin()),
        highs: energy * (0.3 + 0.5 * (t * 7.0).sin().max(0.0)),
        beat_phase: phase,
        beat_pulse: beat,
        tempo_bpm: 120.0,
        beat_confidence: 0.9,
        beat_index: (t * 2.0) as u64,
        bar_phase: (t * 0.5).fract(),
        onset: beat * energy,
        impact: if state == MusicState::Impact {
            1.0
        } else {
            0.0
        },
        reactivity: 1.0,
    }
}
fn fixture(
    id: u32,
    t: f32,
    quality: f32,
    size: (u32, u32),
    smoothed: SmoothedVisualState,
    transformation: f32,
    clock: f32,
) -> VisualUniforms {
    let palette = palette_for(PaletteName::Electric, MusicState::Groove);
    let drive = smoothed.drive;
    VisualUniforms {
        resolution_time: [size.0 as f32, size.1 as f32, t, 1.0 / 30.0],
        music: [
            smoothed.energy,
            smoothed.bass,
            smoothed.mids,
            smoothed.highs,
        ],
        pulse: [(t * 2.0).fract(), smoothed.beat_pulse, smoothed.onset, 0.0],
        visual: [0.6, 0.5, 1.0, (0.3 + drive * 0.7) * 0.8],
        color_a: palette[0],
        color_b: palette[1],
        color_c: palette[2],
        color_d: palette[3],
        style_a: [id as f32, id as f32, 1.0, 0.0],
        style_b: [0.95, drive, 0.4, 0.0],
        effects: [smoothed.sub, smoothed.impact, 1.0, drive],
        scene: [0.8, 0.6, 0.35 + drive * 0.4, 0.8],
        modifiers: [-1.0, 0.0, -1.0, 0.0],
        reactive: [
            smoothed.bass_hit,
            smoothed.mid_motion,
            smoothed.high_hit,
            smoothed.energy_rise,
        ],
        spatial: [clock, transformation, quality, 0.0],
        signal_history: [[
            smoothed.bass_hit,
            smoothed.mid_motion,
            smoothed.high_hit,
            smoothed.energy_rise,
        ]; 32],
    }
}
#[test]
#[ignore = "requires a native GPU; writes explicitly synthetic audition frames and timings"]
fn native_scene_audition() {
    let output = std::env::var_os("PULSEBRIDGE_AUDITION_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("target/audition"));
    fs::create_dir_all(&output).unwrap();
    let stage = pollster::block_on(Stage::new());
    let capture_target = stage.target(640, 360);
    let mut report = String::from("Synthetic native audition. Same Electric palette, flashing off. Timings include queue submit + GPU wait; exclude readback and surface presentation. Not an audio integration test.\n");
    for id in 26..32 {
        let mut smoothed = SmoothedVisualState::default();
        let mut event = SpectacleEnvelope::default();
        let mut clock = 0.0;
        let mut history = crate::visuals::reaction_history::ReactionHistory::default();
        for index in 0..180 {
            let t = index as f32 / 15.0;
            let frame = synthetic_frame(t);
            smoothed.update(frame, 1.0 / 15.0);
            let transformation = event.update(t, frame, super::super::IntensityProfile::Wild);
            clock += (0.25 + smoothed.drive * 0.65) / 15.0;
            let quality = if (60..120).contains(&index) { 0.0 } else { 1.0 };
            history.update(
                [
                    smoothed.bass_hit,
                    smoothed.mid_motion,
                    smoothed.high_hit,
                    smoothed.energy_rise,
                ],
                1.0 / 15.0,
            );
            let mut uniforms = fixture(id, t, quality, (640, 360), smoothed, transformation, clock);
            uniforms.signal_history = history.snapshot();
            uniforms.spatial[3] = history.fraction_seconds();
            stage.draw(
                uniforms,
                &capture_target,
                Some(&output.join(format!("scene-{id}-{index:03}.ppm"))),
            );
        }
        for (quality, width, height) in [(1.0, 1920, 1080), (0.5, 1280, 720), (0.0, 960, 540)] {
            let target = stage.target(width, height);
            let mut timings = Vec::new();
            for sample in 0..30 {
                let t = 6.0 + sample as f32 / 30.0;
                smoothed.update(synthetic_frame(t), 1.0 / 30.0);
                let uniforms = fixture(
                    id,
                    t,
                    quality,
                    (width, height),
                    smoothed,
                    0.75,
                    4.0 + sample as f32 / 60.0,
                );
                let ms = stage.draw(uniforms, &target, None);
                if sample >= 5 {
                    timings.push(ms);
                }
            }
            timings.sort_by(f64::total_cmp);
            let line = format!(
                "scene {id}, {width}x{height}, quality {quality}: median {:.2}ms, p95 {:.2}ms\n",
                timings[timings.len() / 2],
                timings[(timings.len() as f32 * 0.95) as usize]
            );
            eprint!("{line}");
            report.push_str(&line);
        }
    }
    let transition_target = stage.target(1280, 720);
    let mut transition_timings = Vec::new();
    for index in 0..30 {
        let progress = index as f32 / 29.0;
        let mut smoothed = SmoothedVisualState::default();
        smoothed.update(synthetic_frame(8.0), 0.1);
        let mut uniforms = fixture(27, 8.0, 0.5, (1280, 720), smoothed, 0.7, 4.0 + progress);
        uniforms.style_a = [27.0, 30.0, 1.0 - progress, progress];
        let ms = stage.draw(uniforms, &transition_target, None);
        if index > 4 {
            transition_timings.push(ms);
        }
    }
    transition_timings.sort_by(f64::total_cmp);
    let line = format!("Ribbon Reactor + Prism Surge transition, 1280x720, quality 0.5: median {:.2}ms, p95 {:.2}ms\n", transition_timings[12], transition_timings[23]);
    eprint!("{line}");
    report.push_str(&line);
    for index in 0..60 {
        let progress = index as f32 / 59.0;
        let mut smoothed = SmoothedVisualState::default();
        smoothed.update(synthetic_frame(8.0), 0.1);
        let mut uniforms = fixture(27, 8.0, 0.0, (640, 360), smoothed, 0.3, 4.0 + progress);
        uniforms.style_a = [27.0, 28.0, 1.0 - progress, progress];
        stage.draw(
            uniforms,
            &capture_target,
            Some(&output.join(format!("transition-{index:03}.ppm"))),
        );
    }
    for index in 0..30 {
        let progress = index as f32 / 29.0;
        let mut smoothed = SmoothedVisualState::default();
        smoothed.update(synthetic_frame(8.0), 0.1);
        let mut uniforms = fixture(
            0,
            8.0 + progress,
            0.5,
            (640, 360),
            smoothed,
            0.0,
            3.0 + progress,
        );
        uniforms.style_a = [0.0, 27.0, 1.0 - progress, progress];
        stage.draw(
            uniforms,
            &capture_target,
            Some(&output.join(format!("mixed-transition-{index:03}.ppm"))),
        );
    }
    fs::write(output.join("timings.txt"), report).unwrap();
}

/// Compare spatial light distributions after normalizing total brightness.
/// A whole-image brightness pulse cannot pass this response check.
#[test]
#[ignore = "requires a native GPU; captures isolated bands with a fixed camera and exposure"]
fn native_line_response_audition() {
    let output = std::env::var_os("PULSEBRIDGE_AUDITION_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("target/audition"));
    fs::create_dir_all(&output).unwrap();
    let stage = pollster::block_on(Stage::new());
    let target = stage.target(640, 360);
    fn distribution(path: &Path) -> Vec<f32> {
        let bytes = fs::read(path).unwrap();
        let start = bytes
            .iter()
            .enumerate()
            .filter(|(_, byte)| **byte == b'\n')
            .nth(2)
            .unwrap()
            .0
            + 1;
        let mut values: Vec<f32> = bytes[start..]
            .chunks_exact(3)
            .map(|rgb| 0.2126 * rgb[0] as f32 + 0.7152 * rgb[1] as f32 + 0.0722 * rgb[2] as f32)
            .collect();
        let sum: f32 = values.iter().sum();
        assert!(sum > 10000.0, "scene must render visible geometry");
        for value in &mut values {
            *value /= sum;
        }
        values
    }
    let mut report = String::from("Fixed clock/camera/exposure. Flash off. Normalized spatial image difference (0 = brightness-only change).\n");
    for family in 26..32 {
        let mut uniforms = fixture(
            family,
            4.0,
            0.5,
            (640, 360),
            SmoothedVisualState::default(),
            0.0,
            2.5,
        );
        uniforms.visual[3] = 0.65;
        uniforms.signal_history = [[0.0; 4]; 32];
        uniforms.reactive = [0.0; 4];
        let baseline_file = output.join(format!("response-{family}-baseline.ppm"));
        stage.draw(uniforms, &target, Some(&baseline_file));
        let baseline = distribution(&baseline_file);
        // Even an enabled legacy flash must not bleach the line geometry.
        uniforms.pulse[3] = 1.0;
        let flash_file = output.join(format!("response-{family}-flash.ppm"));
        stage.draw(uniforms, &target, Some(&flash_file));
        assert_eq!(
            fs::read(&baseline_file).unwrap(),
            fs::read(&flash_file).unwrap()
        );
        uniforms.pulse[3] = 0.0;
        uniforms.style_b[3] = 1.0;
        let black_file = output.join(format!("response-{family}-black.ppm"));
        stage.draw(uniforms, &target, Some(&black_file));
        let black = fs::read(&black_file).unwrap();
        assert!(black[black.len() - 640 * 360 * 3..]
            .iter()
            .all(|byte| *byte == 0));
        uniforms.style_b[3] = 0.0;
        for (lane, label) in [(0, "bass"), (1, "mids"), (2, "highs")] {
            uniforms.signal_history = [[0.0; 4]; 32];
            for (index, sample) in uniforms.signal_history.iter_mut().enumerate() {
                let age = index as f32 / 30.0;
                sample[lane] = (-(age - 0.35).powi(2) / 0.04).exp() * 0.95;
            }
            let file = output.join(format!("response-{family}-{label}.ppm"));
            stage.draw(uniforms, &target, Some(&file));
            let actual = distribution(&file);
            let difference: f32 = actual
                .iter()
                .zip(&baseline)
                .map(|(a, b)| (a - b).abs())
                .sum();
            let line = format!("scene {family}, {label}: {difference:.3}\n");
            eprint!("{line}");
            report.push_str(&line);
            assert!(
                difference > 0.20,
                "scene {family} must respond spatially to {label}, got {difference}"
            );
        }
    }
    fs::write(output.join("band-response.txt"), report).unwrap();
}
