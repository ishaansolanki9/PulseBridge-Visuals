# PulseBridge V2 follow-up: crash isolation, connection diagnostics, safe display exit, energetic visuals, and macOS support

## Mission

Implement and verify the next PulseBridge iteration. Do not stop after writing another plan. Diagnose the current Windows failure, make every startup stage observable, fix the proven cause, add reliable ways to leave display mode, redesign Auto around a small set of energetic bases plus controlled modifiers, and make live capture supportable on both Windows and macOS.

The current user report is:

- Rekordbox appears connected, but pressing **Start live visuals** terminates the entire application.
- The user needs a JSON or readable diagnostic that identifies the exact connection/startup stage that failed.
- Once display mode begins, there is no dependable way to leave it without Task Manager.
- The visuals are too tranquil and do not react strongly enough to make a crowd move.
- Some layering and V1-style shifting colors were good, but showing everything simultaneously was not.
- Auto should choose from roughly 5–8 coherent base visuals, then introduce, rotate, and remove a small number of modifiers as the music develops.
- The application should work on Windows and macOS.

Treat this document as a follow-up to `CODEX_IMPLEMENTATION_BRIEF.md`. Preserve completed work from that pass, but verify it rather than assuming it solved the reported crash.

## Definition of success

1. A failed Rekordbox/audio/GPU/display startup never takes down the controller.
2. **Test connection** can run without entering fullscreen and returns a versioned, copyable JSON report plus a short human-readable diagnosis.
3. The UI never uses **Connected** to mean only that a Rekordbox process exists. It distinguishes detection, capture initialization, sample flow, and reactive readiness.
4. If the process exits abruptly, the next launch identifies the last completed/in-progress startup stage and offers the prior report/log.
5. `Escape`, an OS-standard close action, and the controller's **Stop visuals** control can each leave display mode. Cursor, focus, fullscreen, topmost state, workers, and power state are restored.
6. Auto renders exactly one base visual at a time, except for a bounded crossfade. It may add at most two compatible modifiers, with the normal case being zero or one.
7. Balanced and Wild modes visibly respond to beat, bass, onset, highs, impacts, energy, and structural changes without relying on white strobing.
8. Live audio works on supported Windows and macOS versions with truthful permissions, route, and fallback reporting.
9. Windows and macOS packages build in CI and pass real-device smoke tests.

## What the repository already does

Do not duplicate these pieces blindly:

- `src-tauri/src/diagnostics.rs` writes rotated JSON-lines records to `pulsebridge.log`.
- `src-tauri/src/display/mod.rs` has a renderer readiness channel, hidden-window startup, rollback, lifecycle states, and worker cleanup.
- `src-tauri/src/audio/wasapi.rs` attempts Rekordbox process loopback, then default-output loopback, and reports sample-flow status.
- `app/src/control/ControlApp.tsx` shows several live status fields and a log path.
- `src-tauri/src/visuals/director.rs` already chooses a primary family and a limited accent/crossfade.
- The native shader and browser preview already contain multiple visual families.
- `src-tauri/src/audio/platform_stub.rs` is still the entire non-Windows audio implementation and explicitly marks capture unsupported.

The current local baseline on macOS is green: frontend typecheck, lint, production build, and all 29 Rust tests pass. That proves static/local correctness only; it does not reproduce the reported Windows native crash or validate Rekordbox sample flow.

## Preliminary investigation findings

These are code-backed findings, not a claimed Windows root cause:

1. **Process detection is not a connection.** `enumerate_sources` marks the Rekordbox option available even when Rekordbox is absent because the option can fall back to default output. Before startup, the app has only enumerated processes/devices; it has not activated a client or received a sample.
2. **Startup readiness currently proves a GPU frame, not reactive audio.** `PerformanceManager::start_transaction` returns after the renderer's first frame. Audio may still be connecting, silent, recovering, or falling back.
3. **The existing copy action is not the requested diagnostic report.** It copies an error string and log path, while the JSON-lines log lacks one self-contained per-run result with stage timings, stable error codes, selected route, and final verdict.
4. **The native fullscreen window has no Escape path.** `PerformanceOutput.tsx` listens for Escape, but the production output is a raw native `tauri::Window` rendered by `wgpu`, not that React component. The native window is also created with `closable(false)`.
5. **GPU failures are not fully captured.** The renderer catches Rust panics on its worker thread, but `wgpu` device errors are uncaptured by default and no device-lost callback is registered. Pipeline creation is not wrapped in validation error scopes. Surface loss is reconfigured even though a true lost surface can require recreation.
6. **A full native-process termination cannot be explained by the current panic hook alone.** Rust panic logging does not reliably capture access violations, driver termination, process aborts, or power loss. A durable last-stage marker is needed, and a separate renderer process may be needed if evidence shows a native GPU fault.
7. **The WASAPI buffer needs stronger RAII cleanup.** After `IAudioCaptureClient::GetBuffer`, conversion/resampling errors can return before `ReleaseBuffer`. Every acquired buffer must be released exactly once, including error and device-change paths.
8. **macOS live capture is not implemented.** The current non-Windows stub returns no sources and tells the user to use Windows.
9. **The visual architecture is already partway toward the requested model, but its behavior is still too restrained.** Several scene budgets and smoothed defaults create slow ambient motion; the base/accent model is not yet exposed or tuned as a deliberate base-plus-modifier system.

The crash must remain labeled **unconfirmed** until a Windows diagnostic, Windows Event Viewer entry, dump, or reproducible test identifies the failing stage.

## Required execution order

### Phase 0 — Preserve evidence before changing behavior

1. Tag every launch attempt with a random `sessionId` and every diagnostic run with a `reportId`.
2. Add synchronous critical markers around these stages:
   - app runtime start
   - settings load
   - display enumeration
   - performance window create/configure
   - audio process discovery
   - process-loopback activation
   - output-loopback activation
   - audio format parse and client initialization
   - first audio packet
   - first non-silent sample
   - GPU instance, surface, adapter, device, shader, pipeline, and first present
   - fullscreen reveal
   - steady running
   - user stop
   - worker stop/join
   - window close and cleanup complete
3. Each stage must write `begin` and `end` or `error`, a monotonic duration, and a stable machine-readable code. Critical records must be flushed so an abrupt process death still leaves the last `begin` record.
4. Maintain a small atomic `last-session.json` marker in the app log directory. Mark it `inProgress` before risky startup, update its last stage, and mark it `cleanExit` only after full cleanup. On the next launch, convert an unfinished marker into an **Previous run ended unexpectedly** report.
5. Add build version, target triple, OS version/build, architecture, and selected `wgpu` backend/adapter to diagnostics. Do not log usernames, full Rekordbox paths, song/library metadata, or audio samples.
6. On Windows, document how to correlate the session timestamp with Windows Error Reporting/Event Viewer. If native faults remain possible, produce symbols for the release build. Add minidump capture only if it can be implemented safely and tested; do not let crash-reporting code introduce another crash path.

### Phase 1 — Add a standalone connection/startup diagnostic

Add a controller action named **Test connection**. It must not enter fullscreen and it must not require the main show to be running.

Implement a Tauri command such as `run_connection_diagnostic` that performs bounded probes and returns a typed `DiagnosticReport`. Add **Copy readable result**, **Copy JSON**, and **Open logs folder** actions.

Use a schema with this shape or a better versioned equivalent:

```json
{
  "schemaVersion": 1,
  "reportId": "uuid",
  "sessionId": "uuid-or-null",
  "startedAt": "RFC3339 timestamp",
  "durationMs": 1842,
  "app": {
    "version": "0.2.0",
    "platform": "windows",
    "osVersion": "...",
    "arch": "x86_64"
  },
  "verdict": "pass | degraded | fail",
  "failureStage": "audio.sampleFlow",
  "failureCode": "NO_SAMPLES",
  "summary": "Rekordbox is running and capture initialized, but no audio samples arrived.",
  "stages": [
    {
      "stage": "rekordbox.processDiscovery",
      "status": "pass",
      "durationMs": 4,
      "code": "REKORDBOX_PROCESS_FOUND",
      "message": "Found a supported Rekordbox process tree.",
      "details": { "candidateCount": 1 }
    }
  ],
  "audio": {
    "processDetected": true,
    "captureInitialized": true,
    "samplesFlowing": false,
    "route": "rekordboxProcess",
    "sampleRate": 48000,
    "channels": 2,
    "format": "float32",
    "capturedFrames": 0,
    "rms": 0.0,
    "peak": 0.0
  },
  "renderer": {
    "adapter": null,
    "backend": null,
    "shaderValidated": true,
    "surfaceTested": false
  },
  "logPath": "..."
}
```

Requirements:

- Give each probe a timeout and honor cancellation.
- The audio probe may calculate aggregate RMS/peak and frame counts, but must never save raw PCM.
- First try the selected route. If it fails, report the exact fallback attempt separately rather than replacing the first error.
- Allow **Audio only**, **Renderer only**, and **Full startup** diagnostic modes so the crash can be bisected without guessing.
- A renderer-only probe should validate the shader and adapter/device safely. A surface/fullscreen probe must use a temporary hidden window and always clean it up.
- Include recent structured events in memory using a small bounded ring, so the report is useful even before the log file is opened.
- If the process dies during a probe, the next launch should reconstruct a partial report from `last-session.json` and the last log records.

Use stable failure codes, including at least:

- `REKORDBOX_PROCESS_NOT_FOUND`
- `WINDOWS_BUILD_UNSUPPORTED`
- `PROCESS_LOOPBACK_ACTIVATION_FAILED`
- `OUTPUT_LOOPBACK_ACTIVATION_FAILED`
- `UNSUPPORTED_AUDIO_FORMAT`
- `AUDIO_CLIENT_START_FAILED`
- `NO_AUDIO_PACKETS`
- `NO_NON_SILENT_SAMPLES`
- `ASIO_BYPASS_SUSPECTED`
- `DISPLAY_WINDOW_CREATE_FAILED`
- `GPU_ADAPTER_NOT_FOUND`
- `GPU_DEVICE_CREATE_FAILED`
- `GPU_SHADER_VALIDATION_FAILED`
- `GPU_PIPELINE_CREATE_FAILED`
- `GPU_SURFACE_CREATE_FAILED`
- `GPU_FIRST_FRAME_TIMEOUT`
- `GPU_DEVICE_LOST`
- `FULLSCREEN_REVEAL_FAILED`
- `UNEXPECTED_PROCESS_EXIT`

### Phase 2 — Fix the proven Windows crash and connection failure

Do not choose a root cause from code inspection alone. Run the three diagnostic modes on the affected Windows machine and branch on evidence.

#### Renderer hardening

1. Register `wgpu::Device::on_uncaptured_error` and `set_device_lost_callback` immediately after device creation. Convert errors into structured status and request an orderly stop.
2. Use `wgpu` error scopes around shader/pipeline/resource creation and force validation completion before declaring readiness.
3. Record adapter name, backend, driver information where available, surface format/present mode, requested limits, and fallback status.
4. Recreate a genuinely lost surface; skip timeout/occluded frames; reconfigure only when that is the documented recovery.
5. Verify window/surface ownership and thread-affinity rules on both platforms. Do not call platform UI APIs from a worker thread unless the API contract permits it.
6. If an in-process native renderer fault is confirmed and cannot be contained, move the performance renderer to a supervised child/sidecar process. The controller must remain alive, collect the child's exit code/last stage, restore the display, and offer retry/safe mode.
7. Add a **Safe renderer** diagnostic mode using conservative adapter/limits/present settings. Label it clearly; do not silently degrade forever.

#### Windows audio hardening

1. Check the Windows build before attempting process loopback. Microsoft's process-loopback activation API requires a sufficiently recent Windows 10/11 build; report unsupported builds explicitly.
2. Enumerate deterministic Rekordbox process candidates and, where supportable, correlate them with active audio sessions to identify the process tree actually rendering audio.
3. Preserve numeric HRESULT values and a readable interpretation for activation, mix-format, initialization, start, packet, and device-loss failures.
4. Represent these as separate facts:
   - `processDetected`
   - `captureInitialized`
   - `packetsReceived`
   - `nonSilentSamplesReceived`
   - `reactiveReady`
5. Use an RAII guard for `GetBuffer`/`ReleaseBuffer`. Once a buffer is acquired, release it exactly once even when format conversion, resampling, queueing, device change, or cancellation fails.
6. Reuse bounded conversion buffers instead of allocating a new `Vec` for every audio packet. Keep resampler state across packets so packet boundaries do not introduce discontinuities.
7. Test rapid Start/Stop, Rekordbox restart, device removal/change, display sleep/wake, ASIO with and without PC MASTER OUT, and silence followed by signal.
8. Process capture remains preferred. Selected-output and default-output loopback remain honest fallbacks. If ASIO bypasses Windows shared output, explain how to enable PC MASTER OUT or select a capturable output; never report silence as connected.

#### Startup semantics

- `Start live visuals` may reveal ambient output after GPU readiness, but the controller must continue to say **Waiting for audio** until samples flow.
- Use **Reactive ready** only when recent non-silent samples are feeding analysis.
- Do not make audio silence kill the visual window. Fade to a deliberate ambient state and keep retrying according to a bounded policy.
- Do not make a failed renderer kill audio diagnostics or the controller.
- Start/Stop must be idempotent and race-tested.

### Phase 3 — Make display mode safely escapable

The existing React Escape listener does not solve the production native window. Implement escape handling in the native output path.

Required exit routes:

1. `Escape` while the performance window is focused stops only the visual session.
2. The normal OS close shortcut/window close request also stops only the visual session. A borderless window can still be programmatically closable; do not use `closable(false)` as the safety strategy.
3. **Stop visuals** remains available in the controller on another display.
4. Add one documented emergency shortcut that does not conflict with Windows Task Manager or common DJ controls. It should work only while PulseBridge output is running.

All routes must use the same idempotent shutdown function:

```text
request stop
  -> stop audio/analysis/renderer
  -> bound worker joins and report any timeout
  -> show cursor
  -> leave fullscreen/topmost
  -> close only the performance window
  -> refocus/show controller
  -> write clean-exit marker
```

Additional requirements:

- Handle the performance window being closed externally.
- Do not exit the whole application when only the performance window closes.
- Do not block forever on a worker join. Use a bounded shutdown protocol and report workers that fail to acknowledge it.
- Verify Escape and OS close behavior on Windows and macOS with one and two displays.

### Phase 4 — Redesign Auto as bases plus modifiers

Use 5–8 polished base families. Start with these eight unless visual review supports merging two:

1. **Wave Field** — broad traveling/standing waves with directional changes.
2. **Bloom** — a flower/petal field that opens, rotates, and folds with the groove.
3. **Pulse Geometry** — rings, cells, or shapes that breathe and strike with beats.
4. **Warp Tunnel** — forward motion and acceleration for builds and peaks.
5. **Fluid Ribbons** — sweeping colored ribbons with controllable turbulence.
6. **Prism Beams** — directional beams and refraction, not a full-screen wash.
7. **Star Trails** — particles/trails with clear rhythmic direction.
8. **Kaleidoscope** — bold symmetrical geometry reserved for higher energy.

Every base must remain recognizable on its own. During normal playback, render one base. During a base transition only, crossfade old and new bases with normalized weights.

Create modifiers as separate, budgeted controls rather than full second scenes:

- V1-style palette drift/hue travel
- beat zoom or camera punch
- bass warp/displacement
- high-frequency sparkle/particle accent
- echo/trail persistence
- mirror/kaleidoscope fold
- prism/chromatic edge split
- impact bloom or brief inversion

Rules:

- Maximum two modifiers at once; zero or one is the normal case.
- A modifier has an attack, hold, and release envelope. Do not hard-toggle shader branches on a single FFT frame.
- Modifiers must have compatibility rules per base.
- Do not run full alternative base shaders merely to obtain a modifier.
- Normalize brightness and layer weights. Additional modifiers must not multiply total luminance without a cap.
- White flash remains opt-in, brief, cooldown-limited, and unnecessary for energetic motion.
- Palette drift should recreate the appealing V1 color movement, but pause/reduce it when another strong color modifier is active.

#### Music-to-motion mapping

Make the response legible:

- beat phase: continuous rhythmic travel, rotation, or propagation
- beat pulse: short scale/camera/shape impulse
- sub/bass: large deformation, width, depth, and forward drive
- mids: primary form complexity and motion path
- highs: fine detail, sparkle, edge activity, and modest color motion
- onset: directional cut, spawn, or short accent
- impact: base transition opportunity or strong modifier envelope
- short-term energy: speed and density
- long-term trend/phrase: base selection, palette family, and overall staging

Balanced mode should never look frozen during a steady groove. Wild should increase speed, depth, and density more than flash rate. Chill may be smooth, but it still needs visible beat response.

#### Scene-direction rules

- Intro/breakdown/outro: one base, low density, visible motion, rare modifiers.
- Verse/groove: one base, rhythmic movement, rotate one compatible modifier every 8–16 bars.
- Build: increase speed/depth and introduce one modifier; preserve the base until the transition pays off.
- Chorus/drop/peak: allow two modifiers briefly, stronger beat/bass motion, and a purposeful base change.
- Fill/impact: short modifier accent instead of replacing the entire scene every time.
- Hold each base long enough to establish identity. Avoid repeating the same base three times in a row.

Keep external phrase provenance truthful. Use verified Rekordbox phrase/playhead data only when available; otherwise label direction as audio inferred.

### Phase 5 — Implement macOS audio and packaging

Create a platform capture trait so analysis and visuals receive the same normalized PCM/status model on Windows and macOS.

#### Windows backend

- Keep WASAPI process loopback as preferred on supported Windows builds.
- Keep selected/default output loopback fallbacks.
- Package NSIS as currently intended and add a signed-artifact path when signing is available.

#### macOS backend

- For macOS 14.2 or later, prefer Apple's Core Audio process taps to capture the outgoing audio of the Rekordbox process/group. Use a private, non-mutating tap and destroy all tap/aggregate objects on stop or crash recovery.
- Add `NSAudioCaptureUsageDescription` and handle the system audio-recording permission flow. Explain denial and how to reopen the relevant System Settings page.
- If a supportable older-macOS fallback is retained, ScreenCaptureKit may capture system audio with explicit user permission. Label system-output capture as a fallback; do not claim it is Rekordbox-only when it is not.
- Discover Rekordbox using supported process/bundle APIs. Never inject into Rekordbox, scrape its UI, or read private memory.
- Negotiate the native stream format and feed the existing bounded mono 48 kHz analysis contract through tested conversion/resampling.
- On unsupported macOS versions, show the exact minimum version and reason. Do not show a generic Windows-only message.

Apple's official Core Audio tap sample documents outgoing process/group capture and a macOS 14.2 minimum: <https://developer.apple.com/documentation/coreaudio/capturing-system-audio-with-core-audio-taps>. ScreenCaptureKit's audio-capture configuration is documented at <https://developer.apple.com/documentation/screencapturekit/scstreamconfiguration>.

#### Cross-platform application behavior

- Keep one shared React controller and one shared renderer/director.
- Put only source discovery, capture, permissions, OS shortcuts, and packaging behind platform modules.
- Update Tauri bundle configuration for Windows and macOS artifacts, including `.exe` installer and `.app`/`.dmg` output and platform-appropriate icons.
- Add Windows and macOS CI jobs. Do not call a compile-only CI job an audio integration test.

## Test and verification requirements

### Automated tests

- Diagnostic schema serialization and backward-compatible parsing
- Stage begin/end/error ordering and duration calculation
- Unclean-exit reconstruction from a partial marker/log
- Stable failure-code mapping from representative HRESULT, Core Audio, permission, GPU, and timeout errors
- Connection states never collapse process detection into sample flow
- Audio probe cancellation and timeouts
- WASAPI buffer release on every post-`GetBuffer` return path
- Reconnect and rapid Start/Stop races
- Renderer error callback/device-lost transitions
- Output-window close requests call session shutdown without exiting the controller
- Scene plans contain one base, except for a normalized crossfade
- No more than two compatible modifiers; modifier envelopes release cleanly
- Deterministic scene selection for a fixed seed/input timeline
- Snapshot or numeric tests for brightness caps and shader seams
- Windows and macOS format conversion/resampling fixtures

Use synthetic PCM and saved analysis-feature timelines. Do not commit copyrighted songs or raw user audio.

### Manual Windows matrix

- Rekordbox absent, present but paused, and playing
- Shared Windows output and ASIO, with PC MASTER OUT off/on
- Process route success, default fallback success, and selected-output success
- Unsupported/older Windows build if available
- Intel/AMD/NVIDIA GPU where available, hardware and safe/fallback renderer
- One display, two displays, unplug/replug, sleep/wake
- Start/Stop repeated 20 times and rapid double-click attempts
- Escape, OS close shortcut, controller Stop, and unexpected performance-window close
- Renderer-only, audio-only, and full diagnostic reports

### Manual macOS matrix

- Permission not requested, granted, denied, and revoked
- Rekordbox absent, paused, playing, and restarted
- Core Audio process tap success and any documented system-output fallback
- Apple Silicon and Intel if supported
- One display and external display; Spaces/fullscreen transitions
- Escape, OS close shortcut, controller Stop, sleep/wake

### Visual review matrix

For every base, review quiet intro, steady groove, build, drop/peak, breakdown, and silence recovery using deterministic feature timelines. Review at 16:9, 16:10, ultrawide, and projector resolutions.

Reject the build if:

- motion is barely visible during a steady Balanced groove;
- more than one base is rendered outside a crossfade;
- more than two modifiers are active;
- brightness jumps only because a modifier was added;
- white flashing is required to perceive the beat;
- a scene changes on every isolated onset;
- color motion becomes a constant rainbow wash;
- Escape or normal close can terminate the controller.

## Deliverables

1. A code fix tied to a captured diagnostic or reproducible failing test.
2. A **Test connection** workflow with human-readable and JSON output.
3. Durable unclean-exit recovery and a user-accessible previous-run report.
4. Reliable native display exit routes.
5. The base-plus-modifier visual system and energetic reaction tuning.
6. Windows and macOS audio implementations with truthful route/provenance labels.
7. Updated `README.md`, architecture, audio, reliability, visual-language, development, and transfer/build documentation.
8. Windows and macOS build artifacts or exact reproducible build commands if signing credentials are unavailable.
9. A final verification note listing machines/OS/Rekordbox versions tested, reports collected, commands run, and any remaining known limitations.

## Non-negotiable constraints

- Preserve the user's existing uncommitted work. Inspect the dirty tree before editing and avoid unrelated rewrites.
- Never write or retain raw source audio.
- Never alter Rekordbox files, databases, process memory, or UI.
- Do not claim a process is connected until capture is initialized and samples are flowing; prefer precise labels over a single connected flag.
- Do not put diagnostics, text, logos, or error overlays on the performance display.
- Do not add an unbounded queue, log, history, or retry loop.
- Do not hide failures behind an automatic fallback; report the preferred-route failure and fallback result separately.
- Do not mark the issue fixed based only on macOS unit tests or a Windows compile. Reproduce or collect evidence on the affected Windows machine.
- Do not increase strobe frequency to make the visuals feel energetic.
- Keep manual base selections and fixed palettes usable.

## Final handoff format

The implementation handoff must begin with the proven outcome, then include:

- confirmed root cause and evidence/report ID;
- exact fix and affected files;
- Windows and macOS support status;
- automated and manual tests passed;
- how to run **Test connection** and copy its JSON;
- every way to leave display mode;
- remaining limitations, especially ASIO, permissions, minimum OS versions, signing, or hardware not tested.

