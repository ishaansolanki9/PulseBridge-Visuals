# PulseBridge 0.1.4: reactive line worlds

This revision replaces the six restrained spatial sculptures from 0.1.3 with large, deforming line structures. Kicks launch traveling waves, mids bend and braid the geometry, and highs pluck or fracture individual strands. The response happens throughout playback; it does not wait for the old rare expansion event. White flashes are excluded from these six worlds, including when the original patterns' flash option is enabled.

Try **Visual scene → Bass Web** or **Ribbon Reactor** first. The other worlds are **Shockwave Tunnel**, **Aurora Strings**, **Prism Surge**, and **Faultline**. Scene selection works during output. Auto still directs all 32 scenes at musical boundaries. Saved 0.1.3 scene IDs map to the corresponding replacement, and existing audio routes and settings remain compatible. The browser controller preview remains an ambient illustration of the original style; the actual line worlds render in the native performance output.

## What makes the response different

Each scene generates 48 strands with 96 segments each as instanced triangle ribbons. Perspective projection, depth attenuation, a narrow core, and a soft halo establish a luminous 3D wire structure. Saturated alpha blending avoids additive white-out. There are no imported models, small particle clouds, raymarched solids, or per-frame mesh allocations.

A fixed 30 Hz history retains approximately one second of the independent live reactive envelopes. The shader samples different ages at different positions, so one kick travels across the mesh or down the tunnel. Mids and highs have their own deformation functions. The history interpolates frame arrivals to keep propagation stable at different refresh rates. Existing intensity and music-reactivity controls scale the live signals before they enter history. Palette drift, brightness, and full-black output also remain supported.

Single scenes can render at the existing 1920×1080 ceiling on integrated GPUs. Crossfades reserve 720p. Sustained late frames still reduce quality and resolution; recovery needs 20 seconds of stable timing. The presentation surface continues to match the full physical display.

## Verification and reproduction

The release passed frontend lint, TypeScript checks and production build; Rust formatting, Clippy and 67 unit tests; Windows x64 cross-target checking; and two explicit native Metal GPU auditions on Apple M1. The production WGSL and line pipeline are shared by normal rendering, renderer diagnostics, and the audition harness.

The isolated-band audition fixes camera, clock, palette, and exposure, then injects bass, mids, or highs separately. It compares normalized spatial luminance distributions so a whole-image brightness change cannot satisfy the check. All 18 scene/band cases exceeded the 0.20 difference threshold (observed range 0.311–1.342). It also checks that a maximum legacy white flash leaves each line scene unchanged and that full-black output contains only black pixels. These image checks supplement visual inspection; they are not a substitute for judging a real track.

The temporal audition captures six 12-second synthetic sequences covering quiet, groove, build, impact, and breakdown, plus line-to-line and original-to-line transitions. The 4–8 second interval deliberately uses the lowest detail tier. No audio device is opened by either audition.

```bash
PULSEBRIDGE_AUDITION_DIR="$PWD/transfer-ready/reactive-lines-audition" \
  cargo test --manifest-path src-tauri/Cargo.toml native_ -- --ignored --nocapture
python3 scripts/render-audition.py transfer-ready/reactive-lines-audition
```

The conversion script requires Pillow. Open the generated `review.html` for animated captures and the isolated-band comparison. All generated media stays in the ignored transfer directory. On Windows, set `$env:PULSEBRIDGE_AUDITION_DIR` to the desired folder before running the same Cargo command.

## Apple M1 measurements, September 9, 2026

1080p single-scene results, 25 measured frames after five warm-up frames:

| Scene | Median | p95 |
| --- | ---: | ---: |
| Bass Web | 1.13 ms | 1.26 ms |
| Ribbon Reactor | 1.16 ms | 1.31 ms |
| Shockwave Tunnel | 1.15 ms | 1.62 ms |
| Aurora Strings | 1.14 ms | 1.20 ms |
| Prism Surge | 1.15 ms | 1.23 ms |
| Faultline | 1.10 ms | 1.83 ms |

The Ribbon Reactor + Prism Surge crossfade at its 1280×720 budget measured 1.11 ms median and 1.48 ms p95. Timings include queue submission and GPU completion wait, but exclude texture readback, display presentation, live capture, and Rekordbox load. They establish rendering headroom, not end-to-end frame-rate or audio-latency guarantees.

## Local installation and Windows readiness

The Apple silicon application is built, ad-hoc signed, signature-verified, and installed at `/Applications/PulseBridge.app`; the preceding app is retained in `transfer-ready/PulseBridge-before-0.1.4.app`. Ad-hoc signing is local signing, not Apple notarization.

Windows source passes:

```bash
PATH="/opt/homebrew/opt/llvm/bin:$PATH" \
  cargo check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc --all-targets
```

The versioned Windows source ZIP contains the reviewed source and existing fail-fast Windows build script. Extract it on Windows, open PowerShell in the extracted project, and run:

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\build-windows.ps1
```

This produces `transfer-ready\PulseBridge Visuals Setup.exe` on Windows. The old installer already present in the Mac transfer directory is not a 0.1.4 build. Windows installer execution, DirectX rendering, and a live Rekordbox session still need a Windows machine. No GitHub push or remote build is part of this local review.
