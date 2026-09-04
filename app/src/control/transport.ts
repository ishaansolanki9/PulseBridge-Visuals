import { defaultSettings, stoppedCapture } from "../visuals/types";
import type {
  AudioSourceInfo,
  DiagnosticMode,
  DiagnosticReport,
  DisplayInfo,
  OutputMode,
  RuntimeSnapshot,
  VisualSettings,
} from "../visuals/types";

interface ControlTransport {
  getDisplays(): Promise<DisplayInfo[]>;
  getAudioSources(): Promise<AudioSourceInfo[]>;
  getState(): Promise<RuntimeSnapshot>;
  updateSettings(settings: VisualSettings): Promise<void>;
  start(settings: VisualSettings): Promise<void>;
  stop(): Promise<void>;
  setOutputMode(mode: OutputMode): Promise<void>;
  openLogsFolder(): Promise<void>;
  runDiagnostic(mode: DiagnosticMode): Promise<DiagnosticReport>;
  cancelDiagnostic(): Promise<boolean>;
  getLatestDiagnostic(): Promise<DiagnosticReport | null>;
  getPreviousRunReport(): Promise<DiagnosticReport | null>;
}

export const visualSettingsStorageKey = "pulsebridge-visual-settings";
export const isNativeApp = Boolean(window.__TAURI_INTERNALS__);

function readBrowserSettings(): VisualSettings {
  try {
    const saved = JSON.parse(localStorage.getItem(visualSettingsStorageKey) ?? "{}") as Partial<VisualSettings>;
    return { ...defaultSettings, ...saved };
  } catch {
    return defaultSettings;
  }
}

class BrowserControlTransport implements ControlTransport {
  private settings = readBrowserSettings();

  async getDisplays() {
    return [{
      id: 0,
      name: "Current display",
      width: window.screen.width,
      height: window.screen.height,
      scaleFactor: window.devicePixelRatio,
      isPrimary: true,
    }];
  }

  async getAudioSources() {
    return [];
  }

  async getState(): Promise<RuntimeSnapshot> {
    return {
      lifecycle: "stopped",
      running: false,
      lastError: null,
      logPath: null,
      settings: this.settings,
      audio: stoppedCapture,
      phrase: {
        provenance: "unavailable",
        phrase: null,
        confidence: null,
        progress: null,
        stale: false,
        tempoBpm: null,
        beatConfidence: null,
        barPhase: null,
        structureModelReady: false,
        message: "Phrase direction is available in the desktop package",
      },
      renderer: {
        state: "stopped",
        adapter: null,
        backend: null,
        softwareFallback: false,
        message: null,
      },
      outputMode: "ambient",
      reactive: false,
      audioAgeMs: null,
    };
  }

  async updateSettings(settings: VisualSettings) {
    this.settings = settings;
    localStorage.setItem(visualSettingsStorageKey, JSON.stringify(settings));
  }

  async start(settings: VisualSettings) {
    await this.updateSettings(settings);
    throw new Error("Live Rekordbox capture starts from the desktop package");
  }

  async stop() {}

  async setOutputMode() {}

  async openLogsFolder() {}

  async runDiagnostic(): Promise<DiagnosticReport> {
    throw new Error("Connection diagnostics run in the desktop package");
  }

  async cancelDiagnostic() { return false; }

  async getLatestDiagnostic() { return null; }

  async getPreviousRunReport() { return null; }
}

class TauriControlTransport implements ControlTransport {
  async getDisplays() {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<DisplayInfo[]>("get_displays");
  }

  async getAudioSources() {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<AudioSourceInfo[]>("get_audio_sources");
  }

  async getState() {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<RuntimeSnapshot>("get_runtime_state");
  }

  async updateSettings(settings: VisualSettings) {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("update_visual_settings", { settings });
  }

  async start(settings: VisualSettings) {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("start_visuals", { settings });
  }

  async stop() {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("stop_visuals");
  }

  async setOutputMode(mode: OutputMode) {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("set_output_mode", { mode });
  }

  async openLogsFolder() {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("open_logs_folder");
  }

  async runDiagnostic(mode: DiagnosticMode) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<DiagnosticReport>("run_connection_diagnostic", { mode });
  }

  async cancelDiagnostic() {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<boolean>("cancel_connection_diagnostic");
  }

  async getLatestDiagnostic() {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<DiagnosticReport | null>("get_latest_diagnostic_report");
  }

  async getPreviousRunReport() {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<DiagnosticReport | null>("get_previous_run_report");
  }
}

export const controlTransport: ControlTransport = isNativeApp
  ? new TauriControlTransport()
  : new BrowserControlTransport();

export function performanceSettings(): VisualSettings {
  return readBrowserSettings();
}
