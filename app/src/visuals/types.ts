export type VisualStyle = "auto" | "fluid" | "waves" | "pulse" | "tunnel" | "burst";
export type IntensityProfile = "chill" | "balanced" | "wild";
export type FlashProfile = "off" | "moderate" | "high";
export type PaletteName =
  | "auto"
  | "electric"
  | "neon"
  | "sunset"
  | "ocean"
  | "infrared"
  | "purpleBlue"
  | "warm"
  | "monochrome"
  | "rainbowFlow";
export type AudioSourceKind = "rekordboxProcess" | "outputDevice" | "inputDevice";
export type CaptureState =
  | "stopped"
  | "connecting"
  | "listening"
  | "recovering"
  | "failed"
  | "unsupported";
export type CaptureRoute = "none" | "rekordboxProcess" | "selectedOutput" | "defaultOutputFallback" | "systemOutputFallback" | "selectedInput";
export type SampleFlowState = "unavailable" | "waiting" | "flowing" | "silent";
export type OutputMode = "reactive" | "ambient" | "black";
export type RuntimeLifecycle = "stopped" | "starting" | "running" | "recovering" | "failed";
export type RendererLifecycle = "stopped" | "initializing" | "running" | "failed";
export type PhraseProvenance = "rekordbox" | "cueMarkers" | "audioInferred" | "unavailable";
export type PhraseKind = "intro" | "verse" | "up" | "chorus" | "down" | "bridge" | "outro" | "fill" | "unknown";

export interface VisualSettings {
  displayId: number;
  audioSourceId: string;
  pcmBufferSeconds: number;
  style: VisualStyle;
  intensity: IntensityProfile;
  palette: PaletteName;
  flash: FlashProfile;
  topmost: boolean;
  musicReactivity: number;
  motion: number;
  brightness: number;
  colorChange: number;
  flashStrength: number;
}

export interface DisplayInfo {
  id: number;
  name: string;
  width: number;
  height: number;
  scaleFactor: number;
  isPrimary: boolean;
}

export interface AudioSourceInfo {
  id: string;
  name: string;
  kind: AudioSourceKind;
  detected: boolean;
  isDefault: boolean;
  available: boolean;
}

export interface CaptureStatus {
  state: CaptureState;
  route: CaptureRoute;
  sampleFlow: SampleFlowState;
  rekordboxDetected: boolean;
  captureInitialized: boolean;
  packetsReceived: boolean;
  nonSilentSamplesReceived: boolean;
  reactiveReady: boolean;
  fallbackAttempted: boolean;
  preferredRouteFailure: string | null;
  sourceName: string | null;
  message: string | null;
  sampleRate: number | null;
  channels: number | null;
  format: string | null;
  capturedSamples: number;
  capturedFrames: number;
  droppedSamples: number;
  rms: number;
  peak: number;
}

export interface PhraseStatus {
  provenance: PhraseProvenance;
  phrase: PhraseKind | null;
  confidence: number | null;
  progress: number | null;
  stale: boolean;
  message: string;
}

export interface RendererStatus {
  state: RendererLifecycle;
  adapter: string | null;
  backend: string | null;
  softwareFallback: boolean;
  message: string | null;
}

export interface RuntimeSnapshot {
  lifecycle: RuntimeLifecycle;
  running: boolean;
  lastError: string | null;
  logPath: string | null;
  settings: VisualSettings;
  audio: CaptureStatus;
  phrase: PhraseStatus;
  renderer: RendererStatus;
  outputMode: OutputMode;
  reactive: boolean;
  audioAgeMs: number | null;
}

export type DiagnosticMode = "audioOnly" | "rendererOnly" | "fullStartup" | "safeRenderer";
export type DiagnosticVerdict = "pass" | "degraded" | "fail" | "cancelled";
export type DiagnosticStageStatus = "pending" | "running" | "pass" | "degraded" | "fail" | "cancelled";

export interface DiagnosticStageResult {
  stage: string;
  status: DiagnosticStageStatus;
  durationMs: number;
  code: string;
  message: string;
  details: unknown;
}

export interface DiagnosticReport {
  schemaVersion: number;
  reportId: string;
  sessionId: string | null;
  mode: DiagnosticMode;
  startedAt: string;
  durationMs: number;
  verdict: DiagnosticVerdict;
  failureStage: string | null;
  failureCode: string | null;
  summary: string;
  stages: DiagnosticStageResult[];
  audio: {
    processDetected: boolean;
    captureInitialized: boolean;
    packetsReceived: boolean;
    nonSilentSamplesReceived: boolean;
    reactiveReady: boolean;
    route: string | null;
    sampleRate: number | null;
    channels: number | null;
    format: string | null;
    capturedFrames: number;
    rms: number;
    peak: number;
  };
  renderer: {
    adapter: string | null;
    backend: string | null;
    driver: string | null;
    driverInfo: string | null;
    deviceType: string | null;
    surfaceFormat: string | null;
    presentMode: string | null;
    shaderValidated: boolean;
    pipelineCreated: boolean;
    surfaceTested: boolean;
    softwareFallback: boolean;
    safeMode: boolean;
  };
  logPath: string;
  reportPath: string | null;
}

export const defaultSettings: VisualSettings = {
  displayId: 0,
  audioSourceId: "process:auto",
  pcmBufferSeconds: 10,
  style: "auto",
  intensity: "balanced",
  palette: "auto",
  flash: "off",
  topmost: false,
  musicReactivity: 1,
  motion: 1,
  brightness: 1,
  colorChange: 1,
  flashStrength: 1,
};

export const stoppedCapture: CaptureStatus = {
  state: "unsupported",
  route: "none",
  sampleFlow: "unavailable",
  rekordboxDetected: false,
  captureInitialized: false,
  packetsReceived: false,
  nonSilentSamplesReceived: false,
  reactiveReady: false,
  fallbackAttempted: false,
  preferredRouteFailure: null,
  sourceName: null,
  message: "Live Rekordbox capture is available in the Windows package",
  sampleRate: null,
  channels: null,
  format: null,
  capturedSamples: 0,
  capturedFrames: 0,
  droppedSamples: 0,
  rms: 0,
  peak: 0,
};
