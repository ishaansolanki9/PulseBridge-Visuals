# Reliability model

- Capture, analysis, rendering, and React control run independently.
- PCM, feature history, and state handoff are all bounded. No visual-frame queue exists.
- If analysis falls more than roughly half a second behind, it discards old PCM and resumes from current audio.
- The renderer reads one latest snapshot, uses a monotonic clock, and continues ambient animation without fresh audio.
- React lag, minimization, or polling failure cannot stall the native renderer.
- Surface resize and recoverable swapchain errors are handled inside the renderer; fatal details are saved for the laptop window only.
- Process and device capture failures enter a reconnect loop. Rekordbox process activation can fall back to default-output capture.
- Flash is Off by default. Moderate and High remain bounded by impact cooldown and user strength.
- The laptop retains immediate Reactive, Ambient, Black screen, and Stop controls.
- Windows display/system sleep is prevented only for the lifetime of the renderer thread and restored automatically afterward.
- Stop signals every worker, joins it, restores the cursor/fullscreen state, and closes only the performance window.
- Settings are sanitized and written through a synchronized temporary file before replacement. Audio content is never persisted.

## Hardware validation

The complete audio/display path can only be validated on the target Windows laptop. Use real Rekordbox playback for repeated Start/Stop, silence, process exit/restart, output-device change, HDMI disconnect/reconnect, sleep/wake, 1080p, and 4K checks.

Before relying on it for an event, run a representative playlist for at least the party’s intended duration. Monitor Task Manager for continuously increasing memory, observe audio-to-light latency, and confirm a capture interruption produces ambient motion rather than a freeze, desktop, text error, white flash, or replay of old impacts.
