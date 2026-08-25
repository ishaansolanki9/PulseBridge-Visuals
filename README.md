# PulseBridge Visuals

PulseBridge Visuals turns a second display into a live music-reactive light show. It listens to audio playing from Rekordbox and generates full-screen shader visuals that respond to rhythm, energy, frequency content, and musical transitions in real time.

The laptop window is a compact controller. The performance window is a borderless native `wgpu` surface containing only abstract light and motion—no waveform, player, labels, metrics, or error text. PulseBridge does not download, record, or save source audio.

## What is implemented

- Rekordbox process discovery and process-specific Windows loopback capture
- Selectable WASAPI output-device loopback, plus automatic default-output fallback
- A bounded 5–30 second lock-free PCM ring buffer held only in RAM
- 48 kHz analysis with RMS, sub/bass/mid/high energy, spectral flux, onset strength, beat pulse, rolling normalization, energy trends, and state hysteresis
- Quiet, Flow, Groove, Build, Impact, Peak, and Breakdown musical behavior
- Fluid, Waves, Pulse, Tunnel, and Burst procedural styles with smooth Auto crossfading
- Adaptive and fixed palettes, Chill/Balanced/Wild profiles, opt-in flash levels, and advanced reaction controls
- Multi-display selection, a raw fullscreen performance window, independent render/audio/analysis threads, 60 FPS pacing, sleep prevention, and ambient audio-loss recovery
- Laptop-only emergency controls for Reactive, Ambient, Black screen, and Stop
- Persistent settings and a normal NSIS Windows installer

There is no generated song progression in the application. The browser route is an ambient, non-reactive appearance preview; real responsiveness begins in the Windows package with live audio.

## Transfer to Windows

Read [WINDOWS_TRANSFER.md](WINDOWS_TRANSFER.md). To create the small source-transfer archive on macOS:

```bash
./scripts/make-transfer-zip.sh
```

On Windows, extract that archive and run:

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\build-windows.ps1
```

The script installs missing Windows build prerequisites, and the completed installer is copied to `transfer-ready\PulseBridge Visuals Setup.exe`. The installed app does not require Node, Rust, Python, Visual Studio, or a terminal.

## Local development

Requirements: Node.js 22.12+ and Rust 1.87+.

```bash
npm ci
npm run dev
```

The local browser build is for control-window and ambient shader review only. Run the native shell with:

```bash
npm run tauri -- dev
```

Live audio capture is Windows-only. Other platforms return an explicit unsupported state and never substitute fake audio.

## Verification

```bash
npm run lint
npm run typecheck
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

Windows CI builds and uploads the NSIS installer. Unit tests cover the bounded ring buffer, rolling normalization, audio-loss fade, musical-state dwell, impact cooldown, parameter smoothing, settings limits, palette interpolation, and WGSL parsing.

## Repository map

- `app/src/control/` — React controller and native command transport
- `app/src/visuals/` — ambient WebGL appearance preview
- `src-tauri/src/audio/` — Rekordbox detection, process loopback, output fallback, and PCM ring
- `src-tauri/src/analysis/` — FFT features, normalization, rhythm, and musical states
- `src-tauri/src/display/` — monitor discovery and performance-session lifetime
- `src-tauri/src/visuals/` — state smoothing, palettes, and native renderer
- `src-tauri/shaders/` — production WGSL performance shader
- `scripts/` — transfer archive and Windows installer build
- `docs/` — architecture, analysis, reliability, development, and visual language

Ableton Link remains an optional timing enhancement and is not required for live audio. DMX, physical fixtures, a music player, waveform extraction, accounts, cloud services, and audio recording are intentionally out of scope.
