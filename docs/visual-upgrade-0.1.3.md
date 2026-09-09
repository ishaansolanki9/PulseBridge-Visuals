# PulseBridge 0.1.3 visual upgrade

The native performance library now contains the original 26 illusions plus Magnetic Swarm, Liquid Relic, Impossible Architecture, Aurora Veil, Kinetic Sculpture, and Topographic Ocean. Choose **Visual scene** in the controller to hold a new scene, or use **Auto** for phrase-directed selection across the complete library. The browser preview continues to show the original ambient appearance; it does not demonstrate the new native scenes or audio responsiveness.

Spatial scenes use world-space positions and a restrained perspective camera. Solid scenes use bounded signed-distance fields, normals, lighting, and atmospheric depth. Aurora uses inexpensive translucent layers. The swarm is a procedural particle arrangement; this release does not add a physical particle simulation, external assets, or a new rendering engine.

Composition history reduces successive scenes with similar layouts. Audio bands retain separate roles. Rare expansion/reassembly events require confident live musical evidence, rearm after low impact, and have a 24-second cooldown. No future drop prediction or private Rekordbox metadata is claimed. Mixed transitions process the original scene's effects separately so its camera and overlays do not jump when 3D enters.

## Native quality budgets

The surface always matches the physical window and uses the existing linear blitter. A single original scene or a single scene on a discrete GPU can use the HD internal ceiling. Integrated/software GPUs reserve a 720p ceiling for spatial scenes. A crossfade between two spatial scenes reserves 540p; other crossfades reserve 720p. Shader detail falls with resolution. Sustained missed frame deadlines can lower quality, and recovery needs 20 seconds of stable timing. These limits are proportional for smaller and non-16:9 windows.

The new scene selector is saved with settings; older settings default to Auto. Existing audio routes, brightness controls, flash opt-in, audio-loss behavior, emergency shortcuts, and performance-window lifecycle remain in place.

## Reproducible synthetic audition

The audition is an **ignored Rust test**, excluded from application builds. It opens no audio device. It uses the production WGSL, real smoothing and transformation envelopes, a fixed Electric palette, and explicit representative test budgets. These are synthetic inputs, not a live performance or an audio integration test.

On macOS, from the repository root:

```bash
PULSEBRIDGE_AUDITION_DIR="$PWD/transfer-ready/audition" \
  cargo test --manifest-path src-tauri/Cargo.toml native_scene_audition -- --ignored --nocapture
python3 scripts/render-audition.py transfer-ready/audition
```

The optional conversion script needs Pillow (`python3 -m pip install Pillow` in your preferred environment). Open `transfer-ready/audition/review.html` to see the animated captures. Raw PPM frames, a contact sheet, filmstrips, GIFs, and `timings.txt` stay in the ignored transfer directory.

PowerShell equivalent for the native test:

```powershell
$env:PULSEBRIDGE_AUDITION_DIR = Join-Path $PWD "transfer-ready\audition"
cargo test --manifest-path src-tauri/Cargo.toml native_scene_audition -- --ignored --nocapture
```

Each 12-second sequence at 15 sampled frames/second contains quiet (0–2s), groove (2–5s), build (5–8s), impact/peak (8–10s), and breakdown (10–12s). Seconds 4–8 deliberately use the lowest shader detail to expose fallback artifacts. Separate clips inspect spatial-to-spatial and original-to-spatial crossfades. The benchmark samples 25 measured frames after five warm-up frames at 1080p/high, 720p/medium, and 540p/low.

## Verification scope

The release was checked on an Apple M1 using Metal: production WGSL parsing/validation, actual GPU rendering of every new scene, temporal frame inspection, fallback detail, crossfades, controller scene selection and persistence, frontend checks, Rust formatting/Clippy/tests, and macOS application/bundle creation. Timing measurements include queue submission and GPU completion wait but exclude texture readback, fullscreen presentation, Rekordbox CPU/GPU load, and audio capture. They demonstrate rendering headroom; they are not a promise of end-to-end 60 FPS.

Windows source is checked from macOS using the installed Windows Rust target and LLVM's resource compiler:

```bash
PATH="/opt/homebrew/opt/llvm/bin:$PATH" \
  cargo check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc --all-targets
```

A Windows installer and real DirectX/Rekordbox session must still be built/tested on Windows. The updated `scripts/build-windows.ps1` installs prerequisites when needed, stops immediately on a failed native command, and selects only the installer matching the current package version. The existing Windows CI job remains ready for when the source is pushed. No GitHub push or remote build is part of this local review.

To build from the supplied source ZIP on Windows: extract the entire folder, open PowerShell there, and run:

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\build-windows.ps1
```

The script produces `transfer-ready\PulseBridge Visuals Setup.exe`. The installer remains unsigned, as in previous releases.


## Measured Apple M1 results (September 9, 2026)

Single-scene medium-quality rendering at 1280×720, measured using the audition described above:

| Scene | Median frame | p95 frame |
| --- | ---: | ---: |
| Magnetic Swarm | 5.91 ms | 7.45 ms |
| Liquid Relic | 9.09 ms | 10.30 ms |
| Impossible Architecture | 7.72 ms | 10.02 ms |
| Aurora Veil | 2.44 ms | 3.11 ms |
| Kinetic Sculpture | 10.47 ms | 11.53 ms |
| Topographic Ocean | 4.59 ms | 4.98 ms |

The Liquid Relic + Kinetic Sculpture crossfade at its 960×540/low budget measured 11.27 ms median and 12.61 ms p95. At 1080p/high, Liquid Relic and Kinetic Sculpture exceeded a 16.67 ms frame on this machine, supporting the integrated-GPU quality ceiling. Actual fullscreen performance alongside Rekordbox still needs a live session check.
