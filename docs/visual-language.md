# Visual language

The performance display is treated as a room-scale light source, not a software interface. It always renders edge-to-edge color and motion with no labels, meters, transport controls, logos, or diagnostic overlays.

## Bases and modifiers

Auto directs 32 scenes. Its original 26 motion-first analytic illusions remain available: **Warp Spiral**, **Moiré Rings**, **Infinite Checker**, **Neon Lattice**, **Twisted Stripes**, **Rotating Snakes**, **Hyperbolic Tunnel**, **Chromatic Maze**, **Vortex Chevron**, **Glass Orbit**, **Sine Interference**, **Impossible Cubes**, **Polar Fan**, **Gravity Lens**, **Ribbon Wormhole**, **Quantum Weave**, **Fractal Compass**, **Liquid Circuit**, **Alien Heads**, **Prism Vortex**, **Diamond Drift**, **Orbital Mesh**, **Helix Portal**, **Radial Escalator**, **Electric Topography**, and **Event Horizon**. The director groups them by musical role: atmospheric scenes for intros/outros/breakdowns, flowing or geometric scenes for verses, tunnels/escalation for builds, high-impact scenes for choruses/fills, and textural scenes for bridges. Smoothed energy, frequency balance, and articulation rank eligible scenes, with the live structure signature and phrase index breaking close ties; there is no cadence-based random scene change. The controller can also hold any of the six reactive line worlds, retaining audio-driven deformation and color. The only time two families render is a short normalized incoming/outgoing crossfade, and transitions prefer impact or four-beat boundaries.

Independent modifiers provide V1-style palette travel, beat zoom, bass warp, high sparkle, echo trails, mirror fold, chromatic edge split, and impact bloom. Each has attack/hold/release, compatibility rules, and a bounded strength. Zero or one is normal; peaks may briefly use two. Modifiers do not invoke an unrelated base shader, and the luminance cap tightens as modifier load rises.

Intro/breakdown/outro use low-density motion and rare Palette Drift/Echo Trails. Verse/groove phrases favor Beat Zoom, Palette Drift, or High Sparkle when a musical trigger arrives. Builds increase speed/depth and favor Bass Warp or Chromatic Split. Drops may use two modifiers briefly and offer a purposeful base transition. A short impact adds Impact Bloom instead of replacing the base on every onset. Minimum dwell and bounded history prevent three consecutive selections of the same base.

## Color

Every palette contains four related colors: a dark foundation, two primary light colors, and an accent. Automatic color direction uses Ocean/Purple Blue for quiet sections, Sunset/Warm for builds, and Neon/Rainbow Flow for peaks. Groove and bridge choices also use dominant frequency balance. Held scenes retain musical color direction. The 3D strands sweep through all three bright palette colors using a separate integrated phase and saturated hue interpolation. Fixed choices also include Purple + blue, Warm, Monochrome, and Rainbow flow.

Palette lookup is genuinely cyclic across four equal intervals: A→B→C→D→A. Angle-derived coordinates use whole periodic angular frequencies or cyclic palette coordinates, removing the negative-X `atan2` seam mathematically rather than masking it.

## Motion and safety

Chill, Balanced, and Wild set the ceiling of one proportional audio-drive dial. The dial combines rolling-normalized energy, bass, mids, highs, beat, onset, and impact, then uses a fast attack and slower release. Four short-lived reactive lanes sit on top of that continuous dial: sub/bass hits produce radial geometry waves and depth jumps, midrange movement bends and ribs the geometry, high-frequency transients slice the image and emit colored shards, and whole-band energy rises increase scale, density, and light. Those lanes use independent attack/release envelopes, so visible motion follows instrumentation rather than merely speeding up a prerecorded-looking animation. Wild permits the full 0–100% range but does not force constant maximum movement. White flash is opt-in at Off, Moderate, or High for original patterns and does not change the motion ceiling. The six line worlds bypass white flashes entirely.

Each ScenePlan includes motion, detail, density, brightness, two normalized base weights, and two modifier slots. The proportional dial controls continuous travel, while frequency-specific transient lanes visibly alter shape, position, depth, density, and color on top of it. Beat and impact can also align scene and modifier changes with musical boundaries. The spatial library uses world-space reactions and bypasses the original screen-space warps, ribs, shards, and overlays to preserve perspective and silhouette. A luminance budget is applied before tone mapping. The native renderer targets tear-free 60 FPS. Its presentation surface always matches the physical window, while high-resolution displays use a separate HD render target with linear upscaling; this avoids platform-dependent partial-frame behavior without quadrupling the expensive shader work. Screen-space detail filtering fades unresolved line frequencies before they can turn into blocky pixels.

## Reactive line worlds (0.1.4)

These replace the six 0.1.3 spatial scenes. Their stored identifiers remain unchanged so saved scene choices continue to work. Each world contains 48 continuous strands, each split into 96 perspective-projected ribbon segments with a narrow luminous core and soft colored halo. The camera is steady enough to make deformation legible. No full-screen white flash is used.

| Scene | Bass | Mids | Highs |
| --- | --- | --- | --- |
| Bass Web | Radial depth shockwaves bow an intersecting mesh | Bend the mesh sideways and vertically | Ripple and tear individual strands |
| Ribbon Reactor | A swelling wave travels through a wide braided cable | Alter braid pitch and bend its strands | Split strands into colored fragments |
| Shockwave Tunnel | Rectangular hoops kick outward in sequence | Hinge and twist the corridor | Kink hoop edges |
| Aurora Strings | Push a wide string curtain through depth | Bend fibers into large waves | Pluck individual strings |
| Prism Surge | Expand and displace nested polygon ribs in sequence | Twist and hinge different ribs | Fracture the polygon edges |
| Faultline | Lift rolling ridges through a wire terrain | Shear the ground and bend its ridges | Snap crests into sawtooth faults |

The renderer retains about one second of independent reactive lanes at 30 Hz. Sampling past values by position turns an onset into a traveling disturbance instead of resizing the whole scene at once. Sampling is interpolated and independent of presentation frame rate. The existing live band envelopes, music-reactivity control, and Chill/Balanced/Wild gain feed this history; silent input cannot invent beat pulses. Reactions occur continuously with the music, without waiting for a rare spectacle trigger.

The spatial motion clock integrates speed each frame, preventing energy changes from jumping the animation to a different time. The palette, brightness, and music-reactivity controls apply to all six worlds. Original screen-space modifiers are excluded from them except palette travel. Lines use alpha composition, so intersections retain saturated color instead of adding into white. Depth attenuation and perspective establish the 3D space; these are luminous wire structures rather than opaque solids.

Auto avoids the most recent composition class when the phrase's candidate group offers an alternative, as well as avoiding recently used scene IDs. Strong timing prefers impact or bar boundaries. Uncertain timing uses a transition of at least 2.4 seconds. Auto normally waits twelve seconds before crossing between 2D and 3D, and a mixed dissolve lasts at least 3.6/2.8/2.0 seconds in Chill/Balanced/Wild. Both completed scene images are blended in linear light, preserving the outgoing identity while audio stays live. Manual scene selection holds the world while leaving its band responses active.

All geometry uses bounded GPU instancing without per-frame mesh allocation, compute particles, external models, or hardware ray tracing. Single scenes can use the HD ceiling on integrated and discrete GPUs. Crossfades keep the current resolution and line width. Sustained late frames can step down to 540p, with a 20-second stable interval before recovery. The surface remains the physical display size. The synthetic native audition checks isolated bands with fixed camera, clock, palette, and brightness, plus opt-in flash immunity and full-black output. See [the current release notes](color-and-transitions-0.1.6.md) for reproduction and measurement limits.

## Tron collection

Select **Visual scene → Tron · all 12 visuals** for a continuously cycling neon set, or hold any of its twelve looks: Grid Highway, Light Trails, Laser Gates, Hex Corridor, Identity Discs, Circuit Board, Neon Arena, Solar Sails, Digital City, Helix Drive, Data Rain, and Reactor Iris. The existing 32-scene Auto library stays separate from this explicitly selected collection.

Tron uses a signature cyan, electric blue, and orange palette on near-black. Palette and color-travel choices apply to the other scene families; Tron keeps its signature colors. Brightness, motion, intensity, and music reactivity still apply. Bass expands the geometry, mids introduce a small camera-plane rotation plus scene-specific deformation, and highs add orange accents and energize circuit packets/data rain. There is no full-screen white flash, even with Flash enabled. Black output remains fully black.

The cycle uses the integrated motion clock, spending eight clock units per look with the last 20% reserved for a smooth dissolve. Actual dwell changes with motion, intensity, and audio drive. Selecting a named look holds its composition while animation and band reactions continue. This timed collection is distinct from the phrase-directed Auto library.

The browser ambient preview renders the same twelve procedural designs without injecting audio. The native renderer uses `src-tauri/shaders/tron.wgsl`; regenerate its WebGL counterpart with `python3 scripts/sync-tron-shader.py` after edits. Analytic distance fields use filtered edges and bounded loops, with no asset downloads or particle allocations. Tron bypasses legacy screen distortions and the instanced line pass.

Verification: `npm run build`, `npm run lint`, and `cargo test --manifest-path src-tauri/Cargo.toml --lib`. With a native GPU, run `cargo test --manifest-path src-tauri/Cargo.toml native_tron_audition -- --ignored --nocapture` to capture each design under `src-tauri/target/tron-audition` and check visibility, distinct images, isolated band responses, flash immunity, blackout, and cycling/held-look equivalence. Captures are synthetic, not a live Rekordbox test or a Windows performance benchmark.
