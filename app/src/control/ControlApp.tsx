import { useEffect, useMemo, useState } from "react";

import { PerformanceCanvas } from "../visuals/PerformanceCanvas";
import { defaultSettings } from "../visuals/types";
import type {
  AudioSourceInfo,
  DisplayInfo,
  FlashProfile,
  IntensityProfile,
  OutputMode,
  PaletteName,
  RuntimeSnapshot,
  VisualSettings,
  VisualStyle,
} from "../visuals/types";
import { controlTransport, isNativeApp } from "./transport";

const styles: Array<{ id: VisualStyle; label: string; detail: string }> = [
  { id: "auto", label: "Auto", detail: "Directs the set" },
  { id: "fluid", label: "Fluid", detail: "Liquid color" },
  { id: "waves", label: "Waves", detail: "Layered motion" },
  { id: "pulse", label: "Pulse", detail: "Beat geometry" },
  { id: "tunnel", label: "Tunnel", detail: "Build energy" },
  { id: "burst", label: "Burst", detail: "Drop impact" },
];

const intensities: Array<{ id: IntensityProfile; label: string }> = [
  { id: "chill", label: "Chill" },
  { id: "balanced", label: "Balanced" },
  { id: "wild", label: "Wild" },
];

const flashes: Array<{ id: FlashProfile; label: string }> = [
  { id: "off", label: "Off" },
  { id: "moderate", label: "Moderate" },
  { id: "high", label: "High" },
];

const palettes: Array<{ id: PaletteName; label: string }> = [
  { id: "auto", label: "Auto · follows energy" },
  { id: "electric", label: "Electric" },
  { id: "neon", label: "Neon" },
  { id: "sunset", label: "Sunset" },
  { id: "ocean", label: "Ocean" },
  { id: "infrared", label: "Infrared" },
  { id: "purpleBlue", label: "Purple + blue" },
  { id: "warm", label: "Warm" },
  { id: "monochrome", label: "Monochrome" },
  { id: "rainbowFlow", label: "Rainbow flow" },
];

export function ControlApp() {
  const [settings, setSettings] = useState<VisualSettings>(defaultSettings);
  const [runtime, setRuntime] = useState<RuntimeSnapshot | null>(null);
  const [displays, setDisplays] = useState<DisplayInfo[]>([]);
  const [sources, setSources] = useState<AudioSourceInfo[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    void Promise.all([
      controlTransport.getDisplays(),
      controlTransport.getAudioSources(),
      controlTransport.getState(),
    ])
      .then(([nextDisplays, nextSources, nextRuntime]) => {
        if (!active) return;
        setDisplays(nextDisplays);
        setSources(nextSources);
        setRuntime(nextRuntime);
        setSettings(nextRuntime.settings);
      })
      .catch((reason: unknown) => active && setError(messageFrom(reason)));
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    const stateTimer = window.setInterval(() => {
      void controlTransport.getState().then(setRuntime).catch(() => undefined);
    }, 600);
    const sourceTimer = window.setInterval(() => {
      if (!runtime?.running) {
        void controlTransport.getAudioSources().then(setSources).catch(() => undefined);
      }
    }, 1500);
    return () => {
      window.clearInterval(stateTimer);
      window.clearInterval(sourceTimer);
    };
  }, [runtime?.running]);

  const activeDisplay = useMemo(
    () => displays.find((display) => display.id === settings.displayId) ?? displays[0],
    [displays, settings.displayId],
  );
  const selectedSource = sources.find((source) => source.id === settings.audioSourceId);
  const canStart = Boolean(isNativeApp && activeDisplay && selectedSource?.available);
  const status = runtimeStatus(runtime);

  const changeSettings = (change: Partial<VisualSettings>) => {
    const next = { ...settings, ...change };
    setSettings(next);
    setRuntime((current) => current ? { ...current, settings: next } : current);
    setError(null);
    void controlTransport.updateSettings(next).catch((reason: unknown) => setError(messageFrom(reason)));
  };

  const toggleOutput = async () => {
    setBusy(true);
    setError(null);
    try {
      if (runtime?.running) await controlTransport.stop();
      else await controlTransport.start(settings);
      setRuntime(await controlTransport.getState());
    } catch (reason) {
      setError(messageFrom(reason));
    } finally {
      setBusy(false);
    }
  };

  const setOutputMode = async (mode: OutputMode) => {
    setError(null);
    try {
      await controlTransport.setOutputMode(mode);
      setRuntime(await controlTransport.getState());
    } catch (reason) {
      setError(messageFrom(reason));
    }
  };

  const refreshSources = () => {
    void controlTransport.getAudioSources().then(setSources).catch((reason: unknown) => setError(messageFrom(reason)));
  };

  return (
    <main className="control-shell">
      <header className="control-header">
        <div className="brand">
          <BrandMark />
          <div><strong>PulseBridge</strong><span>Visuals</span></div>
        </div>
        <div className={`output-state state-${status.tone}`}>
          <i />{status.label}
        </div>
      </header>

      <section className="preview-card" aria-label="Ambient visual preview">
        <PerformanceCanvas settings={settings} className="control-preview" />
        <div className="preview-topline">
          <span>Ambient preview</span>
          <span>No audio injected</span>
        </div>
        <div className="preview-caption">
          <span>Live response activates in the Windows app</span>
          <strong>{settings.style === "auto" ? "Auto directing" : styles.find((style) => style.id === settings.style)?.label}</strong>
        </div>
      </section>

      <section className="control-card" aria-label="Visual controls">
        <div className="setup-grid">
          <label className="field">
            <span>Output display</span>
            <select
              value={settings.displayId}
              onChange={(event) => changeSettings({ displayId: Number(event.target.value) })}
              disabled={runtime?.running}
            >
              {displays.map((display) => (
                <option key={display.id} value={display.id}>
                  {display.name} · {display.width}×{display.height}{display.isPrimary ? " · primary" : ""}
                </option>
              ))}
            </select>
          </label>
          <label className="field source-field">
            <span>Live audio source</span>
            <div className="select-with-action">
              <select
                value={settings.audioSourceId}
                onChange={(event) => changeSettings({ audioSourceId: event.target.value })}
                disabled={runtime?.running || sources.length === 0}
              >
                {sources.length === 0 && <option value="process:auto">Windows package required</option>}
                {sources.map((source) => (
                  <option key={source.id} value={source.id} disabled={!source.available}>
                    {source.name}{source.isDefault ? " · default" : ""}
                  </option>
                ))}
              </select>
              <button type="button" onClick={refreshSources} disabled={runtime?.running} aria-label="Refresh audio sources">↻</button>
            </div>
          </label>
        </div>

        <div className="source-status" data-tone={selectedSource?.available ? "good" : "waiting"}>
          <i />
          <span>{sourceMessage(selectedSource, isNativeApp)}</span>
        </div>

        <div className="control-divider" />

        <fieldset className="choice-group style-group">
          <legend>Visual style</legend>
          <div className="style-options">
            {styles.map((style) => (
              <button
                type="button"
                key={style.id}
                className={settings.style === style.id ? "is-selected" : ""}
                onClick={() => changeSettings({ style: style.id })}
              >
                <StyleGlyph style={style.id} />
                <span>{style.label}</span>
                <small>{style.detail}</small>
              </button>
            ))}
          </div>
        </fieldset>

        <div className="control-grid three-up">
          <fieldset className="choice-group">
            <legend>Intensity</legend>
            <div className="segmented-control">
              {intensities.map((intensity) => (
                <button key={intensity.id} type="button" className={settings.intensity === intensity.id ? "is-selected" : ""} onClick={() => changeSettings({ intensity: intensity.id })}>
                  {intensity.label}
                </button>
              ))}
            </div>
          </fieldset>
          <label className="field">
            <span>Palette</span>
            <select value={settings.palette} onChange={(event) => changeSettings({ palette: event.target.value as PaletteName })}>
              {palettes.map((palette) => <option key={palette.id} value={palette.id}>{palette.label}</option>)}
            </select>
          </label>
          <fieldset className="choice-group">
            <legend>White flash</legend>
            <div className="segmented-control">
              {flashes.map((flash) => (
                <button key={flash.id} type="button" className={settings.flash === flash.id ? "is-selected" : ""} onClick={() => changeSettings({ flash: flash.id })}>
                  {flash.label}
                </button>
              ))}
            </div>
          </fieldset>
        </div>

        <div className="safety-row">
          <label className="switch-setting">
            <button type="button" role="switch" aria-checked={settings.topmost} className={settings.topmost ? "switch is-on" : "switch"} onClick={() => changeSettings({ topmost: !settings.topmost })}>
              <i />
            </button>
            <span><strong>Keep output on top</strong><small>For a dedicated display or projector</small></span>
          </label>
          <div className="safety-note"><strong>Flash safety</strong><small>Off is the default. Motion impacts stay active without white flashes.</small></div>
        </div>

        <details className="advanced-controls">
          <summary>Advanced tuning</summary>
          <div className="slider-grid">
            <RangeSetting label="Music reactivity" value={settings.musicReactivity} min={0} max={1.5} onChange={(musicReactivity) => changeSettings({ musicReactivity })} />
            <RangeSetting label="Motion" value={settings.motion} min={0.25} max={1.75} onChange={(motion) => changeSettings({ motion })} />
            <RangeSetting label="Brightness" value={settings.brightness} min={0.25} max={1.25} onChange={(brightness) => changeSettings({ brightness })} />
            <RangeSetting label="Color change" value={settings.colorChange} min={0} max={1.5} onChange={(colorChange) => changeSettings({ colorChange })} />
            <RangeSetting label="Flash strength" value={settings.flashStrength} min={0} max={1} onChange={(flashStrength) => changeSettings({ flashStrength })} disabled={settings.flash === "off"} />
            <label className="field compact-field">
              <span>PCM safety buffer</span>
              <select value={settings.pcmBufferSeconds} onChange={(event) => changeSettings({ pcmBufferSeconds: Number(event.target.value) })} disabled={runtime?.running}>
                {[5, 10, 15, 20, 30].map((seconds) => <option key={seconds} value={seconds}>{seconds} seconds</option>)}
              </select>
            </label>
          </div>
        </details>
      </section>

      {runtime?.running && (
        <section className="live-safety" aria-label="Live output safety controls">
          <div><strong>{runtime.audio.sourceName ?? "Connecting audio"}</strong><small>{runtime.audio.message ?? `${runtime.reactive ? "Reacting to live audio" : "Ambient fallback"} · ${runtime.outputMode}`}</small></div>
          <div className="mode-actions">
            <button type="button" className={runtime.outputMode === "reactive" ? "is-active" : ""} onClick={() => void setOutputMode("reactive")}>Reactive</button>
            <button type="button" className={runtime.outputMode === "ambient" ? "is-active" : ""} onClick={() => void setOutputMode("ambient")}>Ambient</button>
            <button type="button" className={runtime.outputMode === "black" ? "black-button is-active" : "black-button"} onClick={() => void setOutputMode("black")}>Black screen</button>
          </div>
        </section>
      )}

      {(error || runtime?.lastError) && <p className="control-error" role="alert">{error ?? runtime?.lastError}</p>}

      <footer className="launch-row">
        <div>
          <span>{activeDisplay?.name ?? "Finding displays…"}</span>
          <small>{launchHint(runtime, selectedSource, isNativeApp)}</small>
        </div>
        <button
          className={`launch-button ${runtime?.running ? "is-stop" : ""}`}
          type="button"
          onClick={() => void toggleOutput()}
          disabled={busy || (!runtime?.running && !canStart)}
        >
          <i>{runtime?.running ? <StopIcon /> : <PlayIcon />}</i>
          <span>{busy ? "Working…" : runtime?.running ? "Stop visuals" : "Start live visuals"}</span>
        </button>
      </footer>
    </main>
  );
}

function RangeSetting({ label, value, min, max, onChange, disabled = false }: { label: string; value: number; min: number; max: number; onChange: (value: number) => void; disabled?: boolean }) {
  return (
    <label className="range-setting">
      <span>{label}<output>{Math.round(value * 100)}%</output></span>
      <input type="range" min={min} max={max} step="0.05" value={value} disabled={disabled} onChange={(event) => onChange(Number(event.target.value))} />
    </label>
  );
}

function runtimeStatus(runtime: RuntimeSnapshot | null) {
  if (!runtime) return { label: "Checking", tone: "idle" };
  if (!runtime.running) return { label: "Ready", tone: "idle" };
  if (runtime.outputMode === "black") return { label: "Black screen", tone: "warning" };
  if (runtime.audio.state === "recovering" || runtime.audio.state === "connecting") return { label: "Reconnecting", tone: "warning" };
  if (runtime.reactive) return { label: "Audio reactive", tone: "live" };
  return { label: "Ambient", tone: "ambient" };
}

function sourceMessage(source: AudioSourceInfo | undefined, native: boolean) {
  if (!native) return "Transfer and install the Windows package to connect Rekordbox.";
  if (!source) return "Choose an available audio source.";
  if (source.kind === "rekordboxProcess" && !source.detected) return "Open Rekordbox, then refresh. Windows-output fallback remains available below.";
  if (source.kind === "rekordboxProcess") return "Rekordbox detected · captures only its process when Windows supports it.";
  return `Output loopback ready${source.isDefault ? " · current Windows default" : ""}.`;
}

function launchHint(runtime: RuntimeSnapshot | null, source: AudioSourceInfo | undefined, native: boolean) {
  if (runtime?.running) return runtime.audio.message ?? `${runtime.audio.state} · full screen`;
  if (!native) return "Ambient preview only here · live capture is packaged for Windows";
  if (!source?.available) return "Open Rekordbox or select an available Windows output";
  return `${source.name} · ready for real audio`;
}

function messageFrom(reason: unknown) {
  return reason instanceof Error ? reason.message : String(reason);
}

function BrandMark() {
  return (
    <svg className="brand-mark" viewBox="0 0 44 44" aria-hidden="true">
      <path d="M5 26c5 0 5-14 10-14s5 22 10 22 5-17 14-17" />
      <circle cx="5" cy="26" r="2" /><circle cx="39" cy="17" r="2" />
    </svg>
  );
}

function StyleGlyph({ style }: { style: VisualStyle }) {
  if (style === "fluid") return <svg viewBox="0 0 34 20" aria-hidden="true"><path d="M2 15C8 3 15 21 21 8c3-6 7-5 11-2" /></svg>;
  if (style === "waves") return <svg viewBox="0 0 34 20" aria-hidden="true"><path d="M1 7c5-7 8 7 13 0s8 7 13 0 5 0 6 2M1 14c5-7 8 7 13 0s8 7 13 0 5 0 6 2" /></svg>;
  if (style === "pulse") return <svg viewBox="0 0 34 20" aria-hidden="true"><circle cx="17" cy="10" r="3" /><circle cx="17" cy="10" r="8" /></svg>;
  if (style === "tunnel") return <svg viewBox="0 0 34 20" aria-hidden="true"><path d="M3 2h28l-9 16H12Zm9 0 5 8 5-8m-10 16 5-8 5 8" /></svg>;
  if (style === "burst") return <svg viewBox="0 0 34 20" aria-hidden="true"><path d="m17 1 2 6 6-4-3 6 8 1-8 1 3 6-6-4-2 6-2-6-6 4 3-6-8-1 8-1-3-6 6 4Z" /></svg>;
  return <svg viewBox="0 0 34 20" aria-hidden="true"><path d="M2 15c5-12 8 3 13-7 4-8 7 8 11 0 2-4 4-3 6-2" /><circle cx="8" cy="7" r="2" /></svg>;
}

function PlayIcon() {
  return <svg viewBox="0 0 20 20" aria-hidden="true"><path d="m7 4 9 6-9 6Z" /></svg>;
}

function StopIcon() {
  return <svg viewBox="0 0 20 20" aria-hidden="true"><rect x="5" y="5" width="10" height="10" rx="1" /></svg>;
}
