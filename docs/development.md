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

Open Rekordbox before PulseBridge and enable PC MASTER OUT when using the DDJ-1000 ASIO topology so Rekordbox creates a capturable Windows render stream. The `rekordbox:auto` source is the safe Windows default: it enumerates each endpoint's audio sessions, matches the Rekordbox process tree, and captures the matched endpoint with shared-mode WASAPI loopback. It is an endpoint mix, not process-isolated audio. `output:auto` remains the all-app scanner. Persisted `process:auto` settings migrate to `rekordbox:auto`; the process source is disabled and must not call `ActivateAudioInterfaceAsync`. Process detection, session detection, client initialization, packet arrival, non-silent signal, route, and phrase provenance remain separate.

The Microsoft process-loopback API caused repeatable native whole-process termination on the target Windows machine, including after its callback implemented the agile contract and all activation objects were retained through completion. Because an in-process Rust error boundary cannot recover native heap corruption, release code must not retain or reintroduce that activation path. Validate Audio-only, Renderer-only, Full startup, Rekordbox restart, rapid retry, endpoint changes, and silence recovery on real Windows hardware. Native Windows work should also be checked with the same commands used by `scripts/build-windows.ps1`.

## macOS audio work

Core Audio process/global-output taps require macOS 14.2+. `AudioHardwareCreateProcessTap` and destroy are runtime-resolved so the app can launch on older versions and show an exact unsupported message. Physical microphone/line-in capture uses the selected stable Core Audio device ID and does not require Rekordbox. `Info.plist` contains both system-audio and microphone usage descriptions. Test permission not-requested/granted/denied/revoked states, default-output changes, input disconnect/reconnect, and Rekordbox restart on real Apple Silicon; Intel remains a required manual target if it is advertised.

## Visual work

The GLSL browser shader is a quiet appearance preview and is destroyed while live output is running so it cannot compete with Rekordbox or the native GPU surface. The WGSL shader is the production renderer and must validate through the `native_performance_shader_is_valid_wgsl` test. Never feed generated rhythm into the browser to make it look reactive.

Auto direction is native because it depends on live phrase/musical state, BPM confidence, four-beat boundaries, and a content-derived structure signature. Production family IDs 0–31 are declared by `VisualFamily` and dispatched by the WGSL `visual_family` switch; all 32 must remain distinct and available across the phrase-specific candidate groups. Modifier IDs 0–7 are Palette Drift, Beat Zoom, Bass Warp, High Sparkle, Echo Trails, Mirror Fold, Chromatic Split, and Impact Bloom. Scene and modifier changes must be caused by musical state/phrase boundaries or impact events, never a wall-clock random shuffle. White flashes default to Off; energetic motion must remain legible with flashes disabled.

Whenever WGSL visual math changes, preserve the preview's palette and luminance behavior, but keep the browser preview deliberately cheaper than the production library. The WGSL validator test and the 32-distinct-scene test are mandatory.

Cross-target type checking from macOS may require `llvm-rc` for Tauri's Windows resources. The real Windows build remains `scripts/build-windows.ps1`; a Rust target `cargo check` is not installer or hardware validation.

Windows MSVC builds include the statically linked DXC shader compiler through wgpu's `static-dxc` feature. The locked compiler dependency downloads and verifies its native build archive during the first Windows build; installed users do not need a separate `dxcompiler.dll`. `visuals/gpu.rs` supplies the shared instance policy for live output, diagnostic probes, and auditions. Keep all three paths aligned.

Windows shaders remain optimized in development builds: the wgpu `DEBUG` flag adds DXC `-Od`, which made WARP's first draw exceed 30 seconds for this library. Remove only shader-debug generation; retain the separate API validation flag and WGSL validation. `fs_main` consumes one isolated scene; `SceneCompositor` renders both transition sides independently. Do not reintroduce a duplicate secondary-scene library dispatch into the fragment shader. The Tron collection shares its first look call with held looks and uses a second only during collection blends.

Run `cargo test --manifest-path src-tauri/Cargo.toml native_pipeline_startup_audition -- --ignored --nocapture` to compile all production pipeline types and read back representative scenes. Setup, selected-scene compilation, and completion of the first GPU draw must fit within the live 60-second cold-start deadline. Scene pipelines are compiled on demand instead of compiling the full library on Start. Windows CI runs this in a separate step with a two-minute deadline so a compiler hang cannot block packaging indefinitely. WGSL validation alone does not exercise DX12 shader compilation or the driver. The test renders offscreen and opens no audio device; real Intel Arc/fullscreen/Rekordbox validation is still separate.

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

The production WGSL is the concatenation of `shaders/performance.wgsl` and `shaders/spatial.wgsl`. Keep the `VisualParams` and Rust `VisualUniforms` layouts in agreement. The `spatial` vector holds integrated motion time, the legacy transformation envelope (unused by the line shader), quality, and elapsed time since the last history sample. `signal_history` is 32 aligned vec4 samples, with the current value first and fixed 30 Hz samples after it. Bindings must be visible to both vertex and fragment stages. The fullscreen pass draws original patterns and the line background; a shared instanced line pipeline then draws either or both line worlds. The diagnostic renderer and synthetic audition validate the same pipeline. See [the 0.1.4 reactive-line notes](reactive-lines-0.1.4.md) for the development-only GPU audition.

The 0.1.6 `chromatic` vec4 holds color phase, color-motion amount, integrated 2D travel, and a reserved component. `composite.wgsl` performs the final linear-light dissolve after both scenes render into cached sRGB targets. `compositor.rs` is shared with the native audition and preserves the outgoing appearance snapshot. Validate both WGSL modules; see [the 0.1.6 notes](color-and-transitions-0.1.6.md) for transition and color checks.

Native scene specialization is in `src-tauri/src/visuals/scene_shader.rs`. It derives programs from the checked-in WGSL, removes unselected Tron branches and unreachable functions, and leaves scene formulas/shared effects unchanged. It intentionally recognizes this shader library's limited dispatch syntax; update its exhaustive WGSL validation when changing that syntax. `native_specialized_scene_equivalence` compares 100 held/cycling frames with the original shader. Run it explicitly with `-- --ignored --nocapture`; `native_transition_audition` and `native_tron_audition` additionally verify crossfades and audio reactions using the production scene cache.

Windows CI also builds a debug desktop executable and runs `scripts/smoke-windows.ps1`. That script runs actual fullscreen Start/first GPU completion/present/Stop twice, including a 13-second artificial renderer delay that would fail the old deadline. The debug-only environment hook uses default visual settings and isolated logs under `src-tauri/target/windows-startup-smoke`. Neither saved settings nor existing diagnostic history is changed. The release installer has no smoke hook. Startup logs include source revision and debug/release identity. After pulling source, rebuild with `npm.cmd ci` and `npm.cmd run tauri -- dev`; pulling alone does not replace an installed executable.
