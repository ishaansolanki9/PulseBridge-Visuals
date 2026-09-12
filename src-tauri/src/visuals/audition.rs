//! Explicit, ignored development harness. No audio device is opened and none of
//! this module is compiled into the application. Captures use the production WGSL.
use super::*;
use crate::analysis::MusicState;
use crate::visuals::director::SpectacleEnvelope;
use std::{fs, io::Write, path::Path};

struct Stage {
    startup_duration: Duration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    compositor: compositor::SceneCompositor,
    binding: wgpu::BindGroup,
    uniform: wgpu::Buffer,
}

#[test]
#[ignore = "requires a native GPU; Windows CI runs this with a process timeout"]
fn native_pipeline_startup_audition() {
    let started = Instant::now();
    let mut stage = pollster::block_on(Stage::new());
    // Exercise the production presentation shader as well as scene pipelines.
    let blitter_started = Instant::now();
    let _blitter = TextureBlitterBuilder::new(&stage.device, wgpu::TextureFormat::Bgra8UnormSrgb)
        .sample_type(wgpu::FilterMode::Linear)
        .build();
    // Live output prepares the instance/surface before starting its 12-second
    // worker deadline. Do not charge cold DX12 instance loading to that budget.
    let elapsed = stage.startup_duration + blitter_started.elapsed();
    eprintln!(
        "Native worker startup: {:.3}s; including instance loading: {:.3}s",
        elapsed.as_secs_f64(),
        started.elapsed().as_secs_f64()
    );

    let output = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/pipeline-startup");
    fs::create_dir_all(&output).unwrap();
    let target = stage.target(160, 100);
    // Legacy 2D, Tron cycling/held, new 2D, instanced 3D, and an image dissolve.
    for (index, (from, to)) in [(0, 0), (32, 32), (33, 33), (45, 45), (49, 49), (33, 49)]
        .into_iter()
        .enumerate()
    {
        let mut uniforms = fixture(
            from,
            4.0,
            1.0,
            (160, 100),
            SmoothedVisualState::default(),
            0.0,
            6.8,
        );
        if from != to {
            uniforms.style_a = [from as f32, to as f32, 0.5, 0.5];
        }
        let file = output.join(format!("scene-{index}.ppm"));
        let frame_ms = stage.draw(uniforms, &target, Some(&file));
        eprintln!("Native startup frame {from}/{to}: {frame_ms:.3}ms");
        let image = fs::read(&file).unwrap();
        let pixels = image.splitn(4, |byte| *byte == b'\n').nth(3).unwrap();
        assert_eq!(pixels.len(), 160 * 100 * 3);
        assert!(
            pixels.iter().any(|value| *value > 8),
            "scene {from}/{to} rendered black"
        );
    }
    assert!(
        elapsed < Duration::from_secs(12),
        "native pipelines exceeded the live startup budget: {elapsed:?}"
    );
}

impl Stage {
    async fn new() -> Self {
        let instance_started = Instant::now();
        let instance = gpu::create_instance();
        eprintln!(
            "GPU instance ready: {:.3}s",
            instance_started.elapsed().as_secs_f64()
        );
        let worker_started = Instant::now();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .expect("native GPU required");
        eprintln!("SYNTHETIC AUDITION — {:?}", adapter.get_info());
        eprintln!(
            "GPU adapter ready: {:.3}s",
            worker_started.elapsed().as_secs_f64()
        );
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
        let pipeline =
            create_performance_pipeline(&device, &shader, &pipeline_layout, INTERNAL_RENDER_FORMAT)
                .await
                .expect("production fullscreen pipeline");
        let line_pipeline =
            create_line_pipeline(&device, &shader, &pipeline_layout, INTERNAL_RENDER_FORMAT);
        let compositor = compositor::SceneCompositor::new(&device, &layout);
        Self {
            startup_duration: worker_started.elapsed(),
            compositor,
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
        &mut self,
        uniforms: VisualUniforms,
        target: &wgpu::Texture,
        capture: Option<&Path>,
    ) -> f64 {
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&uniforms));
        let view = target.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        if !self.compositor.draw(
            &self.device,
            &self.queue,
            &mut encoder,
            &view,
            &self.pipeline,
            &self.line_pipeline,
            uniforms,
        ) {
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
        chromatic: [
            (t * 0.12).fract(),
            1.0,
            t * (0.08 + drive * 1.92) * 0.8,
            0.0,
        ],
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
    let mut stage = pollster::block_on(Stage::new());
    let capture_target = stage.target(640, 360);
    let mut report = String::from("Synthetic native audition. Same Electric palette, flashing off. Timings include queue submit + GPU wait; exclude readback and surface presentation. Not an audio integration test.\n");
    for id in 26..32 {
        let mut smoothed = SmoothedVisualState::default();
        let mut event = SpectacleEnvelope::default();
        let mut clock = 0.0;
        let mut color_clock = super::super::palette::ColorMotion::default();
        let mut legacy_clock = 0.0;
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
            uniforms.chromatic[0] =
                color_clock.update(1.0 / 15.0, smoothed.drive, smoothed.onset, 1.0);
            legacy_clock += (0.08 + smoothed.drive * 1.92) * 0.8 / 15.0;
            uniforms.chromatic[2] = legacy_clock;
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
    let transition_target = stage.target(1920, 1080);
    let mut transition_timings = Vec::new();
    for index in 0..30 {
        let progress = index as f32 / 29.0;
        let mut smoothed = SmoothedVisualState::default();
        smoothed.update(synthetic_frame(8.0), 0.1);
        let mut uniforms = fixture(27, 8.0, 1.0, (1920, 1080), smoothed, 0.7, 4.0 + progress);
        uniforms.style_a = [27.0, 30.0, 1.0 - progress, progress];
        let ms = stage.draw(uniforms, &transition_target, None);
        if index > 4 {
            transition_timings.push(ms);
        }
    }
    transition_timings.sort_by(f64::total_cmp);
    let line = format!("Ribbon Reactor + Prism Surge transition, 1920x1080, quality 1: median {:.2}ms, p95 {:.2}ms\n", transition_timings[12], transition_timings[23]);
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
    let mut stage = pollster::block_on(Stage::new());
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

#[test]
#[ignore = "requires a native GPU; isolates color travel from geometry and captures palette transitions"]
fn native_color_audition() {
    let output = std::env::var_os("PULSEBRIDGE_AUDITION_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("target/audition"));
    fs::create_dir_all(&output).unwrap();
    let mut stage = pollster::block_on(Stage::new());
    let target = stage.target(640, 360);
    let mut report = String::from("Fixed geometry, clock, exposure, and reactive bands. Only color phase or palette changes. Average normalized RGB change over bright line pixels.\n");
    fn rgb(path: &Path) -> Vec<[f32; 3]> {
        let bytes = fs::read(path).unwrap();
        bytes[bytes.len() - 640 * 360 * 3..]
            .chunks_exact(3)
            .map(|pixel| {
                pixel
                    .try_into()
                    .map(|p: [u8; 3]| p.map(|v| v as f32))
                    .unwrap()
            })
            .collect()
    }
    for family in 26..32 {
        let mut uniforms = fixture(
            family,
            4.0,
            1.0,
            (640, 360),
            SmoothedVisualState::default(),
            0.0,
            2.5,
        );
        uniforms.visual[3] = 0.65;
        uniforms.signal_history = [[0.6, 0.5, 0.15, 0.0]; 32];
        uniforms.chromatic = [0.0, 1.0, 0.0, 0.0];
        let first = output.join(format!("color-{family}-0.ppm"));
        stage.draw(uniforms, &target, Some(&first));
        let baseline = rgb(&first);
        for (index, phase) in [(1, 0.33), (2, 0.66), (3, 0.0)] {
            uniforms.chromatic[0] = phase;
            if index == 3 {
                let palette = palette_for(PaletteName::Sunset, MusicState::Build);
                [
                    uniforms.color_a,
                    uniforms.color_b,
                    uniforms.color_c,
                    uniforms.color_d,
                ] = palette;
            }
            let file = output.join(format!("color-{family}-{index}.ppm"));
            stage.draw(uniforms, &target, Some(&file));
            let mut change = 0.0;
            let mut count = 0;
            for (before, after) in baseline.iter().zip(rgb(&file)) {
                let a: f32 = before.iter().sum();
                let b: f32 = after.iter().sum();
                if a > 90.0 && b > 90.0 {
                    change += before
                        .iter()
                        .zip(after)
                        .map(|(x, y)| (x / a - y / b).abs())
                        .sum::<f32>();
                    count += 1;
                }
            }
            assert!(count > 1000);
            change /= count as f32;
            assert!(
                change > 0.15,
                "family {family} needs visible hue travel: {change}"
            );
            report.push_str(&format!(
                "scene {family}, color step {index}: {change:.3}\n"
            ));
        }
    }
    let mut clock = super::super::palette::ColorMotion::default();
    let mut colors = palette_for(PaletteName::Electric, MusicState::Groove);
    for index in 0..180 {
        let name = [
            PaletteName::Electric,
            PaletteName::Sunset,
            PaletteName::Neon,
            PaletteName::Ocean,
        ][index / 45];
        super::super::palette::smooth_palette(
            &mut colors,
            palette_for(name, MusicState::Groove),
            1.0 / 15.0,
        );
        let mut uniforms = fixture(
            27,
            4.0,
            1.0,
            (640, 360),
            SmoothedVisualState::default(),
            0.0,
            2.5,
        );
        uniforms.visual[3] = 0.65;
        uniforms.signal_history = [[0.6, 0.5, 0.15, 0.0]; 32];
        [
            uniforms.color_a,
            uniforms.color_b,
            uniforms.color_c,
            uniforms.color_d,
        ] = colors;
        uniforms.chromatic = [clock.update(1.0 / 15.0, 0.7, 0.1, 1.0), 1.0, 0.0, 0.0];
        stage.draw(
            uniforms,
            &target,
            Some(&output.join(format!("color-motion-{index:03}.ppm"))),
        );
    }
    fs::write(output.join("color-response.txt"), report).unwrap();
}

#[test]
#[ignore = "requires a native GPU; checks mixed-scene dissolve endpoints and linear image blending"]
fn native_transition_audition() {
    let output = std::env::var_os("PULSEBRIDGE_AUDITION_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("target/audition"));
    fs::create_dir_all(&output).unwrap();
    let mut stage = pollster::block_on(Stage::new());
    let target = stage.target(640, 360);
    let pixels = |path: &Path| {
        let bytes = fs::read(path).unwrap();
        bytes[bytes.len() - 640 * 360 * 3..].to_vec()
    };
    let mut report = String::from("Each completed frame must equal a linear-light dissolve of the two independently rendered sRGB scenes (within 2 byte values). Fixed time and audio; changed incoming variation and palette.\n");
    for (from, to) in [(0, 27), (27, 0), (8, 26), (26, 8), (25, 30), (30, 25)] {
        let mut smoothed = SmoothedVisualState::default();
        smoothed.update(synthetic_frame(6.0), 0.1);
        let from_uniforms = fixture(from, 4.0, 1.0, (640, 360), smoothed, 0.0, 2.5);
        let mut to_uniforms = fixture(to, 4.0, 1.0, (640, 360), smoothed, 0.0, 2.5);
        to_uniforms.style_b[2] = 0.91;
        [
            to_uniforms.color_a,
            to_uniforms.color_b,
            to_uniforms.color_c,
            to_uniforms.color_d,
        ] = palette_for(PaletteName::Sunset, MusicState::Build);
        let a = output.join(format!("dissolve-{from}-{to}-before.ppm"));
        let b = output.join(format!("dissolve-{from}-{to}-after.ppm"));
        stage.draw(from_uniforms, &target, Some(&a));
        stage.draw(to_uniforms, &target, Some(&b));
        let (before, after) = (pixels(&a), pixels(&b));
        stage.draw(from_uniforms, &target, None);
        for (index, weight) in [(0, 0.0), (1, 0.25), (2, 0.5), (3, 0.75), (4, 1.0)] {
            let mut mixed = to_uniforms;
            mixed.style_a = [from as f32, to as f32, 1.0 - weight, weight];
            let file = output.join(format!("dissolve-{from}-{to}-check-{index}.ppm"));
            stage.draw(mixed, &target, Some(&file));
            let actual = pixels(&file);
            let max_error = actual
                .iter()
                .zip(before.iter().zip(&after))
                .map(|(actual, (a, b))| {
                    let decode = |value: u8| {
                        let x = value as f32 / 255.0;
                        if x <= 0.04045 {
                            x / 12.92
                        } else {
                            ((x + 0.055) / 1.055).powf(2.4)
                        }
                    };
                    let linear = decode(*a) * (1.0 - weight) + decode(*b) * weight;
                    let encoded = if linear <= 0.0031308 {
                        linear * 12.92
                    } else {
                        1.055 * linear.powf(1.0 / 2.4) - 0.055
                    };
                    (*actual as f32 - encoded * 255.0).abs()
                })
                .fold(0.0f32, f32::max);
            assert!(
                max_error <= 2.0,
                "dissolve {from}->{to} at {weight}: {max_error}"
            );
            report.push_str(&format!(
                "{from}->{to}, weight {weight}: max error {max_error:.1}\n"
            ));
        }
        stage.draw(from_uniforms, &target, None);
        for index in 0..60 {
            let progress = (index as f32 / 59.0).clamp(0.0, 1.0);
            let weight = progress.powi(3) * (progress * (progress * 6.0 - 15.0) + 10.0);
            let mut mixed = to_uniforms;
            mixed.style_a = [from as f32, to as f32, 1.0 - weight, weight];
            stage.draw(
                mixed,
                &target,
                Some(&output.join(format!("dissolve-{from}-{to}-{index:03}.ppm"))),
            );
        }
    }
    fs::write(output.join("transition-response.txt"), report).unwrap();
}

#[test]
#[ignore = "requires a native GPU; checks and captures every Tron look"]
fn native_tron_audition() {
    let output = std::path::PathBuf::from("target/tron-audition");
    fs::create_dir_all(&output).unwrap();
    let mut stage = pollster::block_on(Stage::new());
    let target = stage.target(640, 360);
    let pixels = |path: &Path| {
        let data = fs::read(path).unwrap();
        data[data.len() - 640 * 360 * 3..].to_vec()
    };
    let mut images = Vec::new();
    let mut failures = Vec::new();
    for id in 33..53 {
        let mut uniforms = fixture(
            id,
            4.0,
            0.5,
            (640, 360),
            SmoothedVisualState::default(),
            0.0,
            2.5,
        );
        uniforms.visual[3] = 0.65;
        uniforms.reactive = [0.0; 4];
        let file = output.join(format!("tron-{id}.ppm"));
        stage.draw(uniforms, &target, Some(&file));
        let baseline = pixels(&file);
        assert!(
            baseline.iter().filter(|v| **v > 40).count() > 1000,
            "look {id} must be visible"
        );
        assert!(!images.contains(&baseline), "look {id} must be distinct");
        images.push(baseline.clone());
        uniforms.pulse[3] = 1.0;
        stage.draw(uniforms, &target, Some(&file));
        assert_eq!(baseline, pixels(&file), "Tron must ignore white flashes");
        uniforms.pulse[3] = 0.0;
        // Compare normalized images with fixed time, camera, and exposure.
        // A brightness-only change cannot pass; old hits must still move objects
        // even after all live envelopes have returned to zero.
        let normalize = |rgb: &[u8]| {
            let values: Vec<f32> = rgb
                .chunks_exact(3)
                .map(|c| 0.2126 * c[0] as f32 + 0.7152 * c[1] as f32 + 0.0722 * c[2] as f32)
                .collect();
            let total: f32 = values.iter().sum();
            values
                .into_iter()
                .map(|v| v / total.max(0.001))
                .collect::<Vec<_>>()
        };
        let distance =
            |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(a, b)| (a - b).abs()).sum::<f32>();
        let neutral = normalize(&baseline);
        for lane in 0..3 {
            let mut previous = Vec::new();
            let mut strongest = 0.0f32;
            for (phase, center) in [0.22f32, 0.65].iter().enumerate() {
                uniforms.reactive = [0.0; 4];
                uniforms.signal_history = [[0.0; 4]; 32];
                for (index, sample) in uniforms.signal_history.iter_mut().enumerate() {
                    let age = index as f32 / 30.0;
                    sample[lane] = (-((age - center) / 0.17).powi(2)).exp() * 0.95;
                }
                let hit_file = output.join(format!("response-{id}-{lane}-{phase}.ppm"));
                stage.draw(uniforms, &target, Some(&hit_file));
                let actual = normalize(&pixels(&hit_file));
                let deformation = distance(&neutral, &actual);
                strongest = strongest.max(deformation);
                eprintln!("Tron {id} band {lane} wave {phase}: spatial change {deformation:.3}");
                if deformation <= 0.04 {
                    failures.push(format!(
                        "look {id} band {lane}: weak delayed response {deformation}"
                    ));
                }
                if phase > 0 && distance(&previous, &actual) <= 0.08 {
                    failures.push(format!("look {id} band {lane}: hit must travel"));
                }
                previous = actual;
            }
            if strongest <= 0.12 {
                failures.push(format!(
                    "look {id} band {lane}: weak deformation {strongest}"
                ));
            }
        }
        uniforms.signal_history = [[0.0; 4]; 32];
        uniforms.reactive = [0.0; 4];
        uniforms.style_b[3] = 1.0;
        stage.draw(uniforms, &target, Some(&file));
        assert!(
            pixels(&file).iter().all(|v| *v == 0),
            "blackout must be black"
        );
        uniforms.style_b[3] = 0.0;
        stage.draw(uniforms, &target, Some(&file));
        if id <= 48 {
            // Cycling mode must visit the exact held looks.
            uniforms.style_a = [32.0, 32.0, 1.0, 0.0];
            uniforms.spatial[0] = (id - 33) as f32 * 8.0 + 1.0;
            let cycle_file = output.join("cycle.ppm");
            stage.draw(uniforms, &target, Some(&cycle_file));
            uniforms.style_a = [id as f32, id as f32, 1.0, 0.0];
            stage.draw(uniforms, &target, Some(&file));
            assert_eq!(pixels(&cycle_file), pixels(&file));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("; "));
}

#[test]
#[ignore = "requires a native GPU; writes synthetic music-driven Tron motion clips"]
fn native_tron_motion_audition() {
    use crate::visuals::reaction_history::ReactionHistory;
    let output = std::path::PathBuf::from("target/tron-motion");
    fs::create_dir_all(&output).unwrap();
    let mut stage = pollster::block_on(Stage::new());
    let target = stage.target(640, 360);
    for id in [33, 35, 37, 41, 42, 44] {
        let mut history = ReactionHistory::default();
        let mut smoothed = SmoothedVisualState::default();
        let mut clock = 2.5;
        for frame_index in 0..96 {
            let dt = 1.0 / 24.0;
            let t = frame_index as f32 * dt;
            // Independent kick, snare/melodic movement, and offbeat hats. This
            // goes through production smoothing; no testing pulse enters the app.
            let kick = (-((t * 2.0).fract()) * 18.0).exp();
            let snare = (-((t * 2.0 + 0.5).fract()) * 15.0).exp();
            let hat = (-((t * 4.0 + 0.25).fract()) * 25.0).exp();
            let frame = VisualInputFrame {
                energy: 0.38 + kick * 0.45,
                sub: kick * 0.95,
                bass: 0.15 + kick * 0.8,
                mids: 0.2 + snare * 0.7,
                highs: 0.1 + hat * 0.8,
                beat_pulse: kick,
                beat_confidence: 0.95,
                onset: kick.max(snare).max(hat),
                impact: kick * 0.8,
                reactivity: 1.0,
                ..Default::default()
            };
            smoothed.update(frame, dt);
            history.update(
                [
                    smoothed.bass_hit,
                    smoothed.mid_motion,
                    smoothed.high_hit,
                    smoothed.energy_rise,
                ],
                dt,
            );
            clock += dt * (0.25 + smoothed.drive * 0.65);
            let mut uniforms = fixture(id, t, 0.5, (640, 360), smoothed, 0.0, clock);
            uniforms.visual[3] = 0.65;
            uniforms.signal_history = history.snapshot();
            uniforms.spatial[3] = history.fraction_seconds();
            stage.draw(
                uniforms,
                &target,
                Some(&output.join(format!("tron-{id}-{frame_index:03}.ppm"))),
            );
        }
    }
    // Submission-to-completion timings at HD, with no readback. This is a local
    // workload check, not a Windows, presentation, or discrete-GPU benchmark.
    let target_hd = stage.target(1280, 720);
    let mut report = String::from("Native GPU completion time, 1280x720, warmed, no readback:\n");
    for id in 33..45 {
        let mut uniforms = fixture(
            id,
            3.0,
            0.5,
            (1280, 720),
            SmoothedVisualState::default(),
            0.0,
            4.0,
        );
        uniforms.reactive = [0.6; 4];
        uniforms.signal_history = [[0.6; 4]; 32];
        stage.draw(uniforms, &target_hd, None);
        let mut times: Vec<f64> = (0..12)
            .map(|_| stage.draw(uniforms, &target_hd, None))
            .collect();
        times.sort_by(f64::total_cmp);
        report.push_str(&format!(
            "look {id}: median {:.2} ms, max {:.2} ms\n",
            times[6], times[11]
        ));
    }
    eprint!("{report}");
    fs::write(output.join("timing.txt"), report).unwrap();
}

#[test]
#[ignore = "requires a native GPU; verifies dimension-filtered Tron cycling including wraparound"]
fn native_dimension_cycle_audition() {
    let output = std::path::PathBuf::from("target/dimension-audition");
    fs::create_dir_all(&output).unwrap();
    let mut stage = pollster::block_on(Stage::new());
    let target = stage.target(320, 180);
    let variants: [Vec<u32>; 3] = [
        (0..16).collect(),
        vec![4, 5, 7, 10, 11, 12, 13, 14, 15],
        vec![0, 1, 2, 3, 6, 8, 9],
    ];
    for (mode, looks) in variants.iter().enumerate() {
        for chapter in 0..=looks.len() {
            let mut uniforms = fixture(
                32,
                1.0,
                0.5,
                (320, 180),
                SmoothedVisualState::default(),
                0.0,
                chapter as f32 * 8.0 + 1.0,
            );
            uniforms.chromatic[3] = mode as f32;
            let cycle = output.join("cycle.ppm");
            let held = output.join("held.ppm");
            stage.draw(uniforms, &target, Some(&cycle));
            let family = (33 + looks[chapter % looks.len()]) as f32;
            uniforms.style_a = [family, family, 1.0, 0.0];
            stage.draw(uniforms, &target, Some(&held));
            assert_eq!(
                fs::read(&cycle).unwrap(),
                fs::read(&held).unwrap(),
                "mode {mode} chapter {chapter}"
            );
        }
    }
}

#[test]
#[ignore = "requires a native GPU; captures the expanded 2D/3D library with synthetic music"]
fn native_expanded_motion_audition() {
    use crate::visuals::reaction_history::ReactionHistory;
    let output = std::path::PathBuf::from("target/expanded-motion");
    fs::create_dir_all(&output).unwrap();
    let mut stage = pollster::block_on(Stage::new());
    let target = stage.target(384, 216);
    let mut report = String::from("Synthetic build phrase; native GPU, 1280x720 warmed submission-to-completion times (no readback):\n");
    for id in 45..53 {
        let mut history = ReactionHistory::default();
        let mut smoothed = SmoothedVisualState::default();
        let mut clock = 2.5;
        for index in 0..60 {
            let dt = 1.0 / 24.0;
            let t = 5.0 + index as f32 * dt;
            smoothed.update(synthetic_frame(t), dt);
            history.update(
                [
                    smoothed.bass_hit,
                    smoothed.mid_motion,
                    smoothed.high_hit,
                    smoothed.energy_rise,
                ],
                dt,
            );
            clock += dt * (0.25 + smoothed.drive * 0.65);
            let mut uniforms = fixture(id, t, 0.5, (384, 216), smoothed, 0.0, clock);
            uniforms.visual[3] = 0.65;
            uniforms.signal_history = history.snapshot();
            uniforms.spatial[3] = history.fraction_seconds();
            stage.draw(
                uniforms,
                &target,
                Some(&output.join(format!("scene-{id}-{index:03}.ppm"))),
            );
        }
        let hd = stage.target(1280, 720);
        let mut uniforms = fixture(id, 7.0, 0.5, (1280, 720), smoothed, 0.0, clock);
        uniforms.signal_history = history.snapshot();
        stage.draw(uniforms, &hd, None);
        let mut times: Vec<_> = (0..12).map(|_| stage.draw(uniforms, &hd, None)).collect();
        times.sort_by(f64::total_cmp);
        report.push_str(&format!(
            "scene {id}: median {:.2}ms, max {:.2}ms\n",
            times[6], times[11]
        ));
    }
    eprint!("{report}");
    fs::write(output.join("timings.txt"), report).unwrap();
}
