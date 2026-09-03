# Visual language

The performance display is treated as a room-scale light source, not a software interface. It always renders edge-to-edge color and motion with no labels, meters, transport controls, logos, or diagnostic overlays.

## Bases and modifiers

Auto is the only direction mode. Its 32-family library includes **Spinning Alien**, **Spinning Skull**, **Watching Eye**, **Chromatic Splotch Wave**, **Tumbling Cube**, **Techno Laser Grid**, **Moiré Rings**, **Infinite Checker**, **Neon Lattice**, **Twisted Stripes**, **Rotating Snakes**, **Hyperbolic Tunnel**, **Chromatic Maze**, **Vortex Chevron**, **Glass Orbit**, **Sine Interference**, **Impossible Cubes**, **Polar Fan**, **Gravity Lens**, **Ribbon Wormhole**, **Quantum Weave**, **Fractal Compass**, **Liquid Circuit**, **Prism Vortex**, **Diamond Drift**, **Orbital Mesh**, **Helix Portal**, **Radial Escalator**, **Electric Topography**, **Event Horizon**, **Kinetic Bars**, and **Bulging Checker**. The removed spiral and triangle/pyramid scene remain absent. The only time two families render is a short normalized incoming/outgoing crossfade.

Most shuffles draw from a curated set of large silhouettes, solid forms, and low-line-count illusions. Every sixth non-featured shuffle can draw from the full library, preserving variety without letting dense line fields dominate the show. Odd-numbered automatic shuffles rotate through Spinning Alien, Spinning Skull, Watching Eye, and Chromatic Splotch Wave; the first automatic shuffle is always the alien. Auto still waits for a beat, onset, or impact boundary, with a bounded quiet-music fallback, so the guarantee does not create an abrupt cut.

Spinning Alien and Spinning Skull are centered, filled subjects rather than line constructions. Each performs a continuous in-place 360-degree yaw: the silhouette narrows at profile, the facial features disappear across the back of the head, and surface shading carries the turn. Their scale breathes with rolling energy, bass, beat pulse, bass transients, and impact, then contracts as those signals release. The subject center never orbits or jitters.

Chromatic Splotch Wave restores the original fluid renderer's broad layered color fields as a deliberate full-screen animation. Large soft islands of different palette colors flow through one another, a slow horizontal wave bends the mass, and beat, bass, impact, and continuous energy drive a bounded whole-frame vertical shake. It uses its own stable coordinate path and does not receive the shared line, slice, fold, sparkle, chromatic-edge, impact-ring, or feedback overlays.

Independent modifiers provide V1-style palette travel, beat zoom, bass warp, high sparkle, feedback trails, mirror fold, chromatic edge split, and impact bloom. Each has attack/hold/release, compatibility rules, and a bounded strength. Zero or one is normal; peaks may briefly use two. Modifiers do not invoke an unrelated base shader, and the luminance cap tightens as modifier load rises.

Intro/breakdown/outro use low-density motion and rare Palette Drift/Feedback Trails. Groove rotates one modifier on a long interval. Builds increase speed/depth and favor Bass Warp or Chromatic Split. Drops may use two modifiers briefly and offer a purposeful base transition. A short impact adds Impact Bloom instead of replacing the base on every onset. Minimum dwell and bounded history prevent three consecutive selections of the same base.

## Color

Every palette contains four related colors: a dark foundation, two primary light colors, and an accent. Automatic color direction favors Ocean during Quiet, Flow, and Breakdown; Electric during Groove; Sunset during Build; and Neon during Impact and Peak. Fixed choices also include Purple + blue, Warm, Monochrome, and Rainbow flow.

Palette lookup is genuinely cyclic across four equal intervals: A→B→C→D→A. Angle-derived coordinates use whole periodic angular frequencies or cyclic palette coordinates, removing the negative-X `atan2` seam mathematically rather than masking it.

## Motion and safety

Chill, Balanced, and Wild set the ceiling of one proportional audio-drive dial. The dial combines rolling-normalized energy, bass, mids, highs, beat, onset, and impact, then uses a fast attack and slower release. Four short-lived reactive lanes sit on top of that continuous dial: sub/bass hits produce restrained radial movement and depth changes, midrange movement bends the main form, high-frequency transients add brief color accents, and whole-band energy rises increase scale and light. These accents are intentionally weaker than the base silhouette. Balanced and Wild retain distinct motion headroom while using lower detail, density, and brightness budgets than the earlier line-heavy direction. White flash remains an independent opt-in control at Off, Moderate, or High; it does not change the motion ceiling.

Each ScenePlan includes motion, detail, density, brightness, two normalized base weights, and two modifier slots. The proportional dial controls continuous travel, while frequency-specific transient lanes visibly alter shape, position, depth, density, and color on top of it. Beat and impact can also align scene and modifier changes with musical boundaries. A luminance budget is applied before tone mapping. The native renderer targets tear-free 60 FPS. Its presentation surface always matches the physical window, while high-resolution displays use a separate HD render target with linear upscaling; this avoids platform-dependent partial-frame behavior without quadrupling the expensive shader work. Screen-space detail filtering fades unresolved line frequencies before they can turn into blocky pixels.

## Frame feedback

The native renderer uses a pair of HD render targets as a bounded frame-history loop. Each new frame samples the previous target, applies a tiny zoom, rotation, liquid displacement, edge fade, hue drift, and chromatic offset, then mixes retained light beneath the current analytic scene. The targets swap after every successful present and are explicitly cleared on startup, resize, and surface recreation, so old pixels cannot leak between displays or sessions. Spinning Alien and Spinning Skull bypass that history loop as well as the shared coordinate warp, slice, fold, sparkle, chromatic, impact-ring, and frequency-line overlays; only their deliberate yaw, palette shading, music-driven scale, brightness response, and optional white flash remain.

The feedback is deliberately faint in normal scenes. Chill, Balanced, and Wild have progressively higher but bounded persistence, and the Feedback Trails modifier temporarily raises persistence and deformation without exceeding the luminance budget. Bass hits push the feedback outward, midrange motion bends it, and high-frequency hits separate its color channels. Recognizable hero scenes reject busy modifiers such as sparkle, feedback trails, mirror folding, and chromatic splitting so their silhouettes remain readable.

An original oscilloscope-inspired spectral signal ribbon is drawn faintly only during Watching Eye. It is not an imported MilkDrop preset and does not require Winamp, SHOUTcast, projectM, Butterchurn, an API key, or external visual assets. Alien and skull scenes intentionally receive no signal ribbon so their subjects stay readable.
