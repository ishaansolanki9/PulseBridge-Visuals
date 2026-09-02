use std::{
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc::{self, SyncSender},
        Arc, Mutex, RwLock,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use bytemuck::{Pod, Zeroable};
use serde::Serialize;
use tauri::Window;
use wgpu::util::DeviceExt;

use crate::{
    analysis::{SharedAnalysis, VisualInputFrame},
    diagnostics::{self, DiagnosticRendererInfo},
    phrase::SharedPhrase,
};

use super::{
    intensity_ceiling, intensity_values, palette_for, smooth_palette, FlashEnvelope, PaletteName,
    SceneDirector, SmoothedVisualState, VisualSettings,
};

const FRAME_INTERVAL: Duration = Duration::from_nanos(22_222_222);
const MAX_RENDER_PIXELS: u64 = 2_560 * 1_440;
const MAIN_THREAD_SURFACE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RendererLifecycle {
    #[default]
    Stopped,
    Initializing,
    Running,
    Failed,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererStatus {
    pub state: RendererLifecycle,
    pub adapter: Option<String>,
    pub backend: Option<String>,
    pub software_fallback: bool,
    pub message: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RendererReady {
    pub adapter: String,
    pub backend: String,
    pub driver: String,
    pub driver_info: String,
    pub device_type: String,
    pub surface_format: String,
    pub present_mode: String,
    pub software_fallback: bool,
}

pub struct PreparedRendererSurface {
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    width: u32,
    height: u32,
}

pub fn prepare_renderer_surface(window: &Window) -> Result<PreparedRendererSurface, String> {
    let instance_stage = diagnostics::begin_stage(
        "renderer.instance",
        "GPU_INSTANCE_CREATE_BEGIN",
        "Creating the wgpu instance",
        serde_json::Value::Null,
    );
    let instance = match std::panic::catch_unwind(wgpu::Instance::default) {
        Ok(instance) => {
            instance_stage.pass(
                "GPU_INSTANCE_CREATED",
                "wgpu instance created",
                serde_json::Value::Null,
            );
            instance
        }
        Err(_) => {
            let message = "GPU_INSTANCE_CREATE_FAILED: wgpu instance creation panicked";
            instance_stage.error(
                "GPU_INSTANCE_CREATE_FAILED",
                message,
                serde_json::Value::Null,
            );
            return Err(message.to_string());
        }
    };
    let (surface, width, height) = create_surface_on_main_thread(&instance, window)?;
    Ok(PreparedRendererSurface {
        instance,
        surface,
        width,
        height,
    })
}

fn create_surface_on_main_thread(
    instance: &wgpu::Instance,
    window: &Window,
) -> Result<(wgpu::Surface<'static>, u32, u32), String> {
    let (sender, receiver) = mpsc::sync_channel(1);
    let task_instance = instance.clone();
    let task_window = window.clone();
    window
        .run_on_main_thread(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let size = task_window
                    .inner_size()
                    .map_err(|error| format!("GPU_SURFACE_CREATE_FAILED: {error}"))?;
                let surface_stage = diagnostics::begin_stage(
                    "renderer.surface",
                    "GPU_SURFACE_CREATE_BEGIN",
                    "Creating the performance surface on the platform UI thread",
                    serde_json::json!({ "width": size.width, "height": size.height }),
                );
                match task_instance.create_surface(task_window) {
                    Ok(surface) => {
                        surface_stage.pass(
                            "GPU_SURFACE_CREATED",
                            "Performance surface created on the platform UI thread",
                            serde_json::Value::Null,
                        );
                        Ok((surface, size.width, size.height))
                    }
                    Err(error) => {
                        let message = format!("GPU_SURFACE_CREATE_FAILED: {error}");
                        surface_stage.error(
                            "GPU_SURFACE_CREATE_FAILED",
                            &message,
                            serde_json::Value::Null,
                        );
                        Err(message)
                    }
                }
            }))
            .unwrap_or_else(|_| {
                Err(
                    "GPU_SURFACE_CREATE_FAILED: platform UI-thread surface creation panicked"
                        .to_string(),
                )
            });
            let _ = sender.send(result);
        })
        .map_err(|error| {
            format!("GPU_SURFACE_CREATE_FAILED: unable to schedule main-thread creation: {error}")
        })?;

    match receiver.recv_timeout(MAIN_THREAD_SURFACE_TIMEOUT) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(format!(
            "GPU_SURFACE_CREATE_FAILED: main-thread surface creation timed out after {} seconds",
            MAIN_THREAD_SURFACE_TIMEOUT.as_secs()
        )),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(
            "GPU_SURFACE_CREATE_FAILED: main-thread surface creation channel closed".to_string(),
        ),
    }
}

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
    scene: [f32; 4],
    modifiers: [f32; 4],
}

#[allow(clippy::too_many_arguments)]
pub fn run_renderer(
    window: Window,
    prepared_surface: PreparedRendererSurface,
    stop_requested: Arc<AtomicBool>,
    settings: Arc<RwLock<VisualSettings>>,
    analysis: SharedAnalysis,
    phrase: SharedPhrase,
    output_mode: Arc<AtomicU8>,
    ready: SyncSender<Result<RendererReady, String>>,
    status: Arc<Mutex<RendererStatus>>,
) -> Result<(), String> {
    set_renderer_status(
        &status,
        RendererStatus {
            state: RendererLifecycle::Initializing,
            message: Some("Initializing GPU surface".to_string()),
            ..Default::default()
        },
    );
    let error_sender = ready.clone();
    let result = pollster::block_on(Renderer::run(
        window,
        prepared_surface,
        stop_requested,
        settings,
        analysis,
        phrase,
        output_mode,
        ready,
        Arc::clone(&status),
    ));
    if let Err(error) = &result {
        let _ = error_sender.try_send(Err(error.clone()));
        let mut failed = lock_renderer_status(&status);
        failed.state = RendererLifecycle::Failed;
        failed.message = Some(error.clone());
        set_renderer_status(&status, failed);
    }
    result
}

struct Renderer {
    instance: wgpu::Instance,
    window: Window,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl Renderer {
    #[allow(clippy::too_many_arguments)]
    async fn run(
        window: Window,
        prepared_surface: PreparedRendererSurface,
        stop_requested: Arc<AtomicBool>,
        settings: Arc<RwLock<VisualSettings>>,
        analysis: SharedAnalysis,
        phrase: SharedPhrase,
        output_mode: Arc<AtomicU8>,
        ready: SyncSender<Result<RendererReady, String>>,
        status: Arc<Mutex<RendererStatus>>,
    ) -> Result<(), String> {
        let PreparedRendererSurface {
            instance,
            surface,
            width,
            height,
        } = prepared_surface;
        let adapter_stage = diagnostics::begin_stage(
            "renderer.adapter",
            "GPU_ADAPTER_REQUEST_BEGIN",
            "Selecting a compatible GPU adapter",
            serde_json::Value::Null,
        );
        let (adapter, software_fallback) = match request_adapter(&instance, &surface).await {
            Ok(result) => result,
            Err(error) => {
                adapter_stage.error("GPU_ADAPTER_NOT_FOUND", &error, serde_json::Value::Null);
                return Err(error);
            }
        };
        let adapter_info = adapter.get_info();
        let backend = format!("{:?}", adapter_info.backend);
        adapter_stage.pass(
            "GPU_ADAPTER_SELECTED",
            "Selected a compatible GPU adapter",
            serde_json::json!({
                "adapter": adapter_info.name,
                "backend": backend,
                "driver": adapter_info.driver,
                "driverInfo": adapter_info.driver_info,
                "deviceType": format!("{:?}", adapter_info.device_type),
                "softwareFallback": software_fallback,
            }),
        );
        set_renderer_status(
            &status,
            RendererStatus {
                state: RendererLifecycle::Initializing,
                adapter: Some(adapter_info.name.clone()),
                backend: Some(backend.clone()),
                software_fallback,
                message: Some("Creating GPU device and shader pipeline".to_string()),
            },
        );
        let device_stage = diagnostics::begin_stage(
            "renderer.device",
            "GPU_DEVICE_CREATE_BEGIN",
            "Requesting the renderer device and queue",
            serde_json::Value::Null,
        );
        let (device, queue) = match adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("PulseBridge Visuals GPU"),
                ..Default::default()
            })
            .await
        {
            Ok(device) => device,
            Err(error) => {
                let message = format!("GPU_DEVICE_CREATE_FAILED: {error}");
                device_stage.error(
                    "GPU_DEVICE_CREATE_FAILED",
                    &message,
                    serde_json::Value::Null,
                );
                return Err(message);
            }
        };
        device_stage.pass(
            "GPU_DEVICE_CREATED",
            "GPU device and queue created",
            serde_json::Value::Null,
        );
        install_device_error_handlers(&device, Arc::clone(&stop_requested), Arc::clone(&status));
        let window_width = width.max(1);
        let window_height = height.max(1);
        let (render_width, render_height) = performance_render_size(window_width, window_height);
        let config = surface
            .get_default_config(&adapter, render_width, render_height)
            .ok_or_else(|| "The selected display has no compatible GPU surface".to_string())?;
        surface.configure(&device, &config);

        let shader_stage = diagnostics::begin_stage(
            "renderer.shader",
            "GPU_SHADER_VALIDATION_BEGIN",
            "Compiling and validating the performance shader",
            serde_json::Value::Null,
        );
        let shader_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("PulseBridge performance shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/performance.wgsl").into()),
        });
        if let Some(error) = shader_scope.pop().await {
            let message = format!("GPU_SHADER_VALIDATION_FAILED: {error}");
            shader_stage.error(
                "GPU_SHADER_VALIDATION_FAILED",
                &message,
                serde_json::Value::Null,
            );
            return Err(message);
        }
        shader_stage.pass(
            "GPU_SHADER_VALIDATED",
            "Performance shader validated",
            serde_json::Value::Null,
        );
        let pipeline_stage = diagnostics::begin_stage(
            "renderer.pipeline",
            "GPU_PIPELINE_CREATE_BEGIN",
            "Creating renderer resources and pipeline",
            serde_json::Value::Null,
        );
        let pipeline_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
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
        if let Some(error) = pipeline_scope.pop().await {
            let message = format!("GPU_PIPELINE_CREATE_FAILED: {error}");
            pipeline_stage.error(
                "GPU_PIPELINE_CREATE_FAILED",
                &message,
                serde_json::Value::Null,
            );
            return Err(message);
        }
        pipeline_stage.pass(
            "GPU_PIPELINE_CREATED",
            "Renderer resources and pipeline created",
            serde_json::json!({
                "surfaceFormat": format!("{:?}", config.format),
                "presentMode": format!("{:?}", config.present_mode),
                "windowSize": [window_width, window_height],
                "renderSize": [render_width, render_height],
                "targetFps": 45,
            }),
        );

        let mut renderer = Self {
            instance,
            window: window.clone(),
            surface,
            device,
            queue,
            config,
            pipeline,
            uniform_buffer,
            bind_group,
        };
        diagnostics::event("info", "renderer.pipeline.ready", "GPU pipeline created");
        let started_at = Instant::now();
        let session_seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let mut director = SceneDirector::new(session_seed);
        let mut last_frame = started_at;
        let mut next_frame = started_at;
        let mut smoothed = SmoothedVisualState::default();
        let mut flash_envelope = FlashEnvelope::default();
        let initial_settings = read_settings(&settings);
        let mut smoothed_palette =
            palette_for(initial_settings.palette, VisualInputFrame::default().state);
        let mut first_present_stage = Some(diagnostics::begin_stage(
            "renderer.firstPresent",
            "GPU_FIRST_PRESENT_BEGIN",
            "Waiting for the first valid GPU present",
            serde_json::Value::Null,
        ));
        let mut readiness = Some((
            ready,
            RendererReady {
                adapter: adapter_info.name,
                backend,
                driver: adapter_info.driver,
                driver_info: adapter_info.driver_info,
                device_type: format!("{:?}", adapter_info.device_type),
                surface_format: format!("{:?}", renderer.config.format),
                present_mode: format!("{:?}", renderer.config.present_mode),
                software_fallback,
            },
        ));

        while !stop_requested.load(Ordering::Acquire) {
            if let Some(shortcut) = native_exit_shortcut(&window) {
                diagnostics::critical_event(
                    "info",
                    "performance.shortcut.stop",
                    "PERFORMANCE_SHORTCUT_STOP",
                    &format!("Stopping performance output from {shortcut}"),
                    serde_json::json!({ "shortcut": shortcut }),
                );
                stop_requested.store(true, Ordering::Release);
                break;
            }
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
            smoothed.update(frame, delta);

            let size = window.inner_size().map_err(|error| error.to_string())?;
            let (render_width, render_height) =
                performance_render_size(size.width.max(1), size.height.max(1));
            if size.width > 0
                && size.height > 0
                && (render_width != renderer.config.width
                    || render_height != renderer.config.height)
            {
                renderer.config.width = render_width;
                renderer.config.height = render_height;
                renderer
                    .surface
                    .configure(&renderer.device, &renderer.config);
                diagnostics::event(
                    "info",
                    "renderer.surface.resize",
                    &format!(
                        "window={}x{} render={}x{}",
                        size.width, size.height, render_width, render_height
                    ),
                );
            }

            let phrase_context = phrase
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone();
            let scene = director.update(
                elapsed,
                now,
                frame,
                &phrase_context,
                current_settings.style,
                current_settings.intensity,
            );
            let intensities = intensity_values(current_settings.intensity);
            let drive = (smoothed.drive
                * intensity_ceiling(current_settings.intensity)
                * current_settings.music_reactivity)
                .clamp(0.0, 1.0);
            let directed_palette = if current_settings.palette == PaletteName::Auto {
                scene.palette
            } else {
                current_settings.palette
            };
            let target_palette = palette_for(directed_palette, frame.state);
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
                    (0.12 + drive * 1.38) * intensities[0] * current_settings.motion * scene.motion,
                    (0.22 + drive * 0.78) * intensities[1] * scene.detail,
                    0.92 + drive * 0.34 + smoothed.bass * 0.12,
                    (0.3 + drive * 0.7)
                        * intensities[2]
                        * current_settings.brightness
                        * scene.brightness,
                ],
                color_a: smoothed_palette[0],
                color_b: smoothed_palette[1],
                color_c: smoothed_palette[2],
                color_d: smoothed_palette[3],
                style_a: [
                    scene.primary.id(),
                    scene
                        .secondary
                        .map_or(scene.primary.id(), |family| family.id()),
                    scene.primary_mix,
                    scene.secondary_mix,
                ],
                style_b: [
                    0.82 + drive * 0.18,
                    drive,
                    (scene.variation_seed & 0x00ff_ffff) as f32 / 16_777_215.0,
                    (output_mode_value == 2) as u8 as f32,
                ],
                effects: [
                    smoothed.sub,
                    smoothed.impact * intensities[3],
                    current_settings.color_change * (0.8 + drive * 1.7),
                    drive,
                ],
                scene: [scene.motion, scene.detail, scene.density, scene.brightness],
                modifiers: [
                    scene.modifiers[0].kind.map_or(-1.0, |kind| kind.id()),
                    scene.modifiers[0].strength,
                    scene.modifiers[1].kind.map_or(-1.0, |kind| kind.id()),
                    scene.modifiers[1].strength,
                ],
            };
            let presented = renderer.render(uniforms)?;
            if presented {
                if let Some((sender, info)) = readiness.take() {
                    if let Some(stage) = first_present_stage.take() {
                        stage.pass(
                            "GPU_FIRST_FRAME_PRESENTED",
                            "First GPU frame presented",
                            serde_json::Value::Null,
                        );
                    }
                    set_renderer_status(
                        &status,
                        RendererStatus {
                            state: RendererLifecycle::Running,
                            adapter: Some(info.adapter.clone()),
                            backend: Some(info.backend.clone()),
                            software_fallback: info.software_fallback,
                            message: None,
                        },
                    );
                    let _ = sender.send(Ok(info));
                }
            }

            next_frame += FRAME_INTERVAL;
            let after_render = Instant::now();
            if next_frame > after_render {
                thread::sleep(next_frame - after_render);
            } else {
                next_frame = after_render;
            }
        }

        if let Some((sender, _)) = readiness.take() {
            if let Some(stage) = first_present_stage.take() {
                stage.error(
                    "GPU_FIRST_FRAME_TIMEOUT",
                    "Renderer stopped before the first valid performance frame",
                    serde_json::Value::Null,
                );
            }
            let _ = sender.send(Err(
                "Renderer stopped before the first valid performance frame".to_string(),
            ));
        }
        Ok(())
    }

    fn render(&mut self, uniforms: VisualUniforms) -> Result<bool, String> {
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                self.surface.configure(&self.device, &self.config);
                frame
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                diagnostics::critical_event(
                    "warn",
                    "renderer.surface.lost",
                    "GPU_SURFACE_LOST",
                    "Recreating a lost GPU surface",
                    serde_json::Value::Null,
                );
                let (surface, width, height) =
                    create_surface_on_main_thread(&self.instance, &self.window)?;
                self.surface = surface;
                self.config.width = width.max(1);
                self.config.height = height.max(1);
                self.surface.configure(&self.device, &self.config);
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("The GPU rejected a performance frame".to_string());
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
        Ok(true)
    }
}

#[cfg(target_os = "windows")]
fn native_exit_shortcut(window: &Window) -> Option<&'static str> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VIRTUAL_KEY, VK_CONTROL, VK_ESCAPE, VK_F12, VK_MENU, VK_SHIFT,
    };

    let down = |key: VIRTUAL_KEY| unsafe { GetAsyncKeyState(key.0 as i32) } < 0;
    if window.is_focused().unwrap_or(false) && down(VK_ESCAPE) {
        return Some("Escape");
    }
    if down(VK_CONTROL) && down(VK_MENU) && down(VK_SHIFT) && down(VK_F12) {
        return Some("Ctrl+Alt+Shift+F12 emergency shortcut");
    }
    None
}

#[cfg(target_os = "macos")]
fn native_exit_shortcut(window: &Window) -> Option<&'static str> {
    const COMBINED_SESSION_STATE: i32 = 0;
    const ESCAPE: u16 = 53;
    const CONTROL: u16 = 59;
    const OPTION: u16 = 58;
    const SHIFT: u16 = 56;
    const F12: u16 = 111;
    let down = |key| unsafe { CGEventSourceKeyState(COMBINED_SESSION_STATE, key) };
    if window.is_focused().unwrap_or(false) && down(ESCAPE) {
        return Some("Escape");
    }
    if down(CONTROL) && down(OPTION) && down(SHIFT) && down(F12) {
        return Some("Control+Option+Shift+F12 emergency shortcut");
    }
    None
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceKeyState(state_id: i32, key: u16) -> bool;
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn native_exit_shortcut(_window: &Window) -> Option<&'static str> {
    None
}

fn install_device_error_handlers(
    device: &wgpu::Device,
    stop_requested: Arc<AtomicBool>,
    status: Arc<Mutex<RendererStatus>>,
) {
    let uncaptured_stop = Arc::clone(&stop_requested);
    let uncaptured_status = Arc::clone(&status);
    device.on_uncaptured_error(Arc::new(move |error| {
        let message = format!("GPU_UNCAPTURED_ERROR: {error}");
        diagnostics::critical_event(
            "error",
            "renderer.uncapturedError",
            "GPU_UNCAPTURED_ERROR",
            &message,
            serde_json::Value::Null,
        );
        transition_renderer_failure(&uncaptured_stop, &uncaptured_status, message);
    }));
    device.set_device_lost_callback(move |reason, detail| {
        let message = format!("GPU_DEVICE_LOST: {reason:?}: {detail}");
        diagnostics::critical_event(
            "error",
            "renderer.deviceLost",
            "GPU_DEVICE_LOST",
            &message,
            serde_json::Value::Null,
        );
        transition_renderer_failure(&stop_requested, &status, message);
    });
}

fn transition_renderer_failure(
    stop_requested: &AtomicBool,
    status: &Mutex<RendererStatus>,
    message: String,
) {
    let mut current = lock_renderer_status(status);
    current.state = RendererLifecycle::Failed;
    current.message = Some(message);
    set_renderer_status(status, current);
    stop_requested.store(true, Ordering::Release);
}

pub fn probe_renderer(safe_mode: bool) -> Result<DiagnosticRendererInfo, String> {
    pollster::block_on(probe_renderer_async(safe_mode))
}

async fn probe_renderer_async(safe_mode: bool) -> Result<DiagnosticRendererInfo, String> {
    let instance = wgpu::Instance::default();
    let primary_options = wgpu::RequestAdapterOptions {
        power_preference: if safe_mode {
            wgpu::PowerPreference::LowPower
        } else {
            wgpu::PowerPreference::HighPerformance
        },
        compatible_surface: None,
        force_fallback_adapter: false,
        apply_limit_buckets: safe_mode,
    };
    let adapter = match instance.request_adapter(&primary_options).await {
        Ok(adapter) => adapter,
        Err(primary_error) if safe_mode => instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                force_fallback_adapter: true,
                ..primary_options
            })
            .await
            .map_err(|fallback_error| {
                format!(
                    "GPU_ADAPTER_NOT_FOUND: conservative adapter failed ({primary_error}); software fallback failed ({fallback_error})"
                )
            })?,
        Err(error) => return Err(format!("GPU_ADAPTER_NOT_FOUND: {error}")),
    };
    let info = adapter.get_info();
    let descriptor = if safe_mode {
        wgpu::DeviceDescriptor {
            label: Some("PulseBridge safe diagnostic GPU"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            ..Default::default()
        }
    } else {
        wgpu::DeviceDescriptor {
            label: Some("PulseBridge diagnostic GPU"),
            ..Default::default()
        }
    };
    let (device, _queue) = adapter
        .request_device(&descriptor)
        .await
        .map_err(|error| format!("GPU_DEVICE_CREATE_FAILED: {error}"))?;
    let lost = Arc::new(Mutex::new(None::<String>));
    let lost_callback = Arc::clone(&lost);
    device.set_device_lost_callback(move |reason, detail| {
        *lost_callback
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) =
            Some(format!("GPU_DEVICE_LOST: {reason:?}: {detail}"));
    });
    let uncaptured = Arc::new(Mutex::new(None::<String>));
    let uncaptured_callback = Arc::clone(&uncaptured);
    device.on_uncaptured_error(Arc::new(move |error| {
        *uncaptured_callback
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(error.to_string());
    }));

    let shader_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("PulseBridge diagnostic shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/performance.wgsl").into()),
    });
    if let Some(error) = shader_scope.pop().await {
        return Err(format!("GPU_SHADER_VALIDATION_FAILED: {error}"));
    }
    let pipeline_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Diagnostic uniform layout"),
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
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Diagnostic pipeline layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });
    let _pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("PulseBridge diagnostic pipeline"),
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
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });
    if let Some(error) = pipeline_scope.pop().await {
        return Err(format!("GPU_PIPELINE_CREATE_FAILED: {error}"));
    }
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(2)),
        })
        .map_err(|error| format!("GPU_DEVICE_POLL_FAILED: {error}"))?;
    if let Some(error) = lock_renderer_error(&lost) {
        return Err(error);
    }
    if let Some(error) = lock_renderer_error(&uncaptured) {
        return Err(format!("GPU_UNCAPTURED_ERROR: {error}"));
    }
    Ok(DiagnosticRendererInfo {
        adapter: Some(info.name),
        backend: Some(format!("{:?}", info.backend)),
        driver: Some(info.driver),
        driver_info: Some(info.driver_info),
        device_type: Some(format!("{:?}", info.device_type)),
        shader_validated: true,
        pipeline_created: true,
        software_fallback: info.device_type == wgpu::DeviceType::Cpu,
        safe_mode,
        ..Default::default()
    })
}

fn lock_renderer_error(value: &Mutex<Option<String>>) -> Option<String> {
    value
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

async fn request_adapter(
    instance: &wgpu::Instance,
    surface: &wgpu::Surface<'static>,
) -> Result<(wgpu::Adapter, bool), String> {
    let attempts = [
        (
            wgpu::PowerPreference::HighPerformance,
            false,
            "high-performance hardware",
        ),
        (
            wgpu::PowerPreference::LowPower,
            false,
            "compatible hardware",
        ),
        (wgpu::PowerPreference::LowPower, true, "software fallback"),
    ];
    let mut errors = Vec::new();
    for (preference, force_fallback_adapter, label) in attempts {
        match instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: preference,
                compatible_surface: Some(surface),
                force_fallback_adapter,
                apply_limit_buckets: false,
            })
            .await
        {
            Ok(adapter) => return Ok((adapter, force_fallback_adapter)),
            Err(error) => {
                diagnostics::event(
                    "warn",
                    "renderer.adapter.rejected",
                    &format!("{label}: {error}"),
                );
                errors.push(format!("{label}: {error}"));
            }
        }
    }
    Err(format!(
        "No compatible GPU adapter was available ({})",
        errors.join("; ")
    ))
}

fn performance_render_size(width: u32, height: u32) -> (u32, u32) {
    let width = width.max(1);
    let height = height.max(1);
    let pixels = u64::from(width) * u64::from(height);
    if pixels <= MAX_RENDER_PIXELS {
        return (width, height);
    }
    let scale = (MAX_RENDER_PIXELS as f64 / pixels as f64).sqrt();
    (
        (f64::from(width) * scale).round().max(1.0) as u32,
        (f64::from(height) * scale).round().max(1.0) as u32,
    )
}

fn read_settings(settings: &RwLock<VisualSettings>) -> VisualSettings {
    settings
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

fn lock_renderer_status(status: &Mutex<RendererStatus>) -> RendererStatus {
    status
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

fn set_renderer_status(status: &Mutex<RendererStatus>, next: RendererStatus) {
    *status
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = next;
}

#[cfg(test)]
mod tests {
    use std::sync::{atomic::AtomicBool, Mutex};

    use super::{
        performance_render_size, transition_renderer_failure, RendererLifecycle, RendererStatus,
    };

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

    #[test]
    fn renderer_failure_requests_an_orderly_stop_and_preserves_the_reason() {
        let stop = AtomicBool::new(false);
        let status = Mutex::new(RendererStatus {
            state: RendererLifecycle::Running,
            ..Default::default()
        });
        transition_renderer_failure(&stop, &status, "GPU_DEVICE_LOST: test".to_string());
        let current = status.lock().expect("renderer status");
        assert!(stop.load(std::sync::atomic::Ordering::Acquire));
        assert_eq!(current.state, RendererLifecycle::Failed);
        assert_eq!(current.message.as_deref(), Some("GPU_DEVICE_LOST: test"));
    }

    #[test]
    fn performance_surface_keeps_hd_and_caps_four_k_near_1440p() {
        assert_eq!(performance_render_size(1_920, 1_080), (1_920, 1_080));
        let (width, height) = performance_render_size(3_840, 2_160);
        assert_eq!((width, height), (2_560, 1_440));
    }
}
