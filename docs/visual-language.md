# Visual language

The performance display is treated as a room-scale light source, not a software interface. It always renders edge-to-edge color and motion with no labels, meters, transport controls, logos, or diagnostic overlays.

## Bases and modifiers

Auto directs 32 scenes. Its original 26 motion-first analytic illusions remain available: **Warp Spiral**, **Moiré Rings**, **Infinite Checker**, **Neon Lattice**, **Twisted Stripes**, **Rotating Snakes**, **Hyperbolic Tunnel**, **Chromatic Maze**, **Vortex Chevron**, **Glass Orbit**, **Sine Interference**, **Impossible Cubes**, **Polar Fan**, **Gravity Lens**, **Ribbon Wormhole**, **Quantum Weave**, **Fractal Compass**, **Liquid Circuit**, **Alien Heads**, **Prism Vortex**, **Diamond Drift**, **Orbital Mesh**, **Helix Portal**, **Radial Escalator**, **Electric Topography**, and **Event Horizon**. The director groups them by musical role: atmospheric scenes for intros/outros/breakdowns, flowing or geometric scenes for verses, tunnels/escalation for builds, high-impact scenes for choruses/fills, and textural scenes for bridges. The live structure signature and phrase index choose deterministically inside those groups; there is no cadence-based random scene change. The controller can also hold any of the six new spatial scenes, retaining audio-driven detail and lighting. The only time two families render is a short normalized incoming/outgoing crossfade, and transitions prefer impact or four-beat boundaries.

Independent modifiers provide V1-style palette travel, beat zoom, bass warp, high sparkle, echo trails, mirror fold, chromatic edge split, and impact bloom. Each has attack/hold/release, compatibility rules, and a bounded strength. Zero or one is normal; peaks may briefly use two. Modifiers do not invoke an unrelated base shader, and the luminance cap tightens as modifier load rises.

Intro/breakdown/outro use low-density motion and rare Palette Drift/Echo Trails. Verse/groove phrases favor Beat Zoom, Palette Drift, or High Sparkle when a musical trigger arrives. Builds increase speed/depth and favor Bass Warp or Chromatic Split. Drops may use two modifiers briefly and offer a purposeful base transition. A short impact adds Impact Bloom instead of replacing the base on every onset. Minimum dwell and bounded history prevent three consecutive selections of the same base.

## Color

Every palette contains four related colors: a dark foundation, two primary light colors, and an accent. Automatic color direction favors Ocean during Quiet, Flow, and Breakdown; Electric during Groove; Sunset during Build; and Neon during Impact and Peak. Fixed choices also include Purple + blue, Warm, Monochrome, and Rainbow flow.

Palette lookup is genuinely cyclic across four equal intervals: A→B→C→D→A. Angle-derived coordinates use whole periodic angular frequencies or cyclic palette coordinates, removing the negative-X `atan2` seam mathematically rather than masking it.

## Motion and safety

Chill, Balanced, and Wild set the ceiling of one proportional audio-drive dial. The dial combines rolling-normalized energy, bass, mids, highs, beat, onset, and impact, then uses a fast attack and slower release. Four short-lived reactive lanes sit on top of that continuous dial: sub/bass hits produce radial geometry waves and depth jumps, midrange movement bends and ribs the geometry, high-frequency transients slice the image and emit colored shards, and whole-band energy rises increase scale, density, and light. Those lanes use independent attack/release envelopes, so visible motion follows instrumentation rather than merely speeding up a prerecorded-looking animation. Wild permits the full 0–100% range but does not force constant maximum movement. White flash remains an independent opt-in control at Off, Moderate, or High; it no longer changes the motion ceiling.

Each ScenePlan includes motion, detail, density, brightness, two normalized base weights, and two modifier slots. The proportional dial controls continuous travel, while frequency-specific transient lanes visibly alter shape, position, depth, density, and color on top of it. Beat and impact can also align scene and modifier changes with musical boundaries. The spatial library uses world-space reactions and bypasses the original screen-space warps, ribs, shards, and overlays to preserve perspective and silhouette. A luminance budget is applied before tone mapping. The native renderer targets tear-free 60 FPS. Its presentation surface always matches the physical window, while high-resolution displays use a separate HD render target with linear upscaling; this avoids platform-dependent partial-frame behavior without quadrupling the expensive shader work. Screen-space detail filtering fades unresolved line frequencies before they can turn into blocky pixels.

## Spatial families (0.1.3)

- **Magnetic Swarm**: a procedural 3D field of beads on a braided torus. Bass sends a wave around the structure, highs brighten its points, and a confirmed event expands the structure before it gathers again. This is an analytic particle arrangement, not a compute-based physical simulation.
- **Liquid Relic**: a lit, reflective signed-distance sculpture morphs between round and angular surfaces. Mids twist it, bass adds surface ripples, and six satellites detach and merge during rare transformations.
- **Impossible Architecture**: a moving camera reveals repeated monumental frames and floating slabs. Lateral parallax, depth fog, and occlusion establish scale; an event widens the space.
- **Aurora Veil**: six layered luminous curtains and a sparse star field create breathing room. This family uses inexpensive layered depth rather than raymarched solids.
- **Kinetic Sculpture**: nine rounded bars form an evolving mechanical helix, with distinct midrange rotation and bass deformation.
- **Topographic Ocean**: a world-space height field with a traveling camera, shaded ridges, filtered contours, and atmospheric depth.

The spatial motion clock integrates speed each frame, preventing energy changes from jumping the animation to a different time. Camera motion is restrained. Material colors travel slowly and the existing palette/brightness controls still apply. Original screen-space modifiers are excluded from spatial families except palette travel.

Auto avoids the most recent composition class when the phrase's candidate group offers an alternative, as well as avoiding recently used scene IDs. Strong timing prefers impact or bar boundaries. Uncertain timing uses a transition of at least 2.4 seconds. Transformations require live reactivity, impact above 0.72, energy above 0.55, and beat confidence at least 0.5. They expand over 0.85 seconds, hold briefly, and recover over three seconds. A 24-second cooldown and low-impact rearm prevent repeated triggering. Chill suppresses the dramatic transformation; Balanced bounds it to 72% and Wild permits the full envelope.

Spatial distances use fixed loop bounds and conservative stepping. Quality tiers reduce ray steps and internal resolution together. Crossfades between two spatial scenes use 540p internally; other transitions cap at 720p. Integrated GPUs cap these scenes at 720p internally; sustained late frames can step down to 540p, with a 20-second stable interval before recovery. The surface remains the physical display size. No hardware ray tracing, external models, paid services, or recorded audio are required.
