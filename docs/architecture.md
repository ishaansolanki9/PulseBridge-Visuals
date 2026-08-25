# Architecture

PulseBridge separates control, capture, analysis, and rendering so a slow webview cannot delay the TV output.

```text
Rekordbox process / selected Windows output
                    |
             WASAPI capture thread
                    |
        bounded lock-free mono PCM ring
                    |
        FFT + rhythm + state worker
                    |
      one latest AnalysisSnapshot (RwLock)
                    |
           smoothing + Auto direction
                    |
        independent native wgpu renderer
                    |
 borderless selected-display performance window

React control webview ── settings/status only
```

`PerformanceManager` owns exactly one session. Start creates the selected-monitor window, ring, analysis worker, audio worker, and renderer worker. Stop signals one atomic flag, joins the workers, restores the cursor, releases the Windows power request, and closes the performance window.

The audio worker writes only converted mono samples to a fixed-capacity queue. The analysis worker drains current audio, discards excessive backlog, maintains bounded feature history, and replaces one snapshot. The renderer never waits for audio and never queues visual events. Missing audio fades reactivity over 300 ms–2 s and then continues quiet ambient motion.

Renderer time is monotonic seconds since performance start, not frame count. User parameters and musical parameters use asymmetric attack/release envelopes. Auto mode crossfades five procedural layers rather than cutting between presets.

The performance surface is a raw Tauri window passed directly to `wgpu`; it is not a webview. The browser-only performance route is a text-free ambient appearance preview and does not participate in production capture or rendering.
