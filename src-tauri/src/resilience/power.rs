pub struct PerformancePowerGuard;

impl PerformancePowerGuard {
    pub fn acquire() -> Self {
        #[cfg(target_os = "windows")]
        unsafe {
            use windows::Win32::System::Power::{
                SetThreadExecutionState, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED,
            };
            let _ =
                SetThreadExecutionState(ES_CONTINUOUS | ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED);
        }
        Self
    }
}

impl Drop for PerformancePowerGuard {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        unsafe {
            use windows::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS};
            let _ = SetThreadExecutionState(ES_CONTINUOUS);
        }
    }
}
