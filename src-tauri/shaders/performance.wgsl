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
    reactive: vec4<f32>,
    feedback: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: VisualParams;
@group(0) @binding(1) var feedback_texture: texture_2d<f32>;
@group(0) @binding(2) var feedback_sampler: sampler;

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
    let hit_shift = params.reactive.x * 0.07
        + params.reactive.z * 0.19
        + params.pulse.z * (0.06 + drive * 0.08);
    let scaled = fract(
        value + params.resolution_time.z * speed + params.pulse.x * drive * 0.04 + hit_shift,
    ) * 4.0;
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
    let raw = pow(max(0.0, 1.0 - abs(sin(value))), sharpness);
    let detail_visibility = 1.0 - smoothstep(0.72, 1.9, fwidth(value));
    return raw * detail_visibility;
}

fn glow(value: f32, sharpness: f32) -> f32 {
    let raw = exp(-abs(value) * sharpness);
    let detail_visibility = 1.0 - smoothstep(0.7, 2.1, fwidth(value) * sharpness);
    return raw * detail_visibility;
}

fn paint(field: f32, light: f32) -> vec3<f32> {
    return palette_field(field) * (0.045 + max(light, 0.0));
}

fn phase_time(time: f32) -> f32 {
    let drive = params.style_b.y;
    let musical_nudge = params.pulse.y * (0.12 + drive * 0.42)
        + params.reactive.x * 0.24
        + params.reactive.z * 0.11;
    return time * (0.08 + drive * 1.92) * (0.5 + params.visual.x * 0.5)
        + musical_nudge;
}

fn techno_laser_grid(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let turn = phase_time(time);
    let floor_depth = max(uv.y + 1.08, 0.09);
    let projected = vec2<f32>(uv.x / floor_depth, 1.0 / floor_depth + turn * 0.34);
    let floor_gate = smoothstep(-1.18, -0.72, uv.y) * (1.0 - smoothstep(-0.05, 0.38, uv.y));
    let grid_columns = ridge(projected.x * PI * 1.25, 14.0);
    let grid_rows = ridge(projected.y * 2.15, 14.0);
    let floor_grid = max(grid_columns * 0.72, grid_rows) * floor_gate;

    let sweep = sin(turn * 0.42) * (0.24 + params.style_b.y * 0.18);
    let left_beam = glow(
        uv.x + 0.86 - (uv.y + 0.92) * (0.48 + sweep),
        64.0,
    );
    let right_beam = glow(
        uv.x - 0.86 + (uv.y + 0.92) * (0.48 - sweep),
        64.0,
    );
    let center_beam = glow(
        uv.x - sin(turn * 0.68) * (uv.y + 1.05) * 0.38,
        82.0,
    );
    let upper_gate = smoothstep(-1.2, -0.78, uv.y) * (1.0 - smoothstep(0.94, 1.42, uv.y));
    let lasers = (left_beam + right_beam) * 0.58 + center_beam * 0.42;

    let frame_width = 0.58 + sin(turn * 0.2) * 0.08;
    let frame = glow(abs(uv.x) - frame_width, 72.0)
        * smoothstep(-0.78, -0.38, uv.y)
        * (1.0 - smoothstep(0.64, 1.02, uv.y));
    let light_bar = ridge((uv.y + 0.82) * 12.0 - turn * 0.9, 12.0)
        * (1.0 - smoothstep(0.5, 0.82, abs(uv.x)));
    let floor_color = palette_field(projected.y * 0.045 + projected.x * 0.025);
    let laser_color = palette_field(0.24 + uv.y * 0.12 + turn * 0.012);
    return floor_color * floor_grid * 0.48
        + laser_color * lasers * upper_gate * (0.34 + params.style_b.y * 0.48)
        + palette_field(0.68 + turn * 0.01) * (frame * 0.5 + light_bar * 0.2);
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

fn spinning_alien(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let turn = phase_time(time) * 0.34;
    let facing = cos(turn);
    let side = sin(turn);
    let face_width = 0.42 + abs(facing) * 0.58;
    let tilted = rotate2(uv, sin(turn * 0.5) * 0.055);
    let face = vec2<f32>((tilted.x - side * 0.055) / face_width, tilted.y + 0.025);
    let chin_taper = 1.48 + max(-face.y, 0.0) * 1.35;
    let head_distance = length(vec2<f32>(face.x * chin_taper, (face.y - 0.04) * 1.08));
    let head_mask = 1.0 - smoothstep(0.57, 0.63, head_distance);
    let head_edge = glow(head_distance - 0.605, 46.0);

    let left_eye_point = rotate2(face - vec2<f32>(-0.19 + side * 0.035, 0.1), -0.22);
    let right_eye_point = rotate2(face - vec2<f32>(0.19 + side * 0.035, 0.1), 0.22);
    let left_eye_distance = length(left_eye_point * vec2<f32>(1.0, 2.75));
    let right_eye_distance = length(right_eye_point * vec2<f32>(1.0, 2.75));
    let left_eye = (1.0 - smoothstep(0.085, 0.13, left_eye_distance))
        * clamp(0.76 - side * 0.58, 0.1, 1.0);
    let right_eye = (1.0 - smoothstep(0.085, 0.13, right_eye_distance))
        * clamp(0.76 + side * 0.58, 0.1, 1.0);
    let eyes = left_eye + right_eye;
    let scan = ridge(face.y * 19.0 - turn * 3.2, 15.0) * head_mask;
    let facial_shade = clamp(0.45 + face.x * side * 0.9, 0.08, 1.0) * head_mask;

    let halo_radius = length(uv);
    let halo = ridge(halo_radius * 13.0 - turn * 1.1, 13.0)
        * smoothstep(0.58, 0.72, halo_radius)
        * (1.0 - smoothstep(1.12, 1.45, halo_radius));
    let head_color = palette_field(0.2 + side * 0.13 + face.y * 0.16);
    let eye_color = mix(params.color_c.rgb, params.color_d.rgb, side * 0.5 + 0.5);
    return head_color * (facial_shade * 0.16 + head_edge * 0.62 + scan * 0.12)
        + eye_color * eyes * (0.62 + params.style_b.y * 0.38)
        + palette_field(halo_radius * 0.24 - turn * 0.025) * halo * 0.18;
}

fn spinning_skull(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let turn = phase_time(time) * 0.27;
    let facing = cos(turn);
    let side = sin(turn);
    let tilted = rotate2(uv, sin(turn * 0.43) * 0.045);
    let face_width = 0.55 + abs(facing) * 0.45;
    let face = vec2<f32>((tilted.x - side * 0.045) / face_width, tilted.y + 0.015);

    let cranium_distance = length(vec2<f32>(face.x * 0.88, (face.y - 0.13) * 1.02));
    let cranium = 1.0 - smoothstep(0.51, 0.56, cranium_distance);
    let cranium_edge = glow(cranium_distance - 0.535, 50.0);
    let jaw_width = 0.27 - max(-face.y - 0.28, 0.0) * 0.2;
    let jaw_distance = max(abs(face.x) - jaw_width, abs(face.y + 0.31) - 0.18);
    let jaw = 1.0 - smoothstep(0.0, 0.04, jaw_distance);
    let jaw_edge = glow(jaw_distance, 52.0) * smoothstep(-0.48, -0.08, face.y);
    let silhouette = max(cranium, jaw);

    let left_socket_point = face - vec2<f32>(-0.2 + side * 0.035, 0.1);
    let right_socket_point = face - vec2<f32>(0.2 + side * 0.035, 0.1);
    let left_socket = (1.0 - smoothstep(0.115, 0.16, length(left_socket_point * vec2<f32>(1.0, 1.42))))
        * clamp(0.82 - side * 0.48, 0.18, 1.0);
    let right_socket = (1.0 - smoothstep(0.115, 0.16, length(right_socket_point * vec2<f32>(1.0, 1.42))))
        * clamp(0.82 + side * 0.48, 0.18, 1.0);
    let sockets = clamp(left_socket + right_socket, 0.0, 1.0);
    let nose = 1.0 - smoothstep(
        0.055,
        0.095,
        abs(face.x - side * 0.016) + abs(face.y + 0.075) * 0.58,
    );
    let mouth = glow(face.y + 0.285, 42.0)
        * jaw
        * (1.0 - smoothstep(0.14, 0.28, abs(face.x)));
    let surface = silhouette
        * (1.0 - clamp(sockets * 0.92 + nose * 0.82 + mouth * 0.72, 0.0, 0.95));
    let bone_color = palette_field(0.52 + face.y * 0.11 + side * 0.08);
    let rim_color = mix(params.color_c.rgb, params.color_d.rgb, side * 0.5 + 0.5);
    return bone_color * surface * (0.23 + params.style_b.y * 0.08)
        + rim_color * (cranium_edge + jaw_edge) * 0.48
        + params.color_d.rgb * (left_socket + right_socket) * 0.035;
}

fn watching_eye(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let turn = phase_time(time) * 0.19;
    let p = rotate2(uv, sin(turn * 0.37) * 0.08);
    let normalized_x = p.x / 0.84;
    let lid_height = 0.31 * sqrt(max(0.0, 1.0 - normalized_x * normalized_x));
    let horizontal_gate = 1.0 - smoothstep(0.8, 0.86, abs(p.x));
    let eye = (1.0 - smoothstep(lid_height, lid_height + 0.028, abs(p.y)))
        * horizontal_gate;
    let lid = glow(abs(p.y) - lid_height, 58.0) * horizontal_gate;
    let gaze = vec2<f32>(
        sin(turn + params.reactive.y * 0.8) * 0.16,
        sin(turn * 0.73 - params.reactive.z * 0.6) * 0.075,
    );
    let iris_distance = length(p - gaze);
    let iris = (1.0 - smoothstep(0.205, 0.245, iris_distance)) * eye;
    let pupil = (1.0 - smoothstep(0.065, 0.105, iris_distance)) * eye;
    let highlight = glow(length(p - gaze - vec2<f32>(-0.07, 0.075)), 92.0) * iris;
    let sclera = palette_field(0.08 + p.x * 0.08 + turn * 0.006);
    let iris_color = palette_field(0.62 + iris_distance * 0.4 - turn * 0.012);
    return sclera * eye * (1.0 - iris * 0.68) * (1.0 - pupil * 0.9) * 0.2
        + iris_color * iris * (1.0 - pupil) * 0.56
        + mix(params.color_c.rgb, params.color_d.rgb, 0.5) * lid * 0.42
        + vec3<f32>(0.72, 0.86, 1.0) * highlight * 0.3;
}

fn triangle_weights(
    point: vec2<f32>,
    first: vec2<f32>,
    second: vec2<f32>,
    third: vec2<f32>,
) -> vec3<f32> {
    let edge_a = second - first;
    let edge_b = third - first;
    let local = point - first;
    let denominator = edge_a.x * edge_b.y - edge_b.x * edge_a.y;
    let second_weight = (local.x * edge_b.y - edge_b.x * local.y) / denominator;
    let third_weight = (edge_a.x * local.y - local.x * edge_a.y) / denominator;
    return vec3<f32>(1.0 - second_weight - third_weight, second_weight, third_weight);
}

fn morphing_pyramid(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let turn = phase_time(time) * 0.21;
    let p = rotate2(uv, sin(turn * 0.31) * 0.12);
    let apex = vec2<f32>(sin(turn) * 0.16, 0.68 + params.reactive.x * 0.045);
    let left = vec2<f32>(-0.72, -0.56 + sin(turn * 0.73) * 0.035);
    let right = vec2<f32>(0.72, -0.56 - sin(turn * 0.73) * 0.035);
    let weights = triangle_weights(p, apex, left, right);
    let boundary = min(weights.x, min(weights.y, weights.z));
    let pyramid = smoothstep(-0.02, 0.018, boundary);
    let outer_edge = glow(boundary, 48.0) * pyramid;

    let inner_scale = 0.42 + sin(turn * 0.62) * 0.045;
    let inner_weights = triangle_weights(
        p,
        apex * inner_scale + vec2<f32>(0.0, -0.03),
        left * inner_scale + vec2<f32>(0.0, -0.03),
        right * inner_scale + vec2<f32>(0.0, -0.03),
    );
    let inner_boundary = min(inner_weights.x, min(inner_weights.y, inner_weights.z));
    let hollow = smoothstep(-0.02, 0.02, inner_boundary);
    let hollow_edge = glow(inner_boundary, 52.0) * pyramid;

    let side_mix = smoothstep(-0.06, 0.06, p.x - apex.x * 0.35);
    var face_color = mix(params.color_b.rgb, params.color_c.rgb, side_mix);
    let base_face = 1.0 - smoothstep(-0.48, -0.16, p.y);
    face_color = mix(face_color, params.color_d.rgb, base_face * 0.55);
    return face_color * pyramid * (1.0 - hollow * 0.78) * 0.4
        + palette_field(0.72 + turn * 0.01) * outer_edge * 0.42
        + palette_field(0.24 - turn * 0.008) * hollow_edge * 0.24;
}

fn tumbling_cube(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let turn = phase_time(time) * 0.18;
    let p = rotate2(uv, turn * 0.24 + sin(turn * 0.4) * 0.08);
    let size = 0.56 + params.reactive.x * 0.035;
    let hex_field = max(abs(p.y), abs(p.x) * 0.866 + abs(p.y) * 0.5);
    let cube = 1.0 - smoothstep(size, size + 0.035, hex_field);
    let upper_face = smoothstep(-0.025, 0.025, p.y - abs(p.x) * 0.575);
    let right_face = smoothstep(-0.025, 0.025, p.x);
    var face_color = mix(params.color_b.rgb, params.color_c.rgb, right_face);
    face_color = mix(face_color, params.color_d.rgb, upper_face * 0.82);

    let outer_edge = glow(hex_field - size, 48.0);
    let upper_seam = glow(p.x, 72.0) * smoothstep(-0.02, 0.16, p.y);
    let lower_seam = glow(p.y + abs(p.x) * 0.575, 68.0)
        * (1.0 - smoothstep(-0.04, 0.08, p.y));
    let seams = (upper_seam + lower_seam) * cube;
    return face_color * cube * (0.3 + params.style_b.y * 0.08)
        + palette_field(0.3 + p.y * 0.14 + turn * 0.01) * outer_edge * 0.48
        + palette_field(0.78 - p.x * 0.1) * seams * 0.2;
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

fn kinetic_bars(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let turn = phase_time(time);
    let p = rotate2(uv, sin(turn * 0.11) * 0.055);
    let broad_bend = sin(p.y * 3.2 + turn * 0.62) * (0.17 + params.style_b.y * 0.11);
    let fine_bend = sin(p.y * 8.5 - turn * 0.94) * 0.045;
    let phase = (p.x + broad_bend + fine_bend) * (15.0 + params.scene.z * 7.0);
    let bars = smoothstep(0.42, 0.72, sin(phase) * 0.5 + 0.5);
    let edge = ridge(phase, 15.0);
    let counter_phase = (p.x - broad_bend * 0.45) * 8.0 + p.y * 3.0 + turn * 0.36;
    let counter = ridge(counter_phase, 13.0) * params.style_b.y;
    let edge_color = palette_field(p.y * 0.14 + broad_bend * 0.32 + turn * 0.008);
    let field_color = palette_field(0.55 + p.x * 0.1 - turn * 0.006);
    return edge_color * (bars * 0.32 + edge * 0.42)
        + field_color * counter * (1.0 - bars) * 0.28;
}

fn bulging_checker(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let turn = phase_time(time);
    let radius = length(uv);
    let bulge = 1.0
        + exp(-radius * radius * 2.35)
            * (0.54 + sin(turn * 0.42) * 0.09 + params.style_b.y * 0.18);
    let p = rotate2(uv * bulge, sin(turn * 0.16) * 0.08);
    let scale = 4.5 + params.scene.z * 2.6;
    let cell = fract(p * scale + vec2<f32>(turn * 0.07, -turn * 0.055));
    let tile = abs(step(0.5, cell.x) - step(0.5, cell.y));
    let edge_x = ridge(p.x * scale * PI + turn * 0.22, 14.0);
    let edge_y = ridge(p.y * scale * PI - turn * 0.18, 14.0);
    let lens_ring = glow(radius - (0.46 + sin(turn * 0.38) * 0.08), 27.0);
    let tile_color = palette_field(tile * 0.3 + radius * 0.16 + turn * 0.008);
    let alternate_color = palette_field(0.62 - radius * 0.12 - turn * 0.006);
    return mix(alternate_color * 0.08, tile_color * 0.48, tile)
        + palette_field(p.x * 0.09 - p.y * 0.07) * (max(edge_x, edge_y) * 0.24 + lens_ring * 0.32);
}

fn visual_family(id: u32, uv: vec2<f32>, time: f32) -> vec3<f32> {
    switch id {
        case 0u: { return techno_laser_grid(uv, time); }
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
        case 18u: { return spinning_alien(uv, time); }
        case 19u: { return prism_vortex(uv, time); }
        case 20u: { return diamond_drift(uv, time); }
        case 21u: { return orbital_mesh(uv, time); }
        case 22u: { return helix_portal(uv, time); }
        case 23u: { return radial_escalator(uv, time); }
        case 24u: { return electric_topography(uv, time); }
        case 25u: { return event_horizon(uv, time); }
        case 26u: { return kinetic_bars(uv, time); }
        case 27u: { return bulging_checker(uv, time); }
        case 28u: { return spinning_skull(uv, time); }
        case 29u: { return watching_eye(uv, time); }
        case 30u: { return morphing_pyramid(uv, time); }
        case 31u: { return tumbling_cube(uv, time); }
        default: { return techno_laser_grid(uv, time); }
    }
}

fn family_weight(id: u32) -> f32 {
    var weight = 0.0;
    if u32(round(params.style_a.x)) == id {
        weight += params.style_a.z;
    }
    if params.style_a.w > 0.001 && u32(round(params.style_a.y)) == id {
        weight += params.style_a.w;
    }
    return clamp(weight, 0.0, 1.0);
}

fn feedback_frame(screen_uv: vec2<f32>, time: f32) -> vec3<f32> {
    let resolution = max(params.resolution_time.xy, vec2<f32>(1.0));
    let aspect = resolution.x / resolution.y;
    var point = screen_uv * 2.0 - 1.0;
    point.x *= aspect;
    let radius = max(length(point), 0.001);
    let radial = point / radius;
    let seed_phase = params.style_b.z * TAU;
    let turn = (0.35 + sin(time * 0.11 + seed_phase) * 0.65)
        * (params.feedback.y * 0.48 + params.feedback.z * 0.36);
    point = rotate2(point, turn);
    point *= 1.0 - params.feedback.y
        * (1.0 + params.pulse.y * 0.72 + params.reactive.x * 0.58);
    let liquid_flow = vec2<f32>(
        sin(point.y * 3.7 + time * 0.21 + seed_phase),
        cos(point.x * 3.1 - time * 0.17 - seed_phase * 0.7),
    );
    let radial_flow = radial
        * sin(radius * (5.0 + params.scene.z * 2.5) - time * 0.34 + seed_phase)
        * (0.35 + params.reactive.x * 0.65);
    point += (liquid_flow + radial_flow) * params.feedback.z;

    point.x /= aspect;
    let sample_uv = point * 0.5 + 0.5;
    let edge = smoothstep(0.0, 0.025, sample_uv.x)
        * (1.0 - smoothstep(0.975, 1.0, sample_uv.x))
        * smoothstep(0.0, 0.025, sample_uv.y)
        * (1.0 - smoothstep(0.975, 1.0, sample_uv.y));
    let split_direction = vec2<f32>(
        cos(seed_phase + time * 0.07),
        sin(seed_phase * 0.73 - time * 0.06),
    ) * params.feedback.w;
    let center = textureSample(feedback_texture, feedback_sampler, sample_uv).rgb;
    let split = vec3<f32>(
        textureSample(feedback_texture, feedback_sampler, sample_uv + split_direction).r,
        center.g,
        textureSample(feedback_texture, feedback_sampler, sample_uv - split_direction).b,
    );
    let hue_drift = clamp(params.feedback.w * 5.0, 0.0, 0.08);
    return mix(split, split.gbr, hue_drift) * edge;
}

fn spectral_signal_ribbon(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let drive = clamp(params.style_b.y, 0.0, 1.0);
    let trails = modifier_strength(4u);
    let phase = phase_time(time) * 1.35 + params.style_b.z * TAU;
    let carrier = sin(uv.x * (7.0 + params.music.z * 6.0) + phase)
        * (0.035 + params.music.y * 0.09);
    let harmonic = sin(uv.x * (17.0 + params.music.w * 11.0) - phase * 1.47)
        * (0.012 + params.music.w * 0.035);
    let center = sin(time * 0.13 + params.style_b.z * 11.0) * 0.27;
    let signal = center + carrier + harmonic;
    let main_trace = glow(uv.y - signal, 78.0);
    let after_trace = glow(uv.y - signal - 0.035 - params.reactive.y * 0.025, 34.0);
    let sample_lights = ridge(
        uv.x * (22.0 + params.scene.z * 12.0) - phase * 0.6,
        16.0,
    ) * main_trace;
    let horizontal_gate = smoothstep(-1.32, -1.08, uv.x)
        * (1.0 - smoothstep(1.08, 1.32, uv.x));
    let signal_scene = clamp(family_weight(18u) + family_weight(29u) * 0.35, 0.0, 1.0);
    let strength = (0.012 + drive * 0.035 + trails * 0.04)
        * (0.45 + params.music.z * 0.32 + params.music.w * 0.23);
    return palette_field(uv.x * 0.14 + phase * 0.012)
        * (main_trace + after_trace * 0.24 + sample_lights * 0.38)
        * horizontal_gate
        * strength
        * signal_scene;
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let resolution = max(params.resolution_time.xy, vec2<f32>(1.0));
    let screen_uv = position.xy / resolution;
    var uv = (position.xy * 2.0 - resolution) / resolution.y;
    let time = params.resolution_time.z;
    let drive = clamp(params.style_b.y, 0.0, 1.0);
    let beat_zoom = modifier_strength(1u);
    let bass_warp = modifier_strength(2u);
    let mirror_fold = modifier_strength(5u);
    let bass_hit = clamp(params.reactive.x, 0.0, 1.0);
    let mid_motion = clamp(params.reactive.y, 0.0, 1.0);
    let high_hit = clamp(params.reactive.z, 0.0, 1.0);
    let energy_rise = clamp(params.reactive.w, 0.0, 1.0);

    let source_radius = max(length(uv), 0.001);
    let radial_direction = uv / source_radius;
    let bass_wave = sin(
        source_radius * (12.0 + params.scene.z * 9.0) - params.pulse.x * TAU,
    );
    uv += radial_direction * bass_wave * bass_hit * (0.026 + drive * 0.046);
    let mid_bend = vec2<f32>(
        sin(uv.y * (4.0 + params.music.z * 4.5) + phase_time(time) * 0.7),
        sin(uv.x * (3.4 + params.music.z * 3.8) - phase_time(time) * 0.56),
    );
    uv += mid_bend * mid_motion * (0.024 + params.music.z * 0.038);
    let slice_rate = 6.0 + floor(params.music.w * 8.0);
    let slice = floor((uv.y + 1.7) * slice_rate);
    let slice_tick = floor(time * (6.0 + high_hit * 12.0) + params.pulse.x * 4.0);
    uv.x += (hash21(vec2<f32>(slice, slice_tick)) - 0.5)
        * high_hit
        * (0.026 + drive * 0.046);
    let onset_tick = floor(time * 10.0);
    let onset_turn = (hash21(vec2<f32>(onset_tick, params.style_b.z * 97.0)) - 0.5)
        * params.pulse.z
        * (0.045 + drive * 0.055);
    uv = rotate2(uv, onset_turn);

    uv = rotate2(uv, sin(time * (0.16 + drive * 0.8)) * drive * 0.045);
    uv *= 1.0
        - params.pulse.y * (0.035 + drive * 0.06 + beat_zoom * 0.075)
        - bass_hit * (0.055 + beat_zoom * 0.06)
        - energy_rise * 0.028
        - params.pulse.w * 0.06;
    uv += vec2<f32>(
        sin(uv.y * 3.2 + time * 1.1),
        sin(uv.x * 2.8 - time * 0.9),
    ) * (
        params.music.y * drive * (0.035 + bass_warp * 0.07)
        + mid_motion * (0.025 + bass_warp * 0.025)
    );
    uv.x = mix(uv.x, abs(uv.x) - 0.3, mirror_fold * drive * 0.7);

    let primary_id = u32(round(params.style_a.x));
    let secondary_id = u32(round(params.style_a.y));
    var color = visual_family(primary_id, uv, time) * params.style_a.z;
    if params.style_a.w > 0.001 {
        color += visual_family(secondary_id, uv, time) * params.style_a.w;
    }

    let vignette = 1.0 - smoothstep(0.25, 1.55, length(uv * vec2<f32>(0.7, 1.0)));
    color *= (0.32 + vignette * 0.78)
        * params.visual.w
        * (0.86 + params.pulse.y * 0.17 + bass_hit * 0.16 + energy_rise * 0.3);
    color += palette_field(length(uv) * 0.2 + time * 0.01)
        * params.pulse.z
        * drive
        * 0.16;

    let response_radius = length(uv);
    let response_angle = atan2(uv.y, uv.x);
    let bass_front = glow(
        response_radius - (0.12 + fract(params.pulse.x + bass_hit * 0.08) * 1.18),
        20.0,
    );
    color += palette_field(response_angle / TAU + response_radius * 0.42)
        * bass_front
        * bass_hit
        * (0.12 + drive * 0.18);
    let mid_ribs = ridge(
        (uv.x + uv.y * 0.74) * (5.0 + params.music.z * 4.0)
            + phase_time(time) * 0.9,
        10.0,
    );
    color += palette_field(uv.x * 0.21 - uv.y * 0.13 + params.music.z * 0.24)
        * mid_ribs
        * mid_motion
        * (0.025 + params.music.z * 0.06);
    let shard_count = 8.0 + floor(params.music.w * 8.0);
    let high_ray = ridge(
        response_angle * shard_count
            + sin(response_radius * 7.0 - phase_time(time)) * 1.15
            + time * (2.2 + high_hit * 3.4),
        16.0,
    );
    let high_gate = 0.28
        + ridge(
            response_radius * 19.0 - phase_time(time) * 1.8 + params.style_b.z * TAU,
            11.0,
        ) * 0.72;
    let high_shard = high_ray
        * high_gate
        * smoothstep(0.08, 0.34, response_radius)
        * (1.0 - smoothstep(1.1, 1.58, response_radius));
    color += palette_field(response_angle / TAU * 4.0 + response_radius * 0.3)
        * high_shard
        * high_hit
        * (0.07 + drive * 0.12);
    let spectral_tint = palette_field(
        response_angle / TAU
            + params.music.y * 0.12
            + params.music.z * 0.28
            + params.music.w * 0.46,
    );
    let color_reaction = clamp(
        bass_hit * 0.1
            + mid_motion * 0.08
            + high_hit * 0.18
            + params.pulse.z * 0.12,
        0.0,
        0.55,
    );
    color = mix(color, color * (0.52 + spectral_tint * 1.58), color_reaction);

    let sparkle = modifier_strength(3u);
    if sparkle > 0.001 {
        let cell = floor((uv + time * vec2<f32>(0.12, -0.08)) * 38.0);
        let seed = hash21(cell);
        let mask = step(0.988 - params.music.w * 0.025, seed)
            * ridge(time * 8.0 + seed * TAU, 12.0);
        color += palette_field(seed) * mask * sparkle * drive * 0.32;
    }

    color += spectral_signal_ribbon(uv, time);

    let chromatic = modifier_strength(6u);
    color = mix(color, color.gbr, chromatic * drive * params.pulse.z * 0.18);
    let impact_bloom = modifier_strength(7u);
    let impact_level = clamp(params.effects.y, 0.0, 1.0);
    let impact_ring = glow(length(uv) - (0.16 + impact_level * 0.92), 24.0);
    color += palette_field(length(uv) * 0.3 + time * 0.04)
        * impact_ring
        * max(impact_bloom, impact_level * drive)
        * 0.44;

    let previous = feedback_frame(screen_uv, time);
    let feedback_decay = 0.82 + params.feedback.x * 0.13;
    let persistent_light = max(color, previous * feedback_decay);
    color = mix(color, persistent_light, params.feedback.x);

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
