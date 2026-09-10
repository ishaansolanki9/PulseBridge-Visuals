# PulseBridge 0.1.6: color, musical selection, and smooth transitions

This release completes the color and Auto-direction work begun in 0.1.5 and replaces the mixed 2D/3D transition path. It includes all six reactive line worlds from 0.1.4.

## Visible changes

- **Traveling color:** colored bands move along the 3D strands using an independent integrated clock and the live band history. Hue interpolation uses the short path around the color wheel, preserving saturation between palette colors. The existing Color change control adjusts the travel rate and reactive color displacement. At zero, those movements stop; an Auto palette can still change with a musical section.
- **Section-directed palettes:** Auto chooses cool palettes for quiet sections, warmer palettes for builds, and Neon/Rainbow Flow for peaks. Groove and bridge choices also use the dominant frequency band. A manually held scene still receives section-directed colors. A manually selected palette stays within that palette's colors.
- **Musical scene matching:** a 1.5-second smoothed profile ranks eligible scenes by energy, bass/mid/high balance, and articulation. Bass-heavy passages favor the mesh and terrain, mid-heavy passages favor ribbons and flowing scenes, and sharp percussion favors fractured geometry. Phrase eligibility, recent-scene avoidance, and composition variety still apply. Small deterministic tie-breaking cannot outweigh a strong musical fit.
- **Deliberate timing:** a spectral change must persist for three seconds and satisfy the scene dwell before it can create a new inferred boundary. Short fills do not shuffle scenes. Confident changes wait for a bar or impact; weak timing uses a slower dissolve. A confirmed build-to-drop hit can resolve a build after three seconds. Audio loss cannot drive periodic scene rotation.
- **Smoother 2D ↔ 3D changes:** each scene renders into its own cached target, with its own pattern treatment, before a linear-light dissolve combines the completed images. The outgoing scene keeps its palette, pattern seed, budgets, and effects while live audio and time continue. The incoming scene cannot suddenly change the outgoing scene's appearance.

Auto normally keeps a scene at least eight seconds, or twelve before crossing between 2D and 3D. The confirmed-drop exception and deliberate manual scene changes can act sooner. Mixed transitions take at least 3.6 seconds in Chill, 2.8 in Balanced, and 2.0 in Wild. The quintic easing curve starts and finishes gently. A queued choice remains stable until the handoff, and does not change the visible seed or motion budgets while waiting.

The original 2D travel clock is now integrated too. Changing a scene's speed no longer multiplies the entire elapsed session time by a new speed and jumps to another animation phase. Internal resolution and line width remain stable during transitions; sustained frame overload still activates the existing adaptive fallback. The normal internal ceiling is 1080p.

These are local signal-based rules, not a remote AI service, stem separator, or access to private Rekordbox phrase metadata. Their musical decisions still need to be judged against real tracks. The six line worlds retain their geometry responses, brightness controls, and immunity to full-screen white flashes.

## Review and installation

Use **Visual scene → Auto** and **Palette → Auto** to try the combined direction. Select **Ribbon Reactor** or **Bass Web** to isolate the color changes. The browser controller preview remains an ambient illustration of the original style; the native performance output shows the actual changes.

The versioned Mac DMG is `transfer-ready/PulseBridge-0.1.6-Mac.dmg`. To update an installed copy, quit PulseBridge, open the DMG, drag PulseBridge to Applications, choose Replace, then open the app from Applications. Existing saved preferences remain compatible. The bundle is locally ad-hoc signed, not Apple-notarized.

The Windows source ZIP is `transfer-ready/PulseBridge-0.1.6-Windows-source.zip`. Extract it on Windows, open PowerShell in the extracted project, then run:

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\build-windows.ps1
```

This builds the version-matched NSIS installer. Cross-target compilation on macOS is not Windows installer, DirectX, or live Rekordbox validation. Previously generated installers and DMGs for older versions do not contain this transition fix. No GitHub push is part of this local review.

## Verification

Normal Rust tests cover musical rankings at equal loudness, transient rejection, sustained spectral changes, build/drop timing, audio loss, manual holds, confidence fallback, queued-choice stability, outgoing appearance stability, transition residence/easing, and shader validation. Frontend lint, type checking and production build, Rust formatting and Clippy, and Windows x64 cross-target checks are part of the release checks.

Four ignored native GPU auditions use the production pipelines and explicit synthetic inputs:

1. Six temporal scene sequences, fallback detail, and transition timings.
2. Isolated bass/mid/high deformations, white-flash immunity, and black output.
3. Color-phase changes with fixed geometry, plus smooth palette changes.
4. Six 2D/3D transitions in both directions, with distinct incoming palettes and seeds. Endpoints and intermediate images must match the linear-light blend of independently rendered frames to within two 8-bit channel values.

Run them sequentially to avoid concurrent GPU workloads affecting the timing samples:

```bash
PULSEBRIDGE_AUDITION_DIR="$PWD/transfer-ready/smooth-transitions-audition" \
  cargo test --manifest-path src-tauri/Cargo.toml native_ -- --ignored --nocapture --test-threads=1
python3 scripts/render-audition.py transfer-ready/smooth-transitions-audition
```

The optional conversion script needs Pillow. Generated GIFs, still comparisons, and measurement reports are linked from `review.html` in the ignored audition directory. Timing samples include GPU submission and completion wait, but exclude presentation, live audio capture, and Rekordbox load; they do not guarantee end-to-end performance.

The September 10, 2026 Apple M1/Metal run passed 80 unit tests and all four native auditions. At 1080p, single-line scenes measured 2.04–2.45 ms median. The two-line-scene dissolve measured 2.58 ms median and 5.67 ms p95. The six mixed-direction image comparisons passed at every checked blend weight, including exact scene endpoints within the stated quantization tolerance. Frontend checks, strict Clippy, both WGSL validators, and Windows x64 cross-target checks also passed. Live musical suitability and Windows hardware behavior still require listening/viewing on those systems.
