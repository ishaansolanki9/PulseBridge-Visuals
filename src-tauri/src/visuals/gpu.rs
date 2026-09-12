//! Shared GPU policy for live output, diagnostics, and the native audition.
use std::sync::atomic::{AtomicBool, Ordering};

static RENDERER_ACTIVE: AtomicBool = AtomicBool::new(false);

pub(super) fn instance_descriptor() -> wgpu::InstanceDescriptor {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    // Ship the compiler on the supported Windows MSVC targets. Auto otherwise
    // silently falls back to FXC when dxcompiler.dll is absent on the DJ laptop.
    // FXC can spend minutes compiling the expanded scene library.
    if cfg!(all(
        windows,
        not(target_arch = "aarch64"),
        target_env = "msvc"
    )) {
        descriptor.backend_options.dx12.shader_compiler = wgpu::Dx12Compiler::StaticDxc;
        // wgpu maps DEBUG to DXC -Od. That hands a huge unoptimized shader to
        // the driver even in `tauri dev`; WARP can stall on the very first draw.
        // Keep API/WGSL validation enabled, but compile shaders as in release.
        descriptor.flags.remove(wgpu::InstanceFlags::DEBUG);
    }
    descriptor
}

pub(super) fn create_instance() -> wgpu::Instance {
    let descriptor = instance_descriptor();
    crate::diagnostics::critical_event(
        "info",
        "renderer.compiler.selected",
        "GPU_COMPILER_SELECTED",
        "Configured native shader compiler",
        serde_json::json!({
            "dx12Compiler": format!("{:?}", descriptor.backend_options.dx12.shader_compiler),
            "instanceFlags": format!("{:?}", descriptor.flags),
        }),
    );
    wgpu::Instance::new(descriptor)
}

/// A timed-out caller cannot cancel a native driver call. Keep ownership with
/// that worker until it actually returns, preventing retries from piling up.
pub(super) struct RendererLease<'a>(&'a AtomicBool);

impl RendererLease<'static> {
    pub fn acquire() -> Result<Self, String> {
        Self::try_acquire(&RENDERER_ACTIVE)
    }
}

impl<'a> RendererLease<'a> {
    fn try_acquire(active: &'a AtomicBool) -> Result<Self, String> {
        active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Self(active))
            .map_err(|_| {
                "GPU_RENDERER_BUSY: the previous renderer is still running or finishing a GPU call. Wait for it to finish; if it remains stuck, close and reopen PulseBridge before retrying."
                    .to_string()
            })
    }
}

impl Drop for RendererLease<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timed_out_caller_cannot_start_another_gpu_worker() {
        let active = AtomicBool::new(false);
        std::thread::scope(|scope| {
            let lease = RendererLease::try_acquire(&active).unwrap();
            let (finish, wait) = std::sync::mpsc::channel();
            let worker = scope.spawn(move || {
                let _lease = lease;
                wait.recv().unwrap();
            });
            for _ in 0..5 {
                assert!(RendererLease::try_acquire(&active)
                    .err()
                    .unwrap()
                    .starts_with("GPU_RENDERER_BUSY:"));
            }
            finish.send(()).unwrap();
            worker.join().unwrap();
            assert!(RendererLease::try_acquire(&active).is_ok());
        });
    }

    #[test]
    fn gpu_lease_is_released_after_failure() {
        let active = AtomicBool::new(false);
        let _ = std::panic::catch_unwind(|| {
            let _lease = RendererLease::try_acquire(&active).unwrap();
            panic!("simulated GPU initialization failure");
        });
        assert!(RendererLease::try_acquire(&active).is_ok());
    }

    #[test]
    fn windows_msvc_uses_bundled_dxc() {
        if cfg!(all(
            windows,
            not(target_arch = "aarch64"),
            target_env = "msvc"
        )) {
            assert!(matches!(
                instance_descriptor().backend_options.dx12.shader_compiler,
                wgpu::Dx12Compiler::StaticDxc
            ));
            assert!(!instance_descriptor()
                .flags
                .contains(wgpu::InstanceFlags::DEBUG));
            assert_eq!(
                instance_descriptor()
                    .flags
                    .contains(wgpu::InstanceFlags::VALIDATION),
                wgpu::InstanceDescriptor::new_without_display_handle()
                    .flags
                    .contains(wgpu::InstanceFlags::VALIDATION),
            );
        }
    }
}
