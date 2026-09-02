# PulseBridge Visuals

PulseBridge Visuals turns a second display into a live music-reactive light show. It listens to audio playing from Rekordbox and generates full-screen shader visuals that respond to rhythm, energy, frequency content, and musical transitions in real time.

The laptop window is a compact controller. The performance window is a borderless native `wgpu` surface containing only abstract light and motion—no waveform, player, labels, metrics, or error text. PulseBridge does not download, record, or save source audio.

## What is implemented

- Rekordbox process discovery with crash-safe Windows output capture by default, an isolated developer opt-in for Windows process-loopback validation, and private macOS 14.2+ Core Audio taps for Rekordbox or all outgoing system audio
- Negotiated WASAPI/Core Audio formats, stateful mono 48 kHz resampling, RAII packet/tap cleanup, selectable Windows output loopback, selectable macOS microphone/line-in inputs, and explicit route reporting
- A bounded 5–30 second lock-free PCM ring buffer held only in RAM
- 48 kHz analysis with RMS, sub/bass/mid/high energy, spectral flux, onset strength, beat pulse, rolling normalization, energy trends, and state hysteresis
- Quiet, Flow, Groove, Build, Impact, Peak, and Breakdown musical behavior
- Auto-only direction across 26 shuffled analytic illusions, with exactly one family outside a normalized transition and at most two compatible modifiers
- Spirals, wormholes, moiré interference, rotating-snakes luminance drift, impossible grids, gravity lenses, alien heads, chromatic mazes, and other motion-first illusion families
- Adaptive and fixed palettes, Chill/Balanced/Wild profiles, opt-in flash levels, and advanced reaction controls
- A non-fullscreen **Test connection** workflow with Audio only, Renderer only, Full startup, and Safe renderer reports; versioned JSON, readable copy, and durable unclean-exit reconstruction
- A proportional audio-drive dial plus distinct bass geometry waves, midrange bends, high-frequency color shards, energy-rise depth changes, and music-boundary scene transitions
- 45 FPS native pacing, a 1440p high-resolution performance cap, suspended live control preview, first-frame readiness, and ambient audio-loss recovery
- Native Escape/close handling, controller Stop, and `Ctrl+Alt+Shift+F12` (Windows) or `Control+Option+Shift+F12` (macOS) emergency stop
- Persistent settings plus unsigned NSIS (`.exe`) and macOS `.app`/`.dmg` build paths

There is no generated song progression in the application. No documented live Rekordbox phrase/playhead API is currently used: structural direction is labeled **audio inferred**. The browser route remains an ambient, non-reactive appearance preview; real responsiveness begins in the desktop package with live audio.

## Installation

### macOS

PulseBridge currently builds for Apple silicon. Install Node.js 22.12 or newer, Rust 1.87 or newer, and the Xcode Command Line Tools, then run:

```bash
git clone https://github.com/ishaansolanki9/PulseBridge-Visuals.git
cd PulseBridge-Visuals
npm ci
npm run tauri -- build --bundles app
codesign --force --deep --sign - "src-tauri/target/release/bundle/macos/PulseBridge.app"
ditto "src-tauri/target/release/bundle/macos/PulseBridge.app" "/Applications/PulseBridge.app"
open "/Applications/PulseBridge.app"
```

When prompted, allow **Screen & System Audio Recording** for Rekordbox or system-output capture, or **Microphone** for a selected input device.

### Windows

Clone the repository on the Windows computer, open PowerShell in the project folder, and run:

```powershell
git clone https://github.com/ishaansolanki9/PulseBridge-Visuals.git
cd PulseBridge-Visuals
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\build-windows.ps1
```

The build script installs missing Windows prerequisites when possible, verifies the project, and creates `transfer-ready\PulseBridge Visuals Setup.exe`. Run that installer; development tools are not needed after installation. The current installer is unsigned, so Windows SmartScreen may require **More info → Run anyway**.

After installation, open Rekordbox before PulseBridge. If Rekordbox uses ASIO, enable PC MASTER OUT and select the Windows output carrying the master audio. Run **Test connection** before starting fullscreen visuals.

### Local development

With Node.js 22.12+ and Rust 1.87+ installed:

```bash
npm ci
npm run dev
```

The browser build is an ambient controller preview. To run the native application with live audio capture:

```bash
npm run tauri -- dev
```

Windows uses stable output loopback for the Rekordbox-aware default after affected-machine reports isolated a native heap-corruption failure inside process-specific activation. Choose the exact Windows output carrying PC MASTER OUT when Rekordbox uses ASIO. The process-specific API remains developer-opt-in until a repaired build passes the real-device matrix. macOS offers Rekordbox-only and all-system-output Core Audio taps on 14.2+, plus detected microphone/line-in inputs. macOS permissions are requested only when the selected route starts; if denied, enable PulseBridge under **System Settings → Privacy & Security → Screen & System Audio Recording** or **Microphone**.

Before fullscreen, use **Test connection** and choose a mode. The report separates process detection, capture initialization, packets, non-silent samples, reactive readiness, GPU validation, and hidden-surface presentation. **Copy readable result**, **Copy JSON**, and **Open logs folder** are available in the controller.

## Verification

```bash
npm run lint
npm run typecheck
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

Windows and macOS CI compile, lint, test, and build platform bundles. These jobs are compile/package checks, not real Rekordbox audio integration tests. Unit tests also cover diagnostic schema/recovery, cancellation, format conversion/resampling, explicit connection facts, scene normalization, modifier compatibility/envelopes/brightness caps, native-close stop routing, and WGSL validation.

## Repository map

- `app/src/control/` — React controller and native command transport
- `app/src/visuals/` — ambient WebGL appearance preview
- `src-tauri/src/audio/` — Windows WASAPI, macOS Core Audio process taps, format conversion, reconnect policy, and PCM ring
- `src-tauri/src/analysis/` — FFT features, normalization, rhythm, and musical states
- `src-tauri/src/phrase/` — optional playback-context boundary and bounded audio-inferred phrase fallback
- `src-tauri/src/display/` — monitor discovery and performance-session lifetime
- `src-tauri/src/diagnostics.rs` / `diagnostic_runner.rs` — durable logs/session markers and bounded connection reports
- `src-tauri/src/visuals/` — state smoothing, palettes, base/modifier direction, and native renderer
- `src-tauri/shaders/` — production WGSL performance shader
- `scripts/` — Windows installer build
- `docs/` — architecture, analysis, reliability, development, and visual language

Ableton Link remains an optional timing enhancement and is not required for live audio. DMX, physical fixtures, a music player, waveform extraction, accounts, cloud services, and audio recording are intentionally out of scope.
