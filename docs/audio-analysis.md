# Audio analysis

## Capture

On Windows, PulseBridge first activates the virtual process-loopback device for the detected Rekordbox process tree. It requests shared-mode stereo float PCM at 48 kHz and downmixes packets to mono. If process activation fails, it opens the default render endpoint in loopback mode. A selected render endpoint can be captured directly.

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

Ableton Link is not required. Audio-derived beat timing remains the active clock so the product works with streamed or local Rekordbox playback by itself.
