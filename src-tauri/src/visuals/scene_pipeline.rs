//! One bounded native compiler worker; scene changes never compile on the draw
//! thread. Its shared lease survives a timeout until the driver actually exits.
use super::{scene_shader::SceneKey, *};
use std::collections::HashMap;

const MAX_CACHED_PIPELINES: usize = 16;
pub const GPU_STARTUP_TIMEOUT: Duration = Duration::from_secs(60);

type CompiledScene = (SceneKey, Result<wgpu::RenderPipeline, String>);

pub(super) struct ScenePipelines {
    requests: mpsc::SyncSender<SceneKey>,
    results: mpsc::Receiver<CompiledScene>,
    pending: Option<(SceneKey, Instant)>,
    cache: HashMap<SceneKey, (wgpu::RenderPipeline, u64)>,
    tick: u64,
}

impl ScenePipelines {
    pub fn new(
        device: wgpu::Device,
        layout: wgpu::PipelineLayout,
        stop: Arc<AtomicBool>,
        lease: Arc<gpu::RendererLease<'static>>,
    ) -> Result<Self, String> {
        let (requests, jobs) = mpsc::sync_channel::<SceneKey>(1);
        let (completed, results) = mpsc::channel();
        thread::Builder::new()
            .name("pulsebridge-scene-compiler".into())
            .spawn(move || {
                let _lease = lease;
                while let Ok(key) = jobs.recv() {
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        pollster::block_on(compile_scene(&device, &layout, key))
                    }))
                    .unwrap_or_else(|_| {
                        Err(format!(
                            "GPU_SCENE_COMPILE_FAILED: compiler panicked for {key:?}"
                        ))
                    });
                    if completed.send((key, result)).is_err() {
                        break;
                    }
                }
            })
            .map_err(|error| format!("GPU_SCENE_COMPILER_START_FAILED: {error}"))?;
        Ok(Self {
            requests,
            results,
            pending: None,
            cache: HashMap::new(),
            tick: 0,
        })
    }

    pub fn get(
        &mut self,
        keys: [SceneKey; 2],
    ) -> Result<Option<[wgpu::RenderPipeline; 2]>, String> {
        match self.results.try_recv() {
            Err(mpsc::TryRecvError::Disconnected) => {
                return Err("GPU_SCENE_COMPILER_FAILED: compiler worker exited".into())
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Ok((key, result)) => {
                self.pending = None;
                if self.cache.len() >= MAX_CACHED_PIPELINES {
                    if let Some(oldest) = self
                        .cache
                        .iter()
                        .filter(|(key, _)| !keys.contains(key))
                        .min_by_key(|(_, (_, tick))| tick)
                        .map(|(key, _)| *key)
                    {
                        self.cache.remove(&oldest);
                    }
                }
                self.cache.insert(key, (result?, self.tick));
            }
        }
        if let Some((key, started)) = self.pending {
            if started.elapsed() >= GPU_STARTUP_TIMEOUT {
                return Err(format!(
                    "GPU_SCENE_COMPILE_TIMEOUT: {key:?} exceeded {} seconds",
                    GPU_STARTUP_TIMEOUT.as_secs()
                ));
            }
        }
        self.tick += 1;
        for key in keys {
            if let Some((_, tick)) = self.cache.get_mut(&key) {
                *tick = self.tick;
            } else if self.pending.is_none() {
                self.requests
                    .try_send(key)
                    .map_err(|error| format!("GPU_SCENE_COMPILER_FAILED: {error}"))?;
                self.pending = Some((key, Instant::now()));
            }
        }
        Ok(self
            .cache
            .get(&keys[0])
            .zip(self.cache.get(&keys[1]))
            .map(|(a, b)| [a.0.clone(), b.0.clone()]))
    }

    pub fn prefetch(&mut self, key: SceneKey) -> Result<(), String> {
        if self.pending.is_none() && !self.cache.contains_key(&key) {
            self.requests
                .try_send(key)
                .map_err(|error| format!("GPU_SCENE_COMPILER_FAILED: {error}"))?;
            self.pending = Some((key, Instant::now()));
        }
        Ok(())
    }

    pub fn progress(&self) -> Option<String> {
        self.pending.map(|_| "Preparing scene visuals".to_string())
    }
}

pub(super) async fn compile_scene(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    key: SceneKey,
) -> Result<wgpu::RenderPipeline, String> {
    let source = key.source();
    let stage = diagnostics::begin_stage(
        "renderer.scene.compile",
        "GPU_SCENE_COMPILE_BEGIN",
        "Compiling the selected native scene",
        serde_json::json!({ "scene": format!("{key:?}"), "sourceBytes": source.len() }),
    );
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Specialized native scene"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let result = create_performance_pipeline(device, &shader, layout, INTERNAL_RENDER_FORMAT).await;
    let result = match scope.pop().await {
        Some(error) => Err(format!("GPU_SCENE_COMPILE_FAILED: {key:?}: {error}")),
        None => result,
    };
    match &result {
        Ok(_) => stage.pass(
            "GPU_SCENE_COMPILED",
            "Selected native scene compiled",
            serde_json::Value::Null,
        ),
        Err(error) => stage.error("GPU_SCENE_COMPILE_FAILED", error, serde_json::Value::Null),
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changing_scenes_cannot_queue_more_work_behind_a_blocked_compiler() {
        let (requests, jobs) = mpsc::sync_channel(1);
        let (completed, results) = mpsc::channel();
        let mut cache = ScenePipelines {
            requests,
            results,
            pending: None,
            cache: HashMap::new(),
            tick: 0,
        };
        assert!(cache
            .get([SceneKey::Held(0), SceneKey::Held(33)])
            .unwrap()
            .is_none());
        assert_eq!(jobs.try_recv().unwrap(), SceneKey::Held(0));
        for id in 1..=52 {
            assert!(cache.get([SceneKey::Held(id); 2]).unwrap().is_none());
            assert_eq!(jobs.try_recv(), Err(mpsc::TryRecvError::Empty));
        }
        completed
            .send((SceneKey::Held(0), Err("driver rejected the scene".into())))
            .unwrap();
        assert_eq!(
            cache.get([SceneKey::Held(1); 2]).unwrap_err(),
            "driver rejected the scene"
        );
        drop(completed);
        assert!(cache
            .get([SceneKey::Held(1); 2])
            .unwrap_err()
            .contains("compiler worker exited"));
    }
}
