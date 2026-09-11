// Reactive line worlds. Instanced ribbons provide continuous 3D geometry with
// constant pixel widths. These are emissive strands, not illuminated solids.
const LINE_SEGMENTS: u32 = 96u;
const LINE_STRANDS: u32 = 48u;
const LINE_LAYER_STRIDE: u32 = 8192u;

fn spatial_scene(_id: u32, uv: vec2<f32>) -> vec3<f32> {
    // The fullscreen pass only provides a quiet foundation; ribbons draw after it.
    return params.color_a.rgb * 0.014 * (1.0 - smoothstep(0.0, 2.0, length(uv)));
}
fn recent_signal(seconds: f32) -> vec4<f32> {
    let age = clamp(seconds, 0.0, 0.98);
    let fraction = max(params.spatial.w, 0.00001);
    if age < fraction { return mix(params.signal_history[0], params.signal_history[1], age / fraction); }
    let index = clamp(1.0 + (age - fraction) * 30.0, 1.0, 30.99);
    let lower = u32(floor(index));
    return mix(params.signal_history[lower], params.signal_history[lower + 1u], fract(index));
}
fn line_signal(age: f32) -> vec4<f32> {
    return clamp(recent_signal(age) * 1.25 + params.reactive * 0.18, vec4<f32>(0.0), vec4<f32>(1.0));
}
fn rotate_line(p: vec2<f32>, a: f32) -> vec2<f32> {
    return vec2<f32>(cos(a) * p.x - sin(a) * p.y, sin(a) * p.x + cos(a) * p.y);
}
fn line_hsv(color: vec3<f32>) -> vec3<f32> {
    let brightest = max(color.r, max(color.g, color.b));
    let darkest = min(color.r, min(color.g, color.b));
    let chroma = brightest - darkest;
    var hue = 0.0;
    if chroma > 0.00001 {
        if brightest == color.r { hue = (color.g - color.b) / chroma; }
        else if brightest == color.g { hue = 2.0 + (color.b - color.r) / chroma; }
        else { hue = 4.0 + (color.r - color.g) / chroma; }
    }
    return vec3<f32>(fract(hue / 6.0 + 1.0), chroma / max(brightest, 0.00001), brightest);
}
fn line_rgb(hsv: vec3<f32>) -> vec3<f32> {
    let wheel = abs(fract(vec3<f32>(hsv.x) + vec3<f32>(0.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - 3.0);
    return hsv.z * mix(vec3<f32>(1.0), clamp(wheel - 1.0, vec3<f32>(0.0), vec3<f32>(1.0)), hsv.y);
}
fn line_palette(coordinate: f32) -> vec3<f32> {
    let position = fract(coordinate) * 3.0;
    let segment = u32(position);
    var start_color = params.color_b.rgb;
    var end_color = params.color_c.rgb;
    if segment == 1u { start_color = params.color_c.rgb; end_color = params.color_d.rgb; }
    if segment == 2u { start_color = params.color_d.rgb; end_color = params.color_b.rgb; }
    let a = line_hsv(start_color);
    let b = line_hsv(end_color);
    let t = smoothstep(0.0, 1.0, fract(position));
    // Interpolate around the hue wheel: complementary palette colors stay
    // colored as they travel, instead of mixing into a gray/white midpoint.
    let hue_delta = fract(b.x - a.x + 0.5) - 0.5;
    return line_rgb(vec3<f32>(fract(a.x + hue_delta * t + 1.0), mix(a.yz, b.yz, t)));
}
fn strand_color(strand: f32, u: f32, signal: vec4<f32>) -> vec3<f32> {
    // Color has its own continuous clock, independent of geometry rotation.
    // Past onsets move colored bands along the same strands they deform.
    let response = (signal.x * 0.16 + signal.y * 0.22 + signal.z * 0.11) * params.chromatic.y;
    let travel = params.chromatic.x + modifier_strength(0u) * 0.07;
    return line_palette(travel + strand / 48.0 * 0.32 + u * 0.48 + response);
}

fn strand_point(family: u32, strand: f32, u: f32) -> vec3<f32> {
    let clock = params.spatial.x;
    let x = (u * 2.0 - 1.0) * 6.8;
    let row = (strand / 47.0 * 2.0 - 1.0);
    if family == 26u {
        // Bass Web: two intersecting sets of strings, deformed in world space.
        var grid = vec2<f32>(x, row * 3.6);
        if strand >= 32.0 { grid = vec2<f32>((strand - 32.0) / 15.0 * 13.6 - 6.8, (u * 2.0 - 1.0) * 3.6); }
        else { grid.y = (strand / 31.0 * 2.0 - 1.0) * 3.6; }
        let signal = line_signal(length(grid) * 0.095);
        let bend = signal.y * sin(grid.x * 0.95 + grid.y * 0.38 + clock * 0.4);
        let tear = signal.z * sin(grid.x * 3.4 + strand * 0.71);
        return vec3<f32>(grid.x + bend * 0.95, grid.y + bend * 1.25 + tear * 0.22,
            6.5 - signal.x * 2.65 + sin(length(grid) * 1.3 - clock * 0.3) * signal.x * 0.7 + tear * 0.45);
    }
    if family == 27u {
        // Ribbon Reactor: a wide braided cable. Every onset travels end-to-end.
        let signal = line_signal(u * 0.80);
        let a = strand / 48.0 * TAU + x * (0.58 + signal.y * 0.72);
        let envelope = 0.9 + signal.x * (1.6 + 0.45 * sin(x * 1.7));
        let split = signal.z * sin(strand * 2.1) * (0.5 + u);
        return vec3<f32>(x + signal.y * sin(a) * 0.7,
            sin(a + clock * 0.3) * envelope + split * 0.8,
            6.2 + cos(a + clock * 0.3) * (1.15 + signal.x * 0.75) - split * 1.3);
    }
    if family == 28u {
        // Shockwave Tunnel: sharp rectangular hoops fold and kick in sequence.
        let signal = line_signal(strand / 48.0 * 0.9);
        let a = u * TAU;
        let direction = vec2<f32>(cos(a), sin(a));
        let rectangular = direction / max(abs(direction.x), abs(direction.y));
        let kink = sin(a * 8.0 + strand * 0.5) * signal.z * 0.34;
        var xy = rectangular * vec2<f32>(3.5, 2.5) * (1.0 + signal.x * 0.48 + kink);
        xy = rotate_line(xy, signal.y * sin(strand * 0.24) * 1.0);
        xy += vec2<f32>(sin(strand * 0.25 + clock * 0.15), cos(strand * 0.19)) * signal.y * 1.2;
        return vec3<f32>(xy, 2.6 + strand * 0.63 - signal.x * 0.8);
    }
    if family == 29u {
        // Aurora Strings: high notes pluck individual fibers in a flowing curtain.
        let signal = line_signal(u * 0.65 + strand / 48.0 * 0.18);
        let pluck = signal.z * sin(x * 5.5 + strand * 0.85) * exp(-abs(x) * 0.08);
        let sweep = sin(x * 0.68 + strand * 0.09 + clock * 0.25);
        return vec3<f32>(x, row * 2.4 + sweep * (0.2 + signal.y * 1.45) + pluck * 1.05,
            6.0 + strand / 48.0 * 2.2 - signal.x * 1.85 + cos(x * 0.5) * signal.y * 0.8);
    }
    if family == 30u {
        // Prism Surge: polygonal ribs fracture, twist, then rejoin after each hit.
        let signal = line_signal(strand / 48.0 * 0.82);
        let a = u * TAU;
        let sector = ((a + PI / 8.0) % (TAU / 8.0)) - PI / 8.0;
        let polygon = 1.0 / cos(sector);
        let radius = 0.36 + strand * 0.075 + signal.x * 0.8;
        let hinge = signal.y * sin(a * 4.0 + strand * 0.21) * 0.5;
        let shatter = signal.z * sin(a * 12.0 + strand * 0.8) * 0.48;
        let xy = rotate_line(vec2<f32>(cos(a), sin(a)) * (radius * polygon + shatter), hinge + clock * 0.08);
        return vec3<f32>(xy * vec2<f32>(1.5, 1.0), 6.0 + strand * 0.018 - signal.x * 1.4 + signal.y * cos(a * 3.0) * 1.5);
    }
    if family == 49u {
        // Orbit Foundry: twelve gimbals, each split into four luminous rails.
        let ring = floor(strand / 4.0);
        let rail = strand % 4.0;
        let signal = line_signal(ring * 0.065 + u * 0.18);
        let a = u * TAU;
        let radius = 1.4 + rail * 0.07 + signal.x * sin(a * 3.0 + ring) * 0.5;
        var xy = vec2<f32>(cos(a), sin(a)) * radius;
        xy += vec2<f32>(sin(ring * 1.7), cos(ring * 2.1)) * signal.x * 0.8;
        xy = rotate_line(xy, ring * 0.4 + clock * 0.16 + signal.y * 1.1);
        let tilt = ring * 0.6 + signal.y * 1.3;
        return vec3<f32>(xy.x, xy.y * cos(tilt) + signal.z * sin(a * 8.0) * 0.4,
            5.8 + xy.y * sin(tilt) + (ring - 5.5) * 0.2 - signal.z * cos(a * 5.0) * 0.5);
    }
    if family == 50u {
        // Synapse Bloom: tendrils unfurl and send plucks out from a living core.
        let signal = line_signal(u * 0.8 + strand / 48.0 * 0.12);
        let azimuth = strand * 2.39996;
        let latitude = acos(1.0 - (strand + 0.5) / 24.0);
        let radius = 0.3 + u * (2.0 + signal.x * 1.2);
        let curl = sin(u * 6.0 + strand * 0.8 + clock * 0.3) * signal.y;
        let a = azimuth + curl * 0.8 + clock * 0.12;
        let b = latitude + cos(u * 5.0 + strand) * signal.y * 0.55;
        let snap = sin(u * 26.0 + strand) * signal.z * u * 0.3;
        return vec3<f32>(cos(a) * sin(b) * radius + snap,
            sin(a) * sin(b) * radius + curl * u * 0.35,
            5.6 + cos(b) * radius + snap * 0.65);
    }
    if family == 51u {
        // Gravity Braids: four cables weave through depth, bend and peel apart.
        let cable = floor(strand / 12.0);
        let fiber = strand % 12.0;
        let signal = line_signal(u * 0.78 + cable * 0.05);
        let a = fiber / 12.0 * TAU + x * (0.8 + signal.y * 0.8) + clock * 0.3;
        let reach = 0.22 + signal.x * (0.35 + 0.16 * sin(x * 2.0));
        let braid = x * 0.55 + cable * PI * 0.5;
        let peel = signal.z * sin(fiber * 2.1) * 0.65;
        return vec3<f32>(x + signal.y * sin(a) * 0.35,
            sin(braid + signal.y * 0.8) * (1.1 + signal.x * 0.6) + sin(a) * reach + peel,
            6.5 + cos(braid) * 1.7 + cos(a) * reach - peel * 0.8);
    }
    if family == 52u {
        // Prism Conveyor: cuboid cages tumble, hinge and separate in perspective.
        let cage = floor(strand / 4.0);
        let rib = strand % 4.0;
        let signal = line_signal(cage * 0.065 + u * 0.2);
        let a = u * TAU;
        let direction = vec2<f32>(cos(a), sin(a));
        var xy = direction / max(abs(direction.x), abs(direction.y));
        xy *= 0.65 + signal.x * sin(a * 2.0 + cage) * 0.25;
        xy = rotate_line(xy, cage * 0.38 + clock * 0.18 + signal.y * sin(cage) * 1.0);
        let slide = (rib - 1.5) * (0.18 + signal.z * 0.35);
        let center = vec2<f32>(sin(cage * 2.4 + clock * 0.1), cos(cage * 2.4)) * 1.8;
        return vec3<f32>(xy + center + vec2<f32>(slide, signal.x * sin(cage) * 0.7),
            3.5 + cage * 0.6 + xy.y * sin(signal.y * 1.4) + slide);
    }
    // Faultline: kicks lift ridges through an extensive wire terrain; high hits
    // snap the crests into sharp sawtooth faults, while mids shear them sideways.
    var ground = vec2<f32>(x * 1.5, 2.0 + strand * 0.50);
    if strand >= 32.0 { ground = vec2<f32>((strand - 32.0) / 15.0 * 20.4 - 10.2, 2.0 + u * 23.5); }
    let signal = line_signal((ground.y - 2.0) * 0.032);
    let fault = abs(fract(ground.x * 0.38 + ground.y * 0.12) - 0.5) * 2.0;
    let wave = sin(ground.x * 0.65 + ground.y * 0.5 - clock * 0.25);
    return vec3<f32>(ground.x + signal.y * sin(ground.y * 0.7) * 1.2,
        -1.8 + wave * (0.15 + signal.y * 0.65) + signal.x * (1.2 + 0.7 * cos(ground.x * 0.5)) + signal.z * fault * 1.05,
        ground.y);
}
struct LineVertex {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) edge: f32,
    @location(2) opacity: f32,
};
@vertex
fn vs_lines(@builtin(vertex_index) vertex: u32, @builtin(instance_index) instance: u32) -> LineVertex {
    let incoming = instance >= LINE_LAYER_STRIDE;
    let local = instance % LINE_LAYER_STRIDE;
    let family = u32(select(params.style_a.x, params.style_a.y, incoming));
    let weight = select(params.style_a.z, params.style_a.w, incoming);
    let strand = f32(local / LINE_SEGMENTS);
    let segment = local % LINE_SEGMENTS;
    let u0 = f32(segment) / f32(LINE_SEGMENTS);
    let u1 = f32(segment + 1u) / f32(LINE_SEGMENTS);
    let p0 = strand_point(family, strand, u0);
    let p1 = strand_point(family, strand, u1);
    let resolution = max(params.resolution_time.xy, vec2<f32>(1.0));
    let aspect = resolution.x / resolution.y;
    let a = p0.xy * 1.8 / max(p0.z, 0.8) / vec2<f32>(aspect, 1.0);
    let b = p1.xy * 1.8 / max(p1.z, 0.8) / vec2<f32>(aspect, 1.0);
    let screen_delta = (b - a) * resolution;
    let tangent = screen_delta / max(length(screen_delta), 0.001);
    let normal = vec2<f32>(-tangent.y, tangent.x);
    let end = vertex == 1u || vertex == 2u || vertex == 4u;
    let side = select(-1.0, 1.0, vertex == 2u || vertex == 4u || vertex == 5u);
    let signal = line_signal(u0 * 0.65);
    let width = 3.0 + params.spatial.z * 0.65;
    let position = select(a, b, end) + (normal * side * width + tangent * select(-0.75, 0.75, end)) * 2.0 / resolution;
    var opacity = weight;
    opacity *= exp(-max(p0.z - 5.0, 0.0) * 0.055);
    if p0.z < 0.8 || p1.z < 0.8 || params.style_b.w > 0.5 { opacity = 0.0; }
    // High transients break selected strands into fast, colored fragments.
    let break_phase = fract(u0 * 9.0 - params.resolution_time.z * 1.2 + strand * 0.13);
    if signal.z > 0.25 && break_phase < signal.z * 0.18 && family != 26u { opacity *= 0.12; }
    var result: LineVertex;
    result.position = vec4<f32>(position, 0.0, 1.0);
    result.color = strand_color(strand, u0, signal);
    result.edge = side;
    result.opacity = opacity;
    return result;
}
@fragment
fn fs_lines(input: LineVertex) -> @location(0) vec4<f32> {
    let core = exp(-input.edge * input.edge * 32.0);
    let halo = exp(-input.edge * input.edge * 4.6) * 0.23;
    // Alpha composition stays a convex combination of saturated colors. No
    // additive white-out, full-screen white flash, or solid specular highlights.
    let color = 1.0 - exp(-input.color * max(params.visual.w, 0.0) * 3.5);
    return vec4<f32>(color, clamp((core * 0.93 + halo) * input.opacity, 0.0, 0.95));
}
