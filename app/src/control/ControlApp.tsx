import { useEffect, useMemo, useState } from "react";

import { PerformanceCanvas } from "../visuals/PerformanceCanvas";
import { defaultSettings } from "../visuals/types";
import type {
  AudioSourceInfo,
  DiagnosticMode,
  DiagnosticReport,
  DisplayInfo,
  FlashProfile,
  IntensityProfile,
  OutputMode,
  PaletteName,
  RuntimeSnapshot,
  VisualSettings,
} from "../visuals/types";
import { controlTransport, isNativeApp } from "./transport";

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
  const [diagnosticBusy, setDiagnosticBusy] = useState(false);
  const [diagnosticMode, setDiagnosticMode] = useState<DiagnosticMode>("fullStartup");
  const [diagnosticReport, setDiagnosticReport] = useState<DiagnosticReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    void Promise.all([
      controlTransport.getDisplays(),
      controlTransport.getAudioSources(),
      controlTransport.getState(),
      controlTransport.getPreviousRunReport(),
    ])
      .then(([nextDisplays, nextSources, nextRuntime, previousReport]) => {
        if (!active) return;
        setDisplays(nextDisplays);
        setSources(nextSources);
        setRuntime(nextRuntime);
        setSettings(nextRuntime.settings);
        if (previousReport) setDiagnosticReport(previousReport);
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
  const rekordboxSource = sources.find((source) => source.kind === "rekordboxProcess");
  const canStart = Boolean(isNativeApp && activeDisplay && selectedSource?.available);
  const status = runtimeStatus(runtime);
  const wideRangeDial = settings.intensity === "wild";

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

  const openLogs = () => {
    void controlTransport.openLogsFolder().catch((reason: unknown) => setError(messageFrom(reason)));
  };

  const copyDiagnostic = () => {
    const diagnostic = [error ?? runtime?.lastError, runtime?.logPath ? `Logs: ${runtime.logPath}` : null]
      .filter(Boolean)
      .join("\n");
    void navigator.clipboard.writeText(diagnostic).catch(() => undefined);
  };

  const runDiagnostic = async () => {
    setDiagnosticBusy(true);
    setError(null);
    try {
      setDiagnosticReport(await controlTransport.runDiagnostic(diagnosticMode));
    } catch (reason) {
      setError(messageFrom(reason));
    } finally {
      setDiagnosticBusy(false);
    }
  };

  const cancelDiagnostic = () => {
    void controlTransport.cancelDiagnostic();
  };

  const copyReadableReport = () => {
    if (diagnosticReport) void navigator.clipboard.writeText(readableReport(diagnosticReport));
  };

  const copyJsonReport = () => {
    if (diagnosticReport) void navigator.clipboard.writeText(JSON.stringify(diagnosticReport, null, 2));
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
        <PerformanceCanvas settings={settings} className="control-preview" paused={Boolean(runtime?.running)} />
        <div className="preview-topline">
          <span>Ambient preview</span>
          <span>No audio injected</span>
        </div>
        <div className="preview-caption">
          <span>{runtime?.running ? "Preview paused to preserve performance" : "Live response activates in the output"}</span>
          <strong>Auto · 28 illusions</strong>
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
                {sources.length === 0 && <option value="process:auto">Desktop package required</option>}
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

        <div className="diagnostic-grid" aria-label="Live connection diagnostics">
          <DiagnosticItem label="Rekordbox process" value={runtime?.audio.rekordboxDetected || rekordboxSource?.detected ? "Detected" : "Not detected"} tone={runtime?.audio.rekordboxDetected || rekordboxSource?.detected ? "good" : "waiting"} />
          <DiagnosticItem label="Capture client" value={runtime?.audio.captureInitialized ? "Initialized" : runtime?.audio.state === "connecting" ? "Initializing" : "Not initialized"} tone={runtime?.audio.captureInitialized ? "good" : "waiting"} />
          <DiagnosticItem label="Audio route" value={audioRouteLabel(runtime)} tone={runtime?.audio.route !== "none" ? "good" : "waiting"} />
          <DiagnosticItem label="Sample flow" value={sampleFlowLabel(runtime)} tone={runtime?.audio.sampleFlow === "flowing" ? "good" : runtime?.audio.sampleFlow === "silent" ? "warning" : "waiting"} />
          <DiagnosticItem label="Reactive ready" value={runtime?.audio.reactiveReady ? "Ready" : "Waiting"} tone={runtime?.audio.reactiveReady ? "good" : "waiting"} />
          <DiagnosticItem label="Phrase source" value={phraseSourceLabel(runtime)} tone={runtime?.phrase.provenance === "rekordbox" ? "good" : runtime?.phrase.provenance === "audioInferred" ? "info" : "waiting"} />
          <DiagnosticItem label="Renderer" value={rendererLabel(runtime)} tone={runtime?.renderer.state === "running" ? "good" : runtime?.renderer.state === "failed" ? "warning" : "waiting"} />
        </div>

        <div className="connection-test" aria-label="Connection test">
          <div>
            <strong>Test connection</strong>
            <small>Runs bounded probes without entering fullscreen.</small>
          </div>
          <select value={diagnosticMode} onChange={(event) => setDiagnosticMode(event.target.value as DiagnosticMode)} disabled={diagnosticBusy || runtime?.running}>
            <option value="fullStartup">Full startup</option>
            <option value="audioOnly">Audio only</option>
            <option value="rendererOnly">Renderer only</option>
            <option value="safeRenderer">Safe renderer</option>
          </select>
          {diagnosticBusy ? (
            <button type="button" onClick={cancelDiagnostic}>Cancel test</button>
          ) : (
            <button type="button" onClick={() => void runDiagnostic()} disabled={!isNativeApp || runtime?.running}>Test connection</button>
          )}
        </div>

        {diagnosticReport && (
          <div className="diagnostic-report" data-verdict={diagnosticReport.verdict}>
            <div><strong>{diagnosticReport.verdict.toUpperCase()}</strong><span>{diagnosticReport.summary}</span></div>
            <code>{diagnosticReport.failureCode ?? diagnosticReport.reportId}</code>
            <div className="report-actions">
              <button type="button" onClick={copyReadableReport}>Copy readable result</button>
              <button type="button" onClick={copyJsonReport}>Copy JSON</button>
              <button type="button" onClick={openLogs}>Open logs folder</button>
            </div>
          </div>
        )}

        <div className="control-divider" />

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
          <div className="safety-note">
            <strong>{wideRangeDial ? "Wide-range audio dial" : "Flash safety"}</strong>
            <small>
              {wideRangeDial
                ? "Wild stays expansive while leaving a little more headroom at the peaks."
                : "Off is the default. Motion impacts stay active without white flashes."}
            </small>
          </div>
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

      {(error || runtime?.lastError) && (
        <div className="control-error" role="alert">
          <strong>Visual output did not start</strong>
          <span>{error ?? runtime?.lastError}</span>
          {runtime?.logPath && <code>{runtime.logPath}</code>}
          <div>
            <button type="button" onClick={copyDiagnostic}>Copy diagnostic</button>
            {runtime?.logPath && <button type="button" onClick={openLogs}>Open logs folder</button>}
          </div>
        </div>
      )}

      <footer className="launch-row">
        <div>
          <span>{activeDisplay?.name ?? "Finding displays…"}</span>
          <small>{launchHint(runtime, selectedSource, isNativeApp)}</small>
        </div>
        <button
          className={`launch-button ${runtime?.running ? "is-stop" : ""}`}
          type="button"
          onClick={() => void toggleOutput()}
          disabled={busy || diagnosticBusy || (!runtime?.running && !canStart)}
        >
          <i>{runtime?.running ? <StopIcon /> : <PlayIcon />}</i>
          <span>{busy ? "Starting and checking GPU…" : runtime?.running ? "Stop visuals" : runtime?.lifecycle === "failed" ? "Retry live visuals" : "Start live visuals"}</span>
        </button>
      </footer>
    </main>
  );
}

function DiagnosticItem({ label, value, tone }: { label: string; value: string; tone: "good" | "waiting" | "warning" | "info" }) {
  return <div className="diagnostic-item" data-tone={tone}><span>{label}</span><strong><i />{value}</strong></div>;
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
  if (runtime.lifecycle === "starting") return { label: "Starting", tone: "warning" };
  if (runtime.lifecycle === "failed") return { label: "Start failed", tone: "warning" };
  if (!runtime.running) return { label: "Ready", tone: "idle" };
  if (runtime.outputMode === "black") return { label: "Black screen", tone: "warning" };
  if (runtime.audio.state === "recovering" || runtime.audio.state === "connecting") return { label: "Reconnecting", tone: "warning" };
  if (runtime.reactive) return { label: "Audio reactive", tone: "live" };
  return { label: "Ambient", tone: "ambient" };
}

function audioRouteLabel(runtime: RuntimeSnapshot | null) {
  if (!runtime) return "Checking";
  if (runtime.audio.route === "rekordboxProcess") return "Rekordbox process";
  if (runtime.audio.route === "selectedOutput") return runtime.audio.sourceName ?? "Selected output";
  if (runtime.audio.route === "defaultOutputFallback") return "Default-output fallback";
  if (runtime.audio.route === "systemOutputFallback") return "System-output fallback";
  if (runtime.audio.route === "selectedInput") return runtime.audio.sourceName ?? "Selected input";
  return "Not active";
}

function sampleFlowLabel(runtime: RuntimeSnapshot | null) {
  if (!runtime) return "Checking";
  if (runtime.audio.sampleFlow === "flowing") return "Samples flowing";
  if (runtime.audio.sampleFlow === "silent") return "Packets, silent";
  if (runtime.audio.sampleFlow === "waiting") return "Waiting for samples";
  return runtime.audio.state === "unsupported" ? "OS version unsupported" : "Not active";
}

function phraseSourceLabel(runtime: RuntimeSnapshot | null) {
  if (!runtime) return "Checking";
  if (runtime.phrase.provenance === "rekordbox") return `Rekordbox${runtime.phrase.phrase ? ` · ${runtime.phrase.phrase}` : ""}`;
  if (runtime.phrase.provenance === "cueMarkers") return "Cue-derived";
  if (runtime.phrase.provenance === "audioInferred") return `Audio inferred${runtime.phrase.phrase ? ` · ${runtime.phrase.phrase}` : ""}`;
  return "Unavailable";
}

function rendererLabel(runtime: RuntimeSnapshot | null) {
  if (!runtime) return "Checking";
  if (runtime.renderer.state === "running") return `${runtime.renderer.backend ?? "GPU"}${runtime.renderer.softwareFallback ? " · software" : ""}`;
  if (runtime.renderer.state === "initializing") return "Initializing";
  if (runtime.renderer.state === "failed") return "Failed";
  return "Stopped";
}

function sourceMessage(source: AudioSourceInfo | undefined, native: boolean) {
  if (!native) return "Install the Windows or macOS desktop package to connect Rekordbox.";
  if (!source) return "Choose an available audio source.";
  if (source.kind === "inputDevice") {
    return `Input capture ready${source.isDefault ? " · current microphone/line-in default" : ""}. macOS asks for microphone permission when it starts.`;
  }
  if (source.kind === "rekordboxProcess" && source.name.includes("Safe Windows-output capture")) {
    return source.detected
      ? "Rekordbox detected; using the crash-safe Windows output route. Sample flow is verified separately below."
      : "Rekordbox is not detected; the safe Windows output route can still capture the current system output.";
  }
  if (source.kind === "rekordboxProcess" && !source.detected) return "Rekordbox is not detected. Start stays ambient and waits/retries; supported Windows routes may use an explicit output fallback.";
  if (source.kind === "rekordboxProcess") return "Process detected; samples and phrase provenance are verified separately below.";
  if (source.id === "output:default") return "System-audio capture ready · listens to everything playing through macOS speakers/output.";
  return `Output loopback ready${source.isDefault ? " · current system default" : ""}.`;
}

function launchHint(runtime: RuntimeSnapshot | null, source: AudioSourceInfo | undefined, native: boolean) {
  if (runtime?.running) return runtime.audio.message ?? `${runtime.audio.state} · full screen`;
  if (runtime?.lifecycle === "failed") return "The controller stayed open · retry or inspect the diagnostic log";
  if (!native) return "Ambient preview only here · live capture is in the desktop package";
  if (!source?.available) return "Select an available Rekordbox, system-audio, or input route";
  return `${source.name} · startup waits for the first valid GPU frame`;
}

function readableReport(report: DiagnosticReport) {
  const lines = [
    `PulseBridge connection diagnostic: ${report.verdict.toUpperCase()}`,
    report.summary,
    `Report ID: ${report.reportId}`,
    `Mode: ${report.mode}`,
    `Duration: ${report.durationMs} ms`,
  ];
  if (report.failureStage && report.failureCode) lines.push(`Failure: ${report.failureStage} (${report.failureCode})`);
  lines.push(`Audio: detected=${report.audio.processDetected}, initialized=${report.audio.captureInitialized}, packets=${report.audio.packetsReceived}, signal=${report.audio.nonSilentSamplesReceived}, reactive=${report.audio.reactiveReady}`);
  if (report.renderer.adapter) lines.push(`Renderer: ${report.renderer.adapter} (${report.renderer.backend ?? "unknown backend"})`);
  lines.push(`Log: ${report.logPath}`);
  return lines.join("\n");
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

function PlayIcon() {
  return <svg viewBox="0 0 20 20" aria-hidden="true"><path d="m7 4 9 6-9 6Z" /></svg>;
}

function StopIcon() {
  return <svg viewBox="0 0 20 20" aria-hidden="true"><rect x="5" y="5" width="10" height="10" rx="1" /></svg>;
}
