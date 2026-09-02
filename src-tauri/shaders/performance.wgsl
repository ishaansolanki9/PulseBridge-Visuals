struct VisualParams {
    resolution_time: vec4<f32>,
    music: vec4<f32>,
    pulse: vec4<f32>,
    visual: vec4<f32>,
    color_a: vec4<f32>,
    color_b: vec4<f32>,
    color_c: vec4<f32>,
    color_d: vec4<f32>,
    style_a: vec4<f32>,
    style_b: vec4<f32>,
    effects: vec4<f32>,
    scene: vec4<f32>,
    modifiers: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: VisualParams;

const PI: f32 = 3.14159265359;
const TAU: f32 = 6.28318530718;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    return vec4<f32>(positions[index], 0.0, 1.0);
}

fn hash21(point: vec2<f32>) -> f32 {
    let p = fract(point * vec2<f32>(123.34, 456.21));
    let q = p + dot(p, p + 45.32);
    return fract(q.x * q.y);
}

fn rotate2(point: vec2<f32>, angle: f32) -> vec2<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return mat2x2<f32>(c, -s, s, c) * point;
}

fn modifier_strength(kind: u32) -> f32 {
    var strength = 0.0;
    if params.modifiers.x >= 0.0 && u32(round(params.modifiers.x)) == kind {
        strength = max(strength, params.modifiers.y);
    }
    if params.modifiers.z >= 0.0 && u32(round(params.modifiers.z)) == kind {
        strength = max(strength, params.modifiers.w);
    }
    return clamp(strength, 0.0, 1.0);
}

fn palette_field(value: f32) -> vec3<f32> {
    let drive = params.style_b.y;
    let palette_drift = modifier_strength(0u);
    let speed = (0.02 + drive * 0.34 + palette_drift * 0.12) * params.effects.z;
    let scaled = fract(value + params.resolution_time.z * speed + params.pulse.x * drive * 0.04) * 4.0;
    let local = smoothstep(0.0, 1.0, fract(scaled));
    let segment = u32(floor(scaled));
    switch segment {
        case 0u: { return mix(params.color_a.rgb, params.color_b.rgb, local); }
        case 1u: { return mix(params.color_b.rgb, params.color_c.rgb, local); }
        case 2u: { return mix(params.color_c.rgb, params.color_d.rgb, local); }
        default: { return mix(params.color_d.rgb, params.color_a.rgb, local); }
    }
}

fn ridge(value: f32, sharpness: f32) -> f32 {
    return pow(max(0.0, 1.0 - abs(sin(value))), sharpness);
}

fn glow(value: f32, sharpness: f32) -> f32 {
    return exp(-abs(value) * sharpness);
}

fn paint(field: f32, light: f32) -> vec3<f32> {
    return palette_field(field) * (0.045 + max(light, 0.0));
}

fn phase_time(time: f32) -> f32 {
    let drive = params.style_b.y;
    return time * (0.08 + drive * 1.92) * (0.5 + params.visual.x * 0.5);
}

fn warp_spiral(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let radius = max(length(uv), 0.035);
    let angle = atan2(uv.y, uv.x);
    let depth = 1.0 / radius;
    let phase = depth * 2.15 + angle * 5.0 - phase_time(time) * TAU;
    let spiral = ridge(phase, 6.0);
    let spokes = ridge(angle * 9.0 + depth * 0.42 + phase_time(time) * 0.7, 10.0) * 0.35;
    return paint(angle / TAU + depth * 0.075, spiral * (0.45 + params.style_b.y * 0.7) + spokes)
        * smoothstep(0.025, 0.32, radius);
}

fn moire_rings(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let travel = phase_time(time) * 0.08;
    let first = length(uv - vec2<f32>(sin(travel) * 0.24, cos(travel * 0.83) * 0.2));
    let second = length(uv + vec2<f32>(cos(travel * 0.91) * 0.22, sin(travel) * 0.18));
    let interference = abs(sin(first * 30.0) - sin(second * 31.5));
    let bands = 1.0 - smoothstep(0.08, 0.42, interference);
    return paint((first - second) * 1.8 + time * 0.012, bands * (0.48 + params.style_b.y * 0.72));
}

fn infinite_checker(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let p = rotate2(uv, sin(phase_time(time) * 0.13) * 0.25);
    let horizon = max(abs(p.y + 0.15), 0.08);
    let projected = vec2<f32>(p.x / horizon, 1.0 / horizon + phase_time(time) * 0.55);
    let tiles = abs(step(0.5, fract(projected.x * 2.2)) - step(0.5, fract(projected.y * 0.72)));
    let edge = ridge(projected.x * PI, 12.0) + ridge(projected.y * PI, 12.0);
    return paint(projected.y * 0.035 + tiles * 0.25, tiles * 0.52 + edge * 0.18)
        * (1.0 - smoothstep(0.2, 1.45, length(uv)));
}

fn neon_lattice(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let turn = phase_time(time) * 0.18;
    let p = rotate2(uv, turn * 0.13);
    let grid_a = ridge(p.x * 12.0 + sin(p.y * 4.0 + turn), 11.0);
    let grid_b = ridge(p.y * 12.0 + sin(p.x * 4.0 - turn * 1.2), 11.0);
    let diagonal = ridge((p.x + p.y) * 8.0 - turn * 1.4, 14.0) * params.style_b.y;
    return paint(p.x * 0.18 + p.y * 0.12, max(grid_a, grid_b) * 0.58 + diagonal * 0.3);
}

fn twisted_stripes(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let turn = phase_time(time);
    let twist = sin(uv.y * 4.5 - turn * 0.8) * (0.25 + params.style_b.y * 0.45);
    let stripes = ridge((uv.x + twist) * (10.0 + params.scene.z * 5.0) + turn, 7.0);
    let cross = ridge((uv.y - twist * 0.55) * 8.0 - turn * 0.7, 12.0) * 0.32;
    return paint(uv.y * 0.22 + twist * 0.3, stripes * 0.65 + cross);
}

fn rotating_snakes(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let radius = length(uv);
    let turns = atan2(uv.y, uv.x) / TAU;
    let sequence = fract(turns * 14.0 + radius * 5.5 - phase_time(time) * 0.08) * 4.0;
    var luminance = 0.18;
    if sequence >= 1.0 && sequence < 2.0 {
        luminance = 0.82;
    } else if sequence >= 2.0 && sequence < 3.0 {
        luminance = 0.36;
    } else if sequence >= 3.0 {
        luminance = 1.0;
    }
    let rings = ridge(radius * 25.0, 5.0) * 0.35 + 0.65;
    return mix(vec3<f32>(luminance * rings), palette_field(turns * 2.0 + radius), 0.48 + params.style_b.y * 0.35)
        * (1.0 - smoothstep(1.2, 1.5, radius));
}

fn hyperbolic_tunnel(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let p = rotate2(uv, phase_time(time) * 0.045);
    let saddle = (p.x * p.x - p.y * p.y) * (12.0 + params.scene.z * 7.0);
    let hyperbola = ridge(saddle - phase_time(time) * 1.3, 8.0);
    let crossing = ridge(p.x * p.y * 28.0 + phase_time(time) * 0.9, 9.0) * 0.55;
    return paint(saddle * 0.055, max(hyperbola, crossing) * (0.48 + params.style_b.y * 0.65));
}

fn chromatic_maze(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let p = rotate2(uv, floor(phase_time(time) * 0.18) * PI * 0.5);
    let cell = fract((p + 1.4) * (4.0 + params.scene.z * 2.0)) - 0.5;
    let walls = max(glow(abs(cell.x) - 0.34, 42.0), glow(abs(cell.y) - 0.34, 42.0));
    let gates = step(0.1, sin((floor(p.x * 5.0) + floor(p.y * 5.0)) * 7.1));
    return paint(dot(floor(p * 5.0), vec2<f32>(0.07, 0.11)), walls * (0.35 + gates * 0.48));
}

fn vortex_chevron(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let radius = length(uv);
    let angle = atan2(uv.y, uv.x);
    let teeth = abs(fract(angle / TAU * 18.0 + 0.5) - 0.5);
    let chevron = ridge(radius * 18.0 + teeth * 7.0 - phase_time(time) * 2.0, 8.0);
    return paint(angle / TAU * 3.0 + radius * 0.32, chevron * (0.5 + params.style_b.y * 0.72));
}

fn glass_orbit(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let turn = phase_time(time) * 0.34;
    let a = length(uv - vec2<f32>(cos(turn), sin(turn)) * 0.42);
    let b = length(uv - vec2<f32>(cos(turn + 2.1), sin(turn + 2.1)) * 0.55);
    let c = length(uv - vec2<f32>(cos(turn + 4.2), sin(turn + 4.2)) * 0.34);
    let orbits = ridge(a * 17.0, 12.0) + ridge(b * 15.0, 12.0) + ridge(c * 19.0, 12.0);
    return paint(a * 0.24 - b * 0.18 + c * 0.13, orbits * (0.24 + params.style_b.y * 0.42));
}

fn sine_interference(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let turn = phase_time(time);
    let field = sin(uv.x * 13.0 + turn)
        + sin(uv.y * 15.0 - turn * 0.83)
        + sin((uv.x + uv.y) * 9.0 + turn * 1.27);
    let contours = ridge(field * 2.2, 7.0);
    return paint(field * 0.11 + turn * 0.008, contours * (0.52 + params.style_b.y * 0.68));
}

fn impossible_cubes(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let p = rotate2(uv, phase_time(time) * 0.035);
    let iso_a = ridge((p.y + p.x * 0.577) * 13.0, 13.0);
    let iso_b = ridge((p.y - p.x * 0.577) * 13.0, 13.0);
    let iso_c = ridge(p.x * 11.26, 13.0);
    let cut = step(0.0, sin((p.x + p.y) * 6.5 + phase_time(time) * 0.42));
    return paint(p.x * 0.14 + p.y * 0.09, (iso_a * cut + iso_b * (1.0 - cut) + iso_c * 0.72) * 0.56);
}

fn polar_fan(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let radius = length(uv);
    let angle = atan2(uv.y, uv.x);
    let fan = ridge(angle * 22.0 + radius * 9.0 - phase_time(time) * 1.8, 9.0);
    let counter = ridge(angle * 11.0 - radius * 13.0 + phase_time(time), 12.0) * 0.42;
    return paint(angle / TAU * 4.0 - radius * 0.22, max(fan, counter) * (0.48 + params.style_b.y * 0.66));
}

fn gravity_lens(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let turn = phase_time(time) * 0.22;
    let lens = vec2<f32>(sin(turn) * 0.2, cos(turn * 0.73) * 0.16);
    let delta = uv - lens;
    let radius = max(length(delta), 0.05);
    let warped = uv + delta / (radius * radius + 0.12) * (0.08 + params.style_b.y * 0.11);
    let background = ridge((warped.x + warped.y * 0.35) * 15.0 - phase_time(time), 9.0);
    let halo = glow(radius - 0.32, 32.0);
    return paint(warped.x * 0.16 + radius * 0.3, background * 0.5 + halo * (0.35 + params.style_b.y * 0.5));
}

fn ribbon_wormhole(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let radius = max(length(uv), 0.04);
    let angle = atan2(uv.y, uv.x);
    let depth = 1.0 / radius;
    let ribbon_a = ridge(angle * 4.0 + depth * 2.8 - phase_time(time) * 2.4, 8.0);
    let ribbon_b = ridge(angle * 4.0 - depth * 2.3 + phase_time(time) * 1.7, 10.0) * 0.62;
    return paint(angle / TAU + depth * 0.09, max(ribbon_a, ribbon_b) * (0.46 + params.style_b.y * 0.7))
        * smoothstep(0.025, 0.28, radius);
}

fn quantum_weave(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let p = rotate2(uv, sin(phase_time(time) * 0.17) * 0.5);
    let weave_a = ridge(p.x * p.y * 24.0 + p.x * 5.0 - phase_time(time), 8.0);
    let weave_b = ridge(p.x * p.y * 24.0 - p.y * 5.0 + phase_time(time) * 0.8, 8.0);
    let phase = step(0.0, sin((p.x - p.y) * 12.0));
    return paint(p.x * p.y * 0.7, mix(weave_a, weave_b, phase) * (0.5 + params.style_b.y * 0.68));
}

fn fractal_compass(uv: vec2<f32>, time: f32) -> vec3<f32> {
    var p = abs(rotate2(uv, phase_time(time) * 0.04));
    p = abs(fract(p * (2.6 + params.scene.z)) - 0.5);
    let compass = ridge(atan2(p.y, p.x) * 8.0 + length(p) * 18.0 - phase_time(time), 9.0);
    let diamonds = ridge((p.x + p.y) * 24.0, 11.0) * 0.4;
    return paint(p.x * 0.7 + p.y * 0.4, max(compass, diamonds) * (0.48 + params.style_b.y * 0.66));
}

fn liquid_circuit(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let p = uv + vec2<f32>(
        sin(uv.y * 4.0 + phase_time(time)) * params.style_b.y * 0.14,
        sin(uv.x * 3.5 - phase_time(time) * 0.8) * params.style_b.y * 0.14,
    );
    let cell = fract(p * 6.0) - 0.5;
    let horizontal = glow(cell.y, 34.0) * step(abs(cell.x), 0.42);
    let vertical = glow(cell.x, 34.0) * step(abs(cell.y), 0.42);
    let node = glow(length(cell) - 0.24, 42.0);
    return paint(dot(floor(p * 6.0), vec2<f32>(0.05, 0.09)) + phase_time(time) * 0.012, max(horizontal, vertical) * 0.52 + node * 0.4);
}

fn alien_heads(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let drift = vec2<f32>(phase_time(time) * 0.08, sin(phase_time(time) * 0.3) * 0.08);
    let cell = fract((uv + drift) * vec2<f32>(2.2, 1.9)) - 0.5;
    let head_radius = length(vec2<f32>(cell.x * 1.18, cell.y * 0.82));
    let head = max(
        (1.0 - smoothstep(0.4, 0.47, head_radius))
            - (1.0 - smoothstep(0.33, 0.38, head_radius)),
        0.0,
    );
    let eye_left = glow(length((cell - vec2<f32>(-0.14, 0.05)) * vec2<f32>(1.0, 1.8)) - 0.075, 55.0);
    let eye_right = glow(length((cell - vec2<f32>(0.14, 0.05)) * vec2<f32>(1.0, 1.8)) - 0.075, 55.0);
    let signal = ridge(cell.y * 18.0 - phase_time(time) * 2.0, 14.0)
        * (1.0 - smoothstep(0.15, 0.4, abs(cell.x)));
    return paint(cell.x * 0.4 + floor((uv.x + 1.5) * 2.2) * 0.13, head * 0.52 + (eye_left + eye_right) * (0.3 + params.style_b.y * 0.55) + signal * 0.18);
}

fn prism_vortex(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let radius = length(uv);
    let angle = atan2(uv.y, uv.x);
    let phase = angle * 7.0 + radius * 16.0 - phase_time(time) * 2.2;
    let first = ridge(phase, 8.0);
    let second = ridge(phase + TAU / 3.0, 8.0);
    let third = ridge(phase + TAU * 2.0 / 3.0, 8.0);
    return params.color_b.rgb * first * 0.55
        + params.color_c.rgb * second * 0.55
        + params.color_d.rgb * third * 0.55;
}

fn diamond_drift(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let p = rotate2(uv, phase_time(time) * 0.055);
    let cell = fract(p * (3.5 + params.scene.z * 1.8)) - 0.5;
    let diamond = abs(cell.x) + abs(cell.y);
    let shells = ridge(diamond * 18.0 - phase_time(time) * 1.2, 9.0);
    let edge = glow(diamond - 0.36, 36.0);
    return paint(diamond * 0.6 + dot(floor(p * 4.0), vec2<f32>(0.04, 0.08)), max(shells * 0.55, edge));
}

fn orbital_mesh(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let radius = length(uv);
    let angle = atan2(uv.y, uv.x);
    let radial = ridge(radius * 24.0 - phase_time(time) * 1.5, 11.0);
    let angular = ridge(angle * 16.0 + sin(radius * 8.0 - phase_time(time)) * 1.4, 12.0);
    let nodes = radial * angular;
    return paint(angle / TAU * 2.0 + radius * 0.33, max(radial, angular) * 0.28 + nodes * (0.45 + params.style_b.y * 0.5));
}

fn helix_portal(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let p = rotate2(uv, PI * 0.5);
    let turn = phase_time(time);
    let rail_a = glow(p.y - sin(p.x * 8.0 - turn) * (0.24 + params.style_b.y * 0.12), 28.0);
    let rail_b = glow(p.y + sin(p.x * 8.0 - turn) * (0.24 + params.style_b.y * 0.12), 28.0);
    let rungs = ridge(p.x * 14.0 - turn * 1.7, 13.0)
        * (1.0 - smoothstep(0.08, 0.5, abs(p.y)));
    return paint(p.x * 0.18 + turn * 0.01, (rail_a + rail_b) * 0.43 + rungs * 0.35);
}

fn radial_escalator(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let radius = length(uv);
    let turns = atan2(uv.y, uv.x) / TAU;
    let stair = fract(turns * 14.0 + radius * 5.0 - phase_time(time) * 0.4);
    let riser = glow(stair - 0.08, 30.0);
    let tread = step(0.52, stair) * ridge(radius * 15.0, 10.0);
    return paint(turns * 3.0 + stair * 0.28, riser * 0.58 + tread * 0.35);
}

fn electric_topography(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let turn = phase_time(time);
    let height = sin(uv.x * 4.0 + turn * 0.6)
        + sin(uv.y * 5.0 - turn * 0.5)
        + sin((uv.x - uv.y) * 3.0 + turn * 0.8);
    let contours = ridge(height * (5.0 + params.scene.z * 2.0), 12.0);
    let fault = glow(uv.y - sin(uv.x * 3.0 + turn) * 0.28, 32.0) * params.style_b.y;
    return paint(height * 0.13 + turn * 0.009, contours * 0.58 + fault * 0.38);
}

fn event_horizon(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let radius = max(length(uv), 0.015);
    let angle = atan2(uv.y, uv.x);
    let warped_radius = radius + sin(angle * 7.0 - phase_time(time) * 1.4) * params.style_b.y * 0.055;
    let horizon = smoothstep(0.22, 0.27, warped_radius);
    let disk = glow(abs(uv.y) - 0.035 / (radius + 0.2), 38.0) * smoothstep(0.15, 0.7, radius);
    let lens_ring = glow(warped_radius - 0.32, 38.0);
    let color = paint(angle / TAU + radius * 0.4, disk * (0.45 + params.style_b.y * 0.65) + lens_ring * 0.62);
    return color * horizon;
}

fn visual_family(id: u32, uv: vec2<f32>, time: f32) -> vec3<f32> {
    switch id {
        case 0u: { return warp_spiral(uv, time); }
        case 1u: { return moire_rings(uv, time); }
        case 2u: { return infinite_checker(uv, time); }
        case 3u: { return neon_lattice(uv, time); }
        case 4u: { return twisted_stripes(uv, time); }
        case 5u: { return rotating_snakes(uv, time); }
        case 6u: { return hyperbolic_tunnel(uv, time); }
        case 7u: { return chromatic_maze(uv, time); }
        case 8u: { return vortex_chevron(uv, time); }
        case 9u: { return glass_orbit(uv, time); }
        case 10u: { return sine_interference(uv, time); }
        case 11u: { return impossible_cubes(uv, time); }
        case 12u: { return polar_fan(uv, time); }
        case 13u: { return gravity_lens(uv, time); }
        case 14u: { return ribbon_wormhole(uv, time); }
        case 15u: { return quantum_weave(uv, time); }
        case 16u: { return fractal_compass(uv, time); }
        case 17u: { return liquid_circuit(uv, time); }
        case 18u: { return alien_heads(uv, time); }
        case 19u: { return prism_vortex(uv, time); }
        case 20u: { return diamond_drift(uv, time); }
        case 21u: { return orbital_mesh(uv, time); }
        case 22u: { return helix_portal(uv, time); }
        case 23u: { return radial_escalator(uv, time); }
        case 24u: { return electric_topography(uv, time); }
        case 25u: { return event_horizon(uv, time); }
        default: { return warp_spiral(uv, time); }
    }
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let resolution = max(params.resolution_time.xy, vec2<f32>(1.0));
    var uv = (position.xy * 2.0 - resolution) / resolution.y;
    let time = params.resolution_time.z;
    let drive = clamp(params.style_b.y, 0.0, 1.0);
    let beat_zoom = modifier_strength(1u);
    let bass_warp = modifier_strength(2u);
    let mirror_fold = modifier_strength(5u);

    uv = rotate2(uv, sin(time * (0.16 + drive * 0.8)) * drive * 0.045);
    uv *= 1.0 - params.pulse.y * drive * (0.025 + beat_zoom * 0.055) - params.pulse.w * 0.06;
    uv += vec2<f32>(
        sin(uv.y * 3.2 + time * 1.1),
        sin(uv.x * 2.8 - time * 0.9),
    ) * params.music.y * drive * (0.035 + bass_warp * 0.07);
    uv.x = mix(uv.x, abs(uv.x) - 0.3, mirror_fold * drive * 0.7);

    let primary_id = u32(round(params.style_a.x));
    let secondary_id = u32(round(params.style_a.y));
    var color = visual_family(primary_id, uv, time) * params.style_a.z;
    if params.style_a.w > 0.001 {
        color += visual_family(secondary_id, uv, time) * params.style_a.w;
    }

    let vignette = 1.0 - smoothstep(0.25, 1.55, length(uv * vec2<f32>(0.7, 1.0)));
    color *= (0.32 + vignette * 0.78) * params.visual.w;
    color += palette_field(length(uv) * 0.2 + time * 0.01)
        * params.pulse.z
        * drive
        * 0.16;

    let sparkle = modifier_strength(3u);
    if sparkle > 0.001 {
        let cell = floor((uv + time * vec2<f32>(0.12, -0.08)) * 38.0);
        let seed = hash21(cell);
        let mask = step(0.988 - params.music.w * 0.025, seed)
            * ridge(time * 8.0 + seed * TAU, 12.0);
        color += palette_field(seed) * mask * sparkle * drive * 0.32;
    }

    let trails = modifier_strength(4u);
    if trails > 0.001 {
        let trail = ridge((uv.x - uv.y) * 6.0 - time * (0.8 + drive * 2.0), 14.0);
        color += palette_field(uv.x * 0.14 - time * 0.03) * trail * trails * drive * 0.14;
    }

    let chromatic = modifier_strength(6u);
    color = mix(color, color.gbr, chromatic * drive * params.pulse.z * 0.18);
    let impact_bloom = modifier_strength(7u);
    let impact_level = clamp(params.effects.y, 0.0, 1.0);
    let impact_ring = glow(length(uv) - (0.16 + impact_level * 0.92), 24.0);
    color += palette_field(length(uv) * 0.3 + time * 0.04)
        * impact_ring
        * max(impact_bloom, impact_level * drive)
        * 0.44;

    color = mix(color, vec3<f32>(1.0), params.pulse.w * (0.54 + drive * 0.16));
    let luminance = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    let luminance_budget = 0.7 + params.scene.w * 0.32 + drive * 0.12;
    if luminance > luminance_budget {
        color *= luminance_budget / max(luminance, 0.001);
    }
    color = mix(vec3<f32>(dot(color, vec3<f32>(0.2126, 0.7152, 0.0722))), color, params.style_b.x);
    color = 1.0 - exp(-max(color, vec3<f32>(0.0)) * (1.22 + drive * 0.26));
    color = pow(max(color, vec3<f32>(0.0)), vec3<f32>(0.94));
    if params.style_b.w > 0.5 {
        color = vec3<f32>(0.0);
    }
    return vec4<f32>(color, 1.0);
}
