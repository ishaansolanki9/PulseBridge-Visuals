use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AudioSourceKind {
    RekordboxProcess,
    RekordboxSession,
    OutputDevice,
    InputDevice,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioSourceInfo {
    pub id: String,
    pub name: String,
    pub kind: AudioSourceKind,
    pub detected: bool,
    pub is_default: bool,
    pub available: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub enum CaptureState {
    #[default]
    Stopped,
    Connecting,
    Listening,
    Recovering,
    Failed,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub enum CaptureRoute {
    #[default]
    None,
    RekordboxProcess,
    RekordboxSessionOutput,
    SelectedOutput,
    AutomaticOutput,
    DefaultOutputFallback,
    SystemOutputFallback,
    SelectedInput,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub enum SampleFlowState {
    #[default]
    Unavailable,
    Waiting,
    Flowing,
    Silent,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureStatus {
    pub state: CaptureState,
    pub route: CaptureRoute,
    pub sample_flow: SampleFlowState,
    pub rekordbox_detected: bool,
    pub rekordbox_session_detected: bool,
    pub capture_initialized: bool,
    pub packets_received: bool,
    pub non_silent_samples_received: bool,
    pub reactive_ready: bool,
    pub fallback_attempted: bool,
    pub preferred_route_failure: Option<String>,
    pub source_name: Option<String>,
    pub message: Option<String>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub format: Option<String>,
    pub captured_samples: u64,
    pub captured_frames: u64,
    pub dropped_samples: u64,
    pub rms: f32,
    pub peak: f32,
}

#[cfg(test)]
mod tests {
    use super::CaptureRoute;

    #[test]
    fn automatic_output_has_an_explicit_public_route() {
        assert_eq!(
            serde_json::to_string(&CaptureRoute::AutomaticOutput).unwrap(),
            "\"automaticOutput\""
        );
    }
}
