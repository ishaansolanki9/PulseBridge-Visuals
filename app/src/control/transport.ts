import { defaultSettings, stoppedCapture } from "../visuals/types";
import type {
  AudioSourceInfo,
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
      running: false,
      lastError: null,
      settings: this.settings,
      audio: stoppedCapture,
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
    throw new Error("Live Rekordbox capture starts from the Windows package");
  }

  async stop() {}

  async setOutputMode() {}
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
}

export const controlTransport: ControlTransport = isNativeApp
  ? new TauriControlTransport()
  : new BrowserControlTransport();

export function performanceSettings(): VisualSettings {
  return readBrowserSettings();
}
