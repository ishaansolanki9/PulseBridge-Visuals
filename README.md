# PulseBridge Visuals

PulseBridge Visuals turns a second display into a live music-reactive light show. It listens to audio playing from Rekordbox and generates full-screen shader visuals that respond to rhythm, energy, frequency content, and musical transitions in real time.

The laptop window is a compact controller. The performance window is a borderless native `wgpu` surface containing only abstract light and motion—no waveform, player, labels, metrics, or error text. PulseBridge does not download, record, or save source audio.

## What is implemented

- Rekordbox process and Windows Audio Session discovery that finds the render endpoint actually carrying Rekordbox, plus private macOS 14.2+ Core Audio taps for Rekordbox or all outgoing system audio
- Negotiated WASAPI/Core Audio formats, stateful mono 48 kHz resampling, RAII packet/tap cleanup, selectable Windows output loopback, selectable macOS microphone/line-in inputs, and explicit route reporting
- A bounded 5–30 second lock-free PCM ring buffer held only in RAM
- 48 kHz analysis with RMS, sub/bass/mid/high energy, spectral flux, onset strength, rolling normalization, BPM/beat confidence, four-beat bar phase, energy trends, and state hysteresis
- Quiet, Flow, Groove, Build, Impact, Peak, and Breakdown musical behavior
- Phrase-directed Auto behavior across 26 analytic illusions, with curated scene families for intros, verses, builds, drops, breakdowns, bridges, and fills instead of cadence-based random changes
- Spirals, wormholes, moiré interference, rotating-snakes luminance drift, impossible grids, gravity lenses, alien heads, chromatic mazes, and other motion-first illusion families
- Adaptive and fixed palettes, Chill/Balanced/Wild profiles, opt-in flash levels, and advanced reaction controls
- A non-fullscreen **Test connection** workflow with Audio only, Renderer only, Full startup, and Safe renderer reports; versioned JSON, readable copy, and durable unclean-exit reconstruction
- A proportional audio-drive dial plus distinct bass geometry waves, midrange bends, high-frequency color shards, energy-rise depth changes, and music-boundary scene transitions
- Tear-free 60 FPS presentation, a linearly scaled HD internal render target for high-resolution displays, suspended live control preview, first-frame readiness, and ambient audio-loss recovery
- Native Escape/close handling, controller Stop, and `Ctrl+Alt+Shift+F12` (Windows) or `Control+Option+Shift+F12` (macOS) emergency stop
- Persistent settings plus unsigned NSIS (`.exe`) and macOS `.app`/`.dmg` build paths

There is no generated song progression in the application. No documented live Rekordbox interface exposes the current deck's analyzed phrase map and playhead to PulseBridge, so the product does not pretend it has that metadata. Structural direction is labeled **audio inferred** and is driven by the captured mix, BPM confidence, bar boundaries, longer-horizon changes, and a bounded content-derived structure model. The browser route remains an ambient, non-reactive appearance preview; real responsiveness begins in the desktop package with live audio.

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

After installation, open Rekordbox in Performance mode, enable **PC MASTER OUT**, and play a track. Then open PulseBridge and leave **Rekordbox audio** selected. PulseBridge follows Rekordbox and any child audio helper to the Windows render endpoint that owns its audio session, then captures that endpoint through stable shared-mode WASAPI loopback. With a DDJ-1000, keep **DDJ-1000 ASIO** as Rekordbox's primary audio device; PC MASTER OUT creates the parallel Windows render stream PulseBridge needs because direct exclusive-mode ASIO is outside WASAPI loopback. The selected endpoint is a system mix, so another app routed to that same endpoint can also be heard. Run **Test connection** while the track is playing before starting fullscreen visuals.

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

Windows defaults to **Rekordbox audio**, which uses Windows Audio Session APIs to identify the matching render endpoint before opening ordinary endpoint loopback. It reconnects when Rekordbox starts, stops, or moves endpoints. **Automatic Windows output** remains an explicit all-app fallback and individual outputs remain manually selectable. Windows process-specific loopback is disabled because repeated hardware tests ended the entire process inside native activation, even after the documented callback and lifetime requirements were applied. Persisted `process:auto` selections migrate automatically to `rekordbox:auto`. macOS continues to offer Rekordbox-only and all-system-output Core Audio taps on 14.2+, plus detected microphone/line-in inputs. macOS permissions are requested only when the selected route starts; if denied, enable PulseBridge under **System Settings → Privacy & Security → Screen & System Audio Recording** or **Microphone**.

Audio capture supplies the mixed PCM signal used to derive loudness, bass, mids, highs, onsets, BPM, beat/bar timing, phrase changes, and a session-local structure signature. It does not claim access to Rekordbox's rendered waveform graphics, individual deck/stem layers, track identity, or private phrase metadata.

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

Rekordbox can publish tempo and beat timing through Ableton Link, but Link does not publish track identity or Rekordbox's analyzed phrase map. Direct Link integration is not bundled in this release because the available SDK requires a separate GPL/commercial licensing decision; audio-derived timing remains fully functional without it. DMX, physical fixtures, a music player, waveform extraction, accounts, cloud services, and audio recording are intentionally out of scope.
