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
export type AudioSourceKind = "rekordboxProcess" | "outputDevice";
export type CaptureState =
  | "stopped"
  | "connecting"
  | "listening"
  | "recovering"
  | "failed"
  | "unsupported";
export type OutputMode = "reactive" | "ambient" | "black";

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
  sourceName: string | null;
  message: string | null;
  capturedSamples: number;
  droppedSamples: number;
}

export interface RuntimeSnapshot {
  running: boolean;
  lastError: string | null;
  settings: VisualSettings;
  audio: CaptureStatus;
  outputMode: OutputMode;
  reactive: boolean;
  audioAgeMs: number | null;
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
  sourceName: null,
  message: "Live Rekordbox capture is available in the Windows package",
  capturedSamples: 0,
  droppedSamples: 0,
};
