# Audio analysis

## Capture

On Windows, PulseBridge enumerates every `rekordbox.exe` candidate, sorts root processes before descendants and PIDs deterministically, and attempts the virtual process-loopback device for each candidate. If activation or live samples fail, it explicitly opens the default render endpoint in loopback mode. A selected render endpoint can be captured directly.

The capture client asks WASAPI for the actual shared-mode mix format. It accepts 32-bit float and 16/24/32-bit integer PCM, validates channel count/rate/block alignment, downmixes all channels safely, and uses a stateful streaming converter for the fixed 48 kHz analysis rate. A `GetBuffer` RAII guard releases each WASAPI packet exactly once on success, conversion failure, cancellation, or device failure; bounded conversion vectors are reused.

On macOS 14.2+, PulseBridge can either discover Rekordbox through `NSWorkspace` and tap only its process, or create a private global stereo tap for all outgoing system audio. Both use unmuted behavior and a private aggregate device. Tap, IO proc, aggregate, callback context, and Objective-C description are destroyed in reverse order. Physical microphone and line-in devices are enumerated with stable Core Audio IDs and opened only when selected. Every route is downmixed and statefully resampled to the same mono 48 kHz contract. System-audio permission denial points to **System Settings → Privacy & Security → Screen & System Audio Recording**; input denial points to **Microphone**. No fake or undocumented older-system output fallback is claimed.

All available packets are drained on each capture poll. Samples go into an overwriting `ArrayQueue`: 5–30 seconds at 48 kHz, 10 seconds by default. When full, the oldest sample is removed. No captured samples are logged, uploaded, or written to disk.

## Features

The analysis worker uses a 2048-sample Hann FFT with a 960-sample hop (20 ms at 48 kHz). It calculates:

- RMS loudness
- sub energy, approximately 20–80 Hz
- bass energy, 80–250 Hz
- mid energy, 250–2500 Hz
- high energy, 2500–12000 Hz
- positive spectral flux and onset strength
- fast and slow energy envelopes
- onset-derived beat phase and pulse confidence

Each energy channel has a bounded rolling normalizer based on recent low, median, and high percentiles. This makes reaction relative to the material instead of relying on one mastering-level threshold.

## Musical state

State inference compares fast/slow energy trends, frequency balance, onsets, beat confidence, bass return, and recent state. Candidate states have dwell times to avoid flicker. Impacts use a score, a 1.25-second cooldown, and a short release hold.

The latest frame maps to Quiet, Flow, Groove, Build, Impact, Peak, or Breakdown. The renderer sees only the newest snapshot. It will not replay missed states after a stall.

## Audio loss

Freshness is separate from the feature values:

- Through 300 ms without samples, the current response remains intact.
- From 300 ms to 2 seconds, reactivity fades continuously.
- After 2 seconds, the renderer uses a quiet ambient frame while the capture thread reconnects.

Audio return fades back through the normal envelopes. The performance screen never displays capture errors.

Process detection, client initialization, first packet, non-silent signal, and reactive readiness are separate states. Silence is never labeled connected/reactive. Process capture retries candidates/fallback rather than presenting silence as a successful connection. A silent default-output fallback periodically retries the preferred Windows process route; a user-selected output remains selected. macOS releases and recreates the selected tap or input stream if the source exits, disconnects, or changes.

## Phrase direction

See [rekordbox-phrase-integration.md](rekordbox-phrase-integration.md). Because no documented live Rekordbox phrase/playhead API is used, a bounded longer-horizon provider labels its output `AudioInferred`. Phrase kinds select macro scenes; FFT/beat features only animate motion and accents inside the selected scene.

Ableton Link is not required. Audio-derived beat timing remains the active clock so the product works with streamed or local Rekordbox playback by itself.
