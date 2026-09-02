# Visual language

The performance display is treated as a room-scale light source, not a software interface. It always renders edge-to-edge color and motion with no labels, meters, transport controls, logos, or diagnostic overlays.

## Bases and modifiers

Auto is the only direction mode. It shuffles through 26 motion-first analytic illusions: **Warp Spiral**, **Moiré Rings**, **Infinite Checker**, **Neon Lattice**, **Twisted Stripes**, **Rotating Snakes**, **Hyperbolic Tunnel**, **Chromatic Maze**, **Vortex Chevron**, **Glass Orbit**, **Sine Interference**, **Impossible Cubes**, **Polar Fan**, **Gravity Lens**, **Ribbon Wormhole**, **Quantum Weave**, **Fractal Compass**, **Liquid Circuit**, **Alien Heads**, **Prism Vortex**, **Diamond Drift**, **Orbital Mesh**, **Helix Portal**, **Radial Escalator**, **Electric Topography**, and **Event Horizon**. The only time two families render is a short normalized incoming/outgoing crossfade. Timed reshuffling and bounded history prevent a short predictable loop.

Independent modifiers provide V1-style palette travel, beat zoom, bass warp, high sparkle, echo trails, mirror fold, chromatic edge split, and impact bloom. Each has attack/hold/release, compatibility rules, and a bounded strength. Zero or one is normal; peaks may briefly use two. Modifiers do not invoke an unrelated base shader, and the luminance cap tightens as modifier load rises.

Intro/breakdown/outro use low-density motion and rare Palette Drift/Echo Trails. Groove rotates one modifier on a long interval. Builds increase speed/depth and favor Bass Warp or Chromatic Split. Drops may use two modifiers briefly and offer a purposeful base transition. A short impact adds Impact Bloom instead of replacing the base on every onset. Minimum dwell and bounded history prevent three consecutive selections of the same base.

## Color

Every palette contains four related colors: a dark foundation, two primary light colors, and an accent. Automatic color direction favors Ocean during Quiet, Flow, and Breakdown; Electric during Groove; Sunset during Build; and Neon during Impact and Peak. Fixed choices also include Purple + blue, Warm, Monochrome, and Rainbow flow.

Palette lookup is genuinely cyclic across four equal intervals: A→B→C→D→A. Angle-derived coordinates use whole periodic angular frequencies or cyclic palette coordinates, removing the negative-X `atan2` seam mathematically rather than masking it.

## Motion and safety

Chill, Balanced, and Wild set the ceiling of one proportional audio-drive dial. The dial combines rolling-normalized energy, bass, mids, highs, beat, onset, and impact, then uses a fast attack and slower release. Wild permits the full 0–100% range but does not force constant maximum movement. White flash remains an independent opt-in control at Off, Moderate, or High; it no longer changes the motion ceiling.

Each ScenePlan includes motion, detail, density, brightness, two normalized base weights, and two modifier slots. The proportional dial primarily controls travel speed, domain deformation, depth, and palette velocity; beat pulse adds a smaller camera impulse, while impact opens transitions and brief modifiers. A luminance budget is applied before tone mapping. The native renderer targets 45 FPS and caps high-resolution fragment shading near 2560×1440 before compositor scaling so a 4K projector does not quadruple the expensive per-pixel work.
