use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AudioSourceKind {
    RekordboxProcess,
    OutputDevice,
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
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub enum CaptureState {
    #[default]
    Stopped,
    Connecting,
    Listening,
    Recovering,
    Failed,
    #[cfg(not(target_os = "windows"))]
    Unsupported,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureStatus {
    pub state: CaptureState,
    pub source_name: Option<String>,
    pub message: Option<String>,
    pub captured_samples: u64,
    pub dropped_samples: u64,
}
