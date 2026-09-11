# PulseBridge continuation context

## User intent and preferences

- Build on the existing vibrant neon / Tron visual language, not an unrelated redesign.
- Reactions must visibly move, steer, fracture, hinge, or otherwise deform individual structures. Whole-image resizing and brightness pulses alone are insufficient.
- Current request (September 10, 2026): add 2D, 3D, and Combined choices; expand both libraries, especially lively, strongly music-reactive 3D. Save context for later sessions.
- Repository AGENTS.md requires committing verified source/documentation and pushing the active branch to origin. Do not commit generated media, bundles, transfer artifacts, or build caches.
- User requested Mac installation in the previous turn. Installed and launched /Applications/PulseBridge.app from 94d0096; prior bundle is backed up in ignored transfer-ready/PulseBridge-before-tron-20260910-214847.app.

## Verified predecessor (before the dimension expansion)

- Branch main, GitHub ishaansolanki9/PulseBridge-Visuals, last pushed source 94d0096.
- 32 music-directed Auto scenes: IDs 0–25 analytic 2D illusions, 26–31 instanced 3D line worlds.
- Tron: ID 32 cycles 12 looks; held IDs 33–44. Native src-tauri/shaders/tron.wgsl; generated WebGL app/src/visuals/tronShader.ts via python3 scripts/sync-tron-shader.py.
- Tron has signature cyan/blue/orange colors; regular scenes use the palette control. Flash immunity and full-black output are intentional.
- Native shader = performance.wgsl + spatial.wgsl + tron.wgsl. The line draw pass must explicitly include only instanced families, never every ID >= 26.
- Existing ReactionHistory keeps ~1 second of independent bass/mid/high/energy envelopes at 30 Hz. recent_signal(age) interpolates this; object/position-delayed samples make hits travel after live envelopes decay.
- Native renderer integrates spatial.x for continuous motion; do not multiply absolute time by live energy. The expansion now uses chromatic.w for dimension (0 Combined, 1 2D, 2 3D) without resizing uniforms.
- Browser preview deliberately injects no audio; native GPU audition exercises actual production smoothing/history with synthetic inputs.

## Verification and packaging

- Match CI before pushing: `npm run lint`; `npm run typecheck`; `npm run build`; `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`; `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`; `cargo test --manifest-path src-tauri/Cargo.toml`. Clippy must include all targets: ignored GPU audition tests are still compiled and linted.
- GPU: cargo test --manifest-path src-tauri/Cargo.toml native_tron_audition -- --ignored --nocapture. Tests all 12 looks for independent bands with zero live envelopes at two positions in history, normalized image differences, flash immunity, blackout, cycle equivalence.
- Motion: native_tron_motion_audition writes six four-second clips and warmed 1280x720 timings. Baseline Apple M1/Metal medians ~1–6ms; this is not live Rekordbox or Windows performance validation.
- Generated captures: ignored src-tauri/target/tron-motion and tron-audition. Animated preview: src-tauri/target/tron-motion/tron-preview.gif.
- Build Mac: npm run tauri -- build --bundles app. Ad-hoc sign, verify, preserve old /Applications/PulseBridge.app, copy new bundle, verify executable hashes, open app and check process. Signing here is local, not notarization.
- Restore generated app/tsconfig.tsbuildinfo after checks. Do not revert user source changes.

## Current implementation (September 11, 2026)

- Global `dimension` setting: `combined` (default/migration), `twoD`, `threeD`. The control panel filters choices and resets incompatible held selections to Auto. Native sanitization enforces the same constraint; the director clears stale transitions and ranks compatible phrase candidates, with a compatible-library fallback.
- Combined transitions between both kinds; it does not layer unrelated scenes permanently. Native image crossfades remain in place.
- 52 distinct looks in total: 40 Auto families (0–31 plus 45–52) and the original twelve explicit Tron looks (33–44). ID 32 remains the Tron collection.
- New planar neon designs: Neon Mandala (45), Plasma Weave (46), Spectrum Bloom (47), Chromatic Moiré (48). These use cyan/orange plus magenta accents.
- New true 3D instanced worlds: Orbit Foundry (49), Synapse Bloom (50), Gravity Braids (51), Prism Conveyor (52). Only 26–31 and 49–52 use the instanced line pass. Never route all IDs >=26 into it.
- Original six 3D worlds have stronger object/position-specific deformation via `line_signal(age)`, using history plus a bounded immediate response. New worlds hinge gimbals, unfurl tendrils, peel braided fibers, and tumble/split prism cages. Global beat-resizing is not the requested aesthetic.
- Tron cycles through 16 variants in Combined, 9 planar variants in 2D, and 7 perspective variants in 3D. `tron_variant` in WGSL defines the order, shared with generated WebGL. New instanced 3D worlds are in Auto/the 3D selector rather than the analytic Tron collection.
- `SpatialPreview.tsx` renders quiet CPU-projected counterparts of all ten line worlds. Native GPU output supplies the audio reactions. The shader preview continues to inject no audio.
- `VisualFamily::is_spatial` and frontend `sceneMatchesDimension` classify perspective scenes by ID. Keep them aligned when adding scenes. Rust `selection_family` centralizes held-scene mapping for the director and settings sanitization.
- Tests passed: frontend build/lint; 83 regular Rust tests; native extended held-scene response audition (33–52); dimension-cycle equivalence through wraparound in all modes; stronger original 3D isolated-band responses; expanded synthetic motion captures. Browser verified 2D/3D/Combined filtering, incompatible-hold reset, 3D preview, persisted mode, and no console errors.
- Expanded native captures: `src-tauri/target/expanded-motion`. Eight 2.5-second sequences and `expanded-preview.gif`; ignored artifacts, never commit them. On this Apple M1, warmed native 720p medians were ~1.1ms for the four new line worlds and ~1.2–3.3ms for the four planar designs. These timings exclude readback and presentation; live Rekordbox and Windows are not verified.
- Extra ignored test names: `native_dimension_cycle_audition`, `native_expanded_motion_audition`. `native_tron_audition` now covers 20 held looks, so its historical name is broader than its name suggests.
- Dimension expansion source and installed Mac build: `3ec1c8b`. Subsequent CI maintenance is recorded below; future sessions can find the latest source revision in git history.

## Native CI failure (September 11, 2026)

- [Run 34574916152](https://github.com/ishaansolanki9/PulseBridge-Visuals/actions/runs/34574916152) failed on both macOS and Windows at the strict all-target Clippy step. `clippy::collapsible_if` flagged nested conditions in the ignored Tron GPU audition's traveling-hit assertion. Tests and installer packaging were skipped; Node.js action deprecation notices were unrelated warnings.
- Combined the conditions with short-circuit `&&`, preserving the assertion's behavior and avoiding a comparison before a previous image exists. No runtime visual or application behavior changed. Keep the workflow's strict lint settings intact.
- Local verification after the fix: formatting and strict all-target Clippy passed; full Cargo tests passed (83 passed, 8 GPU auditions intentionally ignored); frontend lint, typecheck, and production build passed.

## Installation status

Built the expanded Mac release, ad-hoc signed and signature-verified it, installed at `/Applications/PulseBridge.app`, verified its executable hash against the new build, and launched it. Previous installed app (94d0096) retained at `transfer-ready/PulseBridge-before-dimensions-20260911-033238.app`. Existing user preferences remain in place.

## Windows controller startup freeze (September 11, 2026)

- User reported an immediate freeze on opening the latest checkout on their Windows laptop, before Start visuals. Opening the Vite URL in Opera also became unresponsive. No Windows GPU model or crash log was supplied; the exact driver-level cause remains unconfirmed.
- Found a shared startup hazard: the controller synchronously checked shader compilation/linking for a monolithic WebGL program containing all Tron looks, collection cycling, and multiple scene call sites. React development startup and loading persisted settings could also cause redundant compilation. Native audio enumeration was a separate synchronous command that could block the desktop window on a slow audio driver.
- Browser previews now submit a single specialized look at a time. `scripts/sync-tron-shader.py` generates `tronHelpers` and sixteen `tronLookShaders` from the same WGSL, stripping irrelevant look branches before the driver sees them. Auto compiles its ambient tunnel without any Tron functions. Never restore the monolithic `tron_scene` dispatch in browser shader source.
- `previewProgram.ts` uses `KHR_parallel_shader_compile` completion polling before any link-status query. Missing support, link failure, an eight-second pending compilation, or context loss disables only the preview and leaves a visible fallback. No synchronous compilation fallback. Initialization is deferred until a frame after saved settings arrive, avoiding duplicate development-mode compilation. At most one compile is pending and at most three programs are retained.
- `previewBudget.ts` caps both shader and spatial illustrations at 640×360, 15 FPS, with no high-DPI multiplier; suspends hidden/offscreen work. Native output and connection diagnostics pause the preview. The native shaders, scene detail, and performance-output resolution remain unchanged. Browser collection dissolves use two independently compiled programs; native linear image crossfades remain as before.
- Audio-source enumeration now runs on a blocking worker. Initial settings/displays do not wait on audio enumeration; polling and manual refresh share one in-flight discovery request.
- Added `npm run test:preview` and pinned Playwright for nine browser regression tests. CI installs Chromium and runs these. Locally use `PULSEBRIDGE_TEST_BROWSER=chrome npm run test:preview`; covers all held scenes, saved look startup, unsupported/stalled/failed GPU initialization, context loss, offscreen suspension, dimension cycle wraparound, and a stalled native audio-discovery transport. Normal frontend checks and 83 native unit tests also pass (eight GPU auditions intentionally ignored).
- This is a verified mitigation for the identified startup blocking paths, not a claim of reproducing the user's exact Windows/Opera driver crash. Ask the user to retest after pulling and running `npm.cmd ci`, then `npm.cmd run tauri -- dev`; use the desktop window, not the Vite URL, for live capture. The installed Mac application has not been replaced by this source fix.
