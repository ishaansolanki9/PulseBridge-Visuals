# Architecture

PulseBridge separates control, capture, analysis, and rendering so a slow webview cannot delay the TV output.

```text
Rekordbox process / supported system output / selected macOS input
                    |
      WASAPI or Core Audio capture thread
                    |
        bounded lock-free mono PCM ring
                    |
        FFT + rhythm + state worker --------> bounded audio-inferred phrase provider
                    |                                      |
      one latest AnalysisSnapshot                 one PlaybackContext
                    |                                      |
                    +------------> SceneDirector <---------+
                                         |
                 one of 28 illusions + optional crossfade + 0–2 modifiers
                                         |
                              native wgpu renderer
                                         |
                    hidden-until-ready performance window

React control webview ── settings/status only
```

`PerformanceManager` owns exactly one session and an explicit `stopped → starting → running/recovering → failed` lifecycle. Start creates a hidden selected-monitor window, ring, analysis worker, audio worker, and renderer worker. It returns success only after the GPU surface, adapter, device, shader pipeline, and first presented frame are ready. Only then is the window shown and focused. Any earlier failure signals and joins workers, restores/ closes the hidden window, retains a concise error, and leaves the controller alive.

The audio worker writes only converted mono samples to a fixed-capacity queue. The analysis worker drains current audio, discards excessive backlog, maintains bounded feature history, and replaces one snapshot. The renderer never waits for audio and never queues visual events. Missing audio fades reactivity over 300 ms–2 s and then continues quiet ambient motion.

Renderer time is monotonic seconds since performance start, not frame count. User parameters and musical parameters use asymmetric attack/release envelopes. A dedicated audio-drive envelope converts energy and spectral events into a proportional motion/color dial. The `SceneDirector` keeps macro identity stable long enough to read, shuffles across all 28 illusions on a profile-dependent cadence, emits one family except for normalized old/new crossfades, and schedules at most two compatible modifier envelopes.

Native JSON-line logs rotate in the platform log directory. A synchronously replaced `last-session.json` records the active report and last risky stage. An unfinished marker becomes `previous-run-report.json` on the next launch. Runtime status and reports separate process detection, capture initialization, packet receipt, non-silent signal, reactive readiness, route, phrase provenance, lifecycle, and renderer state.

`run_connection_diagnostic` is independent of the show session. Audio-only uses the real selected backend with bounded waits and no PCM persistence. Renderer-only and Safe renderer validate an adapter/device/shader/pipeline without fullscreen. Full startup also creates a temporary hidden native surface, presents a frame, signals stop, bounds the join, and closes it.

The performance surface is a raw Tauri window passed directly to `wgpu`; it is not a webview. Native output is paced at 45 FPS and display sizes above 2560×1440 use a proportional performance surface that the compositor scales to the physical monitor. The controller destroys its WebGL preview while output is live. The browser-only performance route is a text-free ambient appearance preview and does not participate in production capture or rendering.
