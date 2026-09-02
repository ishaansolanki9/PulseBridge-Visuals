# Reliability model

- Capture, analysis, rendering, and React control run independently.
- PCM, feature history, and state handoff are all bounded. No visual-frame queue exists.
- If analysis falls more than roughly half a second behind, it discards old PCM and resumes from current audio.
- The renderer reads one latest snapshot, uses a monotonic clock, and continues ambient animation without fresh audio.
- React lag, minimization, or polling failure cannot stall the native renderer.
- Startup is transactional. The performance window stays hidden until `wgpu` presents its first valid frame; a 12-second readiness timeout or any window/GPU/worker failure rolls back workers, cursor/fullscreen state, and the window.
- Surface resize and recoverable swapchain errors are handled inside the renderer; a genuinely lost surface is recreated. Validation error scopes, uncaptured-error handling, and device-lost callbacks request orderly stop and record adapter/backend/driver details.
- Process and device capture failures enter a reconnect loop. On Windows, the Rekordbox-aware normal route uses the proven output-loopback path; experimental process activation keeps its COM operation/handler on the originating MTA thread until a late callback completes instead of applying an unsafe timeout. Process candidates are deterministic, fallback route is explicit, and initialized/silent differs from samples flowing.
- Flash is Off by default. Moderate and High remain bounded by impact cooldown and user strength.
- The laptop retains immediate Reactive, Ambient, Black screen, and Stop controls.
- Windows display/system sleep is prevented only for the lifetime of the renderer thread and restored automatically afterward.
- Escape while focused, the normal OS close action, controller Stop, and the documented emergency chord all request the same idempotent session stop. Joins are bounded to two seconds per worker; cleanup restores cursor, fullscreen, topmost, controller visibility/focus, and closes only the performance window.
- Settings are sanitized and written through a synchronized temporary file before replacement. Audio content is never persisted.
- Overall lifecycle, renderer state, audio route/sample flow, phrase provenance, and process detection are independent status fields. A found process never means samples or phrase metadata are connected.
- Worker bodies catch panics, record a failure, signal peer workers, and allow the polling controller to reap a stale session. Session locks are not held while joins or GPU initialization block.
- Native surface creation and lost-surface recreation are marshalled through the platform UI thread. This is required on macOS because accessing the performance window's `NSView` from the renderer worker panics before Metal adapter selection; GPU drawing remains on the renderer worker after the surface is prepared.

Diagnostic files are `pulsebridge.log`, `.1`, `.2`, and `.3` in the platform application log directory. Each file rotates at approximately 1 MB. `last-session.json`, `latest-diagnostic.json`, per-report JSON, and `previous-run-report.json` are atomically replaced and flushed around risky stages. Records contain session/report IDs, RFC3339 and monotonic times, stage begin/pass/degraded/error, stable codes, and bounded recent events—never samples, library contents, track paths, usernames, or device IDs.

The original Windows whole-process termination was reproduced when Rekordbox process-loopback activation and renderer startup overlapped. Durable records show the process activation began but never returned; Windows Error Reporting recorded `0xC0000374` (`STATUS_HEAP_CORRUPTION`). The same renderer and explicit output-loopback route subsequently ran and stopped cleanly. Normal builds therefore bypass process-specific activation, while its repaired lifetime path remains opt-in pending a symbolized dump and affected-machine validation. To correlate any new native failure, copy the report/session ID and timestamp, then open **Event Viewer → Windows Logs → Application** and find an Error at that time for `PulseBridge Visuals.exe`, `Application Error`, or Windows Error Reporting. Record the faulting module, exception code, and app version. Release builds retain debug information and are not stripped so symbols can be archived with a signed release.

## Hardware validation

The complete audio/display path requires real Windows and macOS Rekordbox tests; CI is compile/package validation only. Use the matrices in the V2 brief for repeated Start/Stop, silence, process exit/restart, route/permission changes, display disconnect, sleep/wake, Escape/close/emergency exit, and representative GPUs/displays.

Before relying on it for an event, run a representative playlist for at least the party’s intended duration. Monitor Task Manager for continuously increasing memory, observe audio-to-light latency, and confirm a capture interruption produces ambient motion rather than a freeze, desktop, text error, white flash, or replay of old impacts.
