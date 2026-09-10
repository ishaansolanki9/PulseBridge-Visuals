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
fn rotate_line(p: vec2<f32>, a: f32) -> vec2<f32> {
    return vec2<f32>(cos(a) * p.x - sin(a) * p.y, sin(a) * p.x + cos(a) * p.y);
}
fn strand_color(strand: f32, u: f32, high: f32) -> vec3<f32> {
    let drift = params.spatial.x * 0.17 * params.effects.z * (1.0 + modifier_strength(0u) * 0.5);
    let sweep = 0.5 + 0.5 * sin(strand * 0.16 + u * 3.1 + drift);
    let base = mix(params.color_b.rgb, params.color_c.rgb, sweep);
    return mix(base, params.color_d.rgb, clamp(high * 0.72 + 0.12 * sin(strand), 0.0, 0.86));
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
        let signal = recent_signal(length(grid) * 0.095);
        let bend = signal.y * sin(grid.x * 0.95 + grid.y * 0.38 + clock * 0.4);
        let tear = signal.z * sin(grid.x * 3.4 + strand * 0.71);
        return vec3<f32>(grid.x + bend * 0.62, grid.y + bend * 0.88 + tear * 0.22,
            6.5 - signal.x * 2.65 + sin(length(grid) * 1.3 - clock * 0.3) * signal.x * 0.7 + tear * 0.45);
    }
    if family == 27u {
        // Ribbon Reactor: a wide braided cable. Every onset travels end-to-end.
        let signal = recent_signal(u * 0.80);
        let a = strand / 48.0 * TAU + x * (0.58 + signal.y * 0.72);
        let envelope = 0.9 + signal.x * (1.6 + 0.45 * sin(x * 1.7));
        let split = signal.z * sin(strand * 2.1) * (0.5 + u);
        return vec3<f32>(x + signal.y * sin(a) * 0.7,
            sin(a + clock * 0.3) * envelope + split * 0.8,
            6.2 + cos(a + clock * 0.3) * (1.15 + signal.x * 0.75) - split * 1.3);
    }
    if family == 28u {
        // Shockwave Tunnel: sharp rectangular hoops fold and kick in sequence.
        let signal = recent_signal(strand / 48.0 * 0.9);
        let a = u * TAU;
        let direction = vec2<f32>(cos(a), sin(a));
        let rectangular = direction / max(abs(direction.x), abs(direction.y));
        let kink = sin(a * 8.0 + strand * 0.5) * signal.z * 0.34;
        var xy = rectangular * vec2<f32>(3.5, 2.5) * (1.0 + signal.x * 0.48 + kink);
        xy = rotate_line(xy, signal.y * sin(strand * 0.24) * 0.65);
        xy += vec2<f32>(sin(strand * 0.25 + clock * 0.15), cos(strand * 0.19)) * signal.y * 0.75;
        return vec3<f32>(xy, 2.6 + strand * 0.63 - signal.x * 0.8);
    }
    if family == 29u {
        // Aurora Strings: high notes pluck individual fibers in a flowing curtain.
        let signal = recent_signal(u * 0.65 + strand / 48.0 * 0.18);
        let pluck = signal.z * sin(x * 5.5 + strand * 0.85) * exp(-abs(x) * 0.08);
        let sweep = sin(x * 0.68 + strand * 0.09 + clock * 0.25);
        return vec3<f32>(x, row * 2.4 + sweep * (0.2 + signal.y * 1.45) + pluck * 0.65,
            6.0 + strand / 48.0 * 2.2 - signal.x * 1.85 + cos(x * 0.5) * signal.y * 0.8);
    }
    if family == 30u {
        // Prism Surge: polygonal ribs fracture, twist, then rejoin after each hit.
        let signal = recent_signal(strand / 48.0 * 0.82);
        let a = u * TAU;
        let sector = ((a + PI / 8.0) % (TAU / 8.0)) - PI / 8.0;
        let polygon = 1.0 / cos(sector);
        let radius = 0.36 + strand * 0.075 + signal.x * 0.8;
        let hinge = signal.y * sin(a * 4.0 + strand * 0.21) * 0.5;
        let shatter = signal.z * sin(a * 12.0 + strand * 0.8) * 0.48;
        let xy = rotate_line(vec2<f32>(cos(a), sin(a)) * (radius * polygon + shatter), hinge + clock * 0.08);
        return vec3<f32>(xy * vec2<f32>(1.5, 1.0), 6.0 + strand * 0.018 - signal.x * 1.4 + signal.y * cos(a * 3.0) * 0.9);
    }
    // Faultline: kicks lift ridges through an extensive wire terrain; high hits
    // snap the crests into sharp sawtooth faults, while mids shear them sideways.
    var ground = vec2<f32>(x * 1.5, 2.0 + strand * 0.50);
    if strand >= 32.0 { ground = vec2<f32>((strand - 32.0) / 15.0 * 20.4 - 10.2, 2.0 + u * 23.5); }
    let signal = recent_signal((ground.y - 2.0) * 0.032);
    let fault = abs(fract(ground.x * 0.38 + ground.y * 0.12) - 0.5) * 2.0;
    let wave = sin(ground.x * 0.65 + ground.y * 0.5 - clock * 0.25);
    return vec3<f32>(ground.x + signal.y * sin(ground.y * 0.7) * 1.2,
        -1.8 + wave * (0.15 + signal.y * 0.65) + signal.x * (1.2 + 0.7 * cos(ground.x * 0.5)) + signal.z * fault * 0.55,
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
    let signal = recent_signal(u0 * 0.65);
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
    result.color = strand_color(strand, u0, signal.z);
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
