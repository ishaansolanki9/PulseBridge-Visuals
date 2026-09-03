# Development notes

## Runtime boundaries

The browser route (`/?performance=1`) renders quiet ambient motion with zero beat, onset, impact, and audio reactivity. It exists only to review shader appearance and must remain text-free. It is not an audio test path.

The Windows/macOS Tauri application owns the real path: platform audio → bounded PCM ring → Rust analysis + optional phrase context → SceneDirector → native `wgpu` renderer. React reads status, writes settings, and starts/cancels bounded diagnostics.

Run the browser UI:

```bash
npm run dev
```

Run the desktop shell:

```bash
npm run tauri -- dev
```

Debug builds include an opt-in native Start/first-present/Stop smoke path. It uses the real `PerformanceManager` and is compiled out of release builds:

```bash
PULSEBRIDGE_SMOKE_AUTOSTART=1 "src-tauri/target/debug/bundle/macos/PulseBridge.app/Contents/MacOS/PulseBridge Visuals"
```

The configured control window is 960×720 with an 820×640 minimum. Native settings are stored as `visual-settings.json` in the platform application-config directory. Raw audio is never included in that file.

## Windows audio work

Open Rekordbox before PulseBridge so `process:auto` can report detection, but the normal Windows build deliberately uses the stable default-output fallback. The affected laptop repeatedly raised `STATUS_HEAP_CORRUPTION` while `ActivateAudioInterfaceAsync` was pending; explicit Windows output capture completed successfully on the same GPU and audio hardware. Detection, route, sample flow, and phrase provenance remain separate.

The Microsoft process-loopback API requires a sufficiently recent Windows build. It can be enabled only for isolated developer validation by setting `PULSEBRIDGE_EXPERIMENTAL_PROCESS_LOOPBACK=1`; its operation and completion handler now stay on the originating MTA thread until Windows actually invokes the callback instead of being dropped by an unsafe application timeout. Because the API has no cancellation operation, a hung experimental activation may leave that diagnostic worker waiting; normal builds never enter this path. Do not ship the opt-in as the party default until Audio-only, Renderer-only, Full startup, rapid retry, and late-callback tests pass on the affected laptop. Always verify the device-output route. Native Windows work should be checked with the same commands used by `scripts/build-windows.ps1`.

## macOS audio work

Core Audio process/global-output taps require macOS 14.2+. `AudioHardwareCreateProcessTap` and destroy are runtime-resolved so the app can launch on older versions and show an exact unsupported message. Physical microphone/line-in capture uses the selected stable Core Audio device ID and does not require Rekordbox. `Info.plist` contains both system-audio and microphone usage descriptions. Test permission not-requested/granted/denied/revoked states, default-output changes, input disconnect/reconnect, and Rekordbox restart on real Apple Silicon; Intel remains a required manual target if it is advertised.

## Visual work

The GLSL browser shader is a quiet appearance preview and is destroyed while live output is running so it cannot compete with Rekordbox or the native GPU surface. The WGSL shader is the production renderer and must validate through the `native_performance_shader_is_valid_wgsl` test. Never feed generated rhythm into the browser to make it look reactive.

Auto direction is native because it depends on phrase/musical state and a timed random shuffle. Production family IDs 0–31 are declared by `VisualFamily` and dispatched by the WGSL `visual_family` switch; all 32 must remain distinct and available to Auto. IDs 28–31 are Spinning Skull, Watching Eye, Morphing Pyramid, and Tumbling Cube. Modifier IDs 0–7 are Palette Drift, Beat Zoom, Bass Warp, High Sparkle, Feedback Trails (`EchoTrails` internally), Mirror Fold, Chromatic Split, and Impact Bloom. White flashes default to Off; energetic motion must remain legible with flashes disabled.

Whenever WGSL visual math changes, preserve the preview's palette and luminance behavior, but keep the browser preview deliberately cheaper than the production library. The WGSL validator test and the 32-distinct-family test are mandatory. The curated readable-family list should remain free of dense full-screen line fields, while periodic full-library slots keep every family reachable.

The performance bind group contains the uniform snapshot at binding 0, the previous render target at binding 1, and its filtering sampler at binding 2. Any pipeline or diagnostic-probe layout change must keep those three bindings synchronized with `performance.wgsl`. Feedback coefficients are calculated on the CPU, clamped per intensity profile, and passed in the `feedback` uniform vector as persistence, zoom, warp, and chromatic offset.

Cross-target type checking from macOS may require `llvm-rc` for Tauri's Windows resources. The real Windows build remains `scripts/build-windows.ps1`; a Rust target `cargo check` is not installer or hardware validation.

## Release

Clean unsigned bundle commands are:

```powershell
npm run tauri -- build --bundles nsis
```

```bash
npm run tauri -- build --bundles app,dmg
```

The macOS command requires full Xcode for DMG tooling. CI uploads unsigned artifacts; code signing/notarization and Windows Authenticode signing require credentials and are deliberately not simulated.

The README contains the supported Windows build and installation steps. Keep hardware validation notes here focused on development and diagnostics.
