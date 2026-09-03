# Visual language

The performance display is an abstract music-reactive canvas, not a software interface or concert-light simulator. It renders edge-to-edge waves, ribbons, fields, grids, and restrained fractals with no labels, meters, transport controls, logos, mascots, or literal icons.

## Preset library

Auto uses 12 intentionally different structures:

1. **Color-Splotch Wave** — one continuous organic wave with a soft body, optional close echoes, small embedded color accents, and bounded whole-frame vertical movement.
2. **Multi-Layer Wave Field** — three to eight evenly spaced waves that breathe together while preserving the identity of each line.
3. **Fractal Bloom** — two to five nested petal levels with a strong silhouette and open center.
4. **Recursive Tunnel** — seven nested circle/diamond frames traveling through depth.
5. **Ribbon Flow** — a broad two-edge ribbon with a quiet fill and one optional secondary trace.
6. **Branching Tree** — a symmetric trunk-and-branch system with a pulse traveling upward.
7. **Contour Field** — smooth topographic contours shaped by a deterministic field and a bass ripple.
8. **Lattice Flow** — an evenly spaced grid whose intersections and curvature respond without breaking continuity.
9. **Helix Spiral** — two depth-weighted strands with restrained connecting rungs.
10. **Ring Pulse System** — a small hierarchy of breathing rings plus a kick-driven emitted ring.
11. **Arc Fan** — curved rays opening from a single lower origin.
12. **Fractal Wave Hybrid** — a readable base wave followed by progressively smaller recursive harmonics.

Color-Splotch Wave, Multi-Layer Wave Field, Fractal Bloom, and Recursive Tunnel form the recurring core rotation. Odd-numbered automatic shuffles cycle through those four in that order; other shuffles choose from the complete library while avoiding recent repetition. Scene changes wait for a beat, onset, impact, or bounded quiet-music fallback, and the only simultaneous presets are the outgoing and incoming structures during a normalized crossfade.

The controller's ambient preview rotates through the four core presets. Production output contains all 12 and uses live audio.

## Musical meaning

Reactivity belongs to the structure rather than a shared layer of unrelated effects:

- Bass controls primary amplitude, width, scale, depth, and opening.
- Mids control curvature, secondary branches, field displacement, and internal detail.
- Highs brighten small embedded accents or fine structure; they do not generate random rays or sparkle noise.
- Kicks emit or travel through rings and lines, briefly widen forms, or add a bounded brightness accent.
- Energy rises open structures and expose additional levels.
- Intro, breakdown, and outro budgets reduce density, brightness, and motion to preserve black space.
- Build and peak budgets increase visible hierarchy and motion without regenerating geometry on every hit.

The existing `MusicState` and phrase system act as the high-level energy model. Their hysteresis and asymmetric smoothing let a visual evolve over phrases instead of twitching frame by frame.

## Color and density

Every palette contains a dark foundation, two related line colors, and an accent. Most geometry moves only between the two related colors. The accent is reserved for embedded splotches, traveling pulses, intersections, or a small central highlight. Palette Drift moves slowly; it does not assign a random color to every line.

Density is capped per structure: wave layers top out at eight, fractal bloom at five levels, and the recursive tunnel at seven frames. Sparse energy states show fewer levels and leave more of the black canvas untouched. Screen-space derivative filtering keeps thin lines from turning into blocky noise at output resolution.

## Motion and transitions

Motion is deterministic and continuous: phase travel, wave propagation, contour breathing, grid bending, recursive unfolding, slow rotation, and path pulses. The shader no longer applies global slice jitter, random line placement, mirror folding, chromatic shards, or radial ray fields over every preset.

The supported accents are Palette Drift, Beat Zoom, Feedback Trails, and Impact Bloom. Palette, zoom, and impact are safe for every structure. Trails are limited to Recursive Tunnel, Ribbon Flow, Contour Field, and Helix Spiral, where persistence reinforces the underlying path. Each accent has bounded attack, hold, and release values, and the luminance cap tightens as modifier load rises.

The native renderer retains its two-target frame-history loop, but normal persistence is deliberately faint. Feedback becomes noticeable only when the compatible Trails accent is active. Render targets are still cleared on startup, resize, and surface recreation.

White flash remains an independent opt-in control at Off, Moderate, or High. It never changes the motion ceiling.

## Readability rule

A frozen frame must be describable as a wave, ribbon, bloom, tree, tunnel, contour field, lattice, helix, ring system, or arc fan. If it reads as “just a bunch of lines,” the preset has failed its design goal even if it is technically reactive.
