# Rekordbox phrase integration

## Decision

PulseBridge currently uses `AudioInferredPhraseProvider`. The controller labels this source **Audio inferred** and explicitly says it is not Rekordbox phrase data.

No documented, supportable live Rekordbox interface was found that supplies all of the information PulseBridge would need: phrase boundaries and kinds, a stable current-track identity, active/master deck, live playhead, beat grid, and track/deck changes. PulseBridge therefore does not read Rekordbox databases or analysis files, inspect its UI, inject into its process, or infer track identity from a title.

This boundary is intentionally optional. A future documented provider can publish a `PlaybackContext` without changing capture, analysis, direction, or rendering. Metadata older than five seconds is rejected for direction; the current scene is held briefly before audio-inferred direction resumes.

## Official interfaces investigated

| Interface | Phrase boundaries/kinds | Current track/deck | Live playhead/grid | Stability and failure mode |
| --- | --- | --- | --- | --- |
| [rekordbox for Developers](https://rekordbox.com/en/support/developer/) playlist XML | No | No | No | Documents XML playlist import into Rekordbox, not a live outward API. |
| [Phrase Edit guide 5.1](https://cdn.rekordbox.com/files/20200312172204/rekordbox5.1.0_Phrase_Edit_operation_guide_EN.pdf) and [Phrase Edit guide 7.0.5](https://cdn.rekordbox.com/files/20241203210634/rekordbox7.0.5_Phrase_Edit_operation_guide_EN.pdf) | Confirms Intro, Up, Down, Chorus, Bridge, Verse, and Outro inside Rekordbox | Shows analyzed tracks loaded on decks | Visible inside Rekordbox, but no documented programmatic feed | Product/UI documentation only. It does not define a read API or a stable file contract. |
| [Lighting guide 6.5.2](https://cdn.rekordbox.com/files/20210602085902/rekordbox6.5.2_lighting_operation_guide_EN.pdf) | Confirms phrase-analyzed tracks drive Rekordbox Lighting | Internal PERFORMANCE-mode deck | Internal playback/lighting behavior | No documented third-party live phrase interface. |
| [Current manuals and downloads](https://rekordbox.com/en/support/manual.php) | Product analysis/editing behavior | Product UI behavior | Product UI behavior | No public live phrase/playhead contract identified. |

The investigation was refreshed on 2026-08-27. It was a documentation review, not a successful live API integration.

## Tested versions

- Local implementation and automated tests: macOS development host, Rust Windows cross-check target `x86_64-pc-windows-msvc`.
- Rekordbox version on target Windows hardware: **not yet tested**.
- Windows build/version on target DJ laptop: **not yet recorded**.
- Guides reviewed: Rekordbox 5.1 Phrase Edit, Rekordbox 6.5.2 Lighting, Rekordbox 7.0.5 Phrase Edit, and the current developer/manual pages.

The installed Windows test must record the exact Windows build, Rekordbox version, selected process PID/route, negotiated audio format, and fallback result in `pulsebridge.log`.

## Audio-inferred provider

The fallback uses a bounded, downsampled observation history covering approximately one minute. It combines longer-horizon energy change, onset activity, existing hysteretic musical state, impact confidence, and minimum/maximum phrase dwell. It publishes:

- an inferred phrase kind and monotonically increasing phrase index;
- confidence and estimated phrase progress;
- `AudioInferred` provenance;
- session-relative position, without claiming a Rekordbox track or deck identity.

The scene director uses this only for macro timing. Live FFT, onset, beat, bass, mid, and high features continue to animate the selected scene. The history is capped at 256 observations and contains no audio samples.

## Future provider acceptance

A future Rekordbox provider must be documented, live, read-only, version-gated, and able to match phrase data to current playback using a stable identifier—not title text. Unknown versions fail closed. Provider errors must not stop WASAPI or rendering, and runtime diagnostics must preserve the exact provenance.
