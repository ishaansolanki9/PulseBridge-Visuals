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

fn field_noise(point: vec2<f32>) -> f32 {
    let cell = floor(point);
    let local = fract(point);
    let smooth_local = local * local * (3.0 - 2.0 * local);
    return mix(
        mix(hash21(cell), hash21(cell + vec2<f32>(1.0, 0.0)), smooth_local.x),
        mix(
            hash21(cell + vec2<f32>(0.0, 1.0)),
            hash21(cell + vec2<f32>(1.0, 1.0)),
            smooth_local.x,
        ),
        smooth_local.y,
    );
}

fn field_fbm(input_point: vec2<f32>) -> f32 {
    var point = input_point;
    var value = 0.0;
    var amplitude = 0.5;
    for (var octave = 0; octave < 3; octave += 1) {
        value += field_noise(point) * amplitude;
        point = mat2x2<f32>(1.62, 1.18, -1.18, 1.62) * point + 0.17;
        amplitude *= 0.5;
    }
    return value;
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
    let palette_drift = modifier_strength(0u);
    let drift = params.resolution_time.z
        * (0.004 + params.effects.z * (0.006 + palette_drift * 0.02));
    let amount = 0.5 + 0.5 * sin((value + drift) * TAU);
    return mix(params.color_b.rgb, params.color_c.rgb, amount);
}

fn accent_field(value: f32) -> vec3<f32> {
    let amount = 0.5 + 0.5 * sin(value * TAU + params.resolution_time.z * 0.018);
    return mix(params.color_c.rgb, params.color_d.rgb, amount);
}

fn phase_time(time: f32, rate: f32) -> f32 {
    let motion = clamp(params.visual.x, 0.2, 1.8);
    return time * rate * (0.55 + motion * 0.45);
}

fn line_glow(distance: f32, width: f32) -> f32 {
    let antialias = max(fwidth(distance) * 1.4, 0.0015);
    return 1.0 - smoothstep(width, width + antialias, abs(distance));
}

fn soft_line(distance: f32, sharpness: f32) -> f32 {
    return exp(-abs(distance) * sharpness);
}

fn ridge(value: f32, sharpness: f32) -> f32 {
    let raw = pow(max(0.0, 1.0 - abs(sin(value))), sharpness);
    return raw * (1.0 - smoothstep(0.72, 1.9, fwidth(value)));
}

fn sd_segment(point: vec2<f32>, start: vec2<f32>, end: vec2<f32>) -> f32 {
    let to_point = point - start;
    let segment = end - start;
    let position = clamp(dot(to_point, segment) / max(dot(segment, segment), 0.0001), 0.0, 1.0);
    return length(to_point - segment * position);
}

fn soft_spot(point: vec2<f32>, center: vec2<f32>, radius: f32) -> f32 {
    let scaled = (point - center) / vec2<f32>(radius * 1.35, radius);
    return exp(-dot(scaled, scaled) * 2.6);
}

fn structure_opening() -> f32 {
    return 1.0
        + params.music.y * 0.16
        + params.reactive.x * 0.1
        + params.reactive.w * 0.12
        + params.pulse.y * 0.045
        + modifier_strength(1u) * params.pulse.y * 0.08;
}

fn wave_center(x: f32, time: f32, offset: f32) -> f32 {
    let travel = phase_time(time, 0.28) + offset;
    let amplitude = 0.105
        + params.music.y * 0.16
        + params.reactive.x * 0.075
        + params.reactive.w * 0.055;
    return sin(x * 2.45 + travel) * amplitude
        + sin(x * 5.1 - travel * 0.63 + offset * 1.7) * (0.026 + params.music.z * 0.035);
}

fn color_splotch_wave(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let shake = sin(params.pulse.x * TAU) * params.pulse.y * 0.038
        + sin(time * 7.0) * params.effects.y * 0.026
        + sin(time * 3.7) * params.reactive.x * 0.022;
    let point = uv + vec2<f32>(0.0, shake);
    let center = wave_center(point.x, time, 0.0);
    let distance = point.y - center;
    let width = 0.009 + params.scene.z * 0.005;
    let main_trace = line_glow(distance, width);
    let body = soft_line(distance, 8.5) * (1.0 - smoothstep(0.18, 0.42, abs(distance)));
    let echo_spacing = 0.07 + params.music.y * 0.045;
    let echo_gate = smoothstep(0.22, 0.62, params.scene.z);
    let upper = line_glow(distance - echo_spacing, width * 0.72) * echo_gate;
    let lower = line_glow(distance + echo_spacing, width * 0.72) * echo_gate;
    var color = palette_field(point.x * 0.08)
        * (main_trace * 0.9 + body * 0.12 + (upper + lower) * 0.22);

    let x_a = -0.78 + sin(time * 0.11) * 0.09;
    let x_b = -0.24 + sin(time * 0.09 + 1.8) * 0.08;
    let x_c = 0.33 + sin(time * 0.1 + 3.1) * 0.1;
    let x_d = 0.86 + sin(time * 0.08 + 4.4) * 0.07;
    let spot_a = soft_spot(point, vec2<f32>(x_a, wave_center(x_a, time, 0.0)), 0.055);
    let spot_b = soft_spot(point, vec2<f32>(x_b, wave_center(x_b, time, 0.0)), 0.045);
    let spot_c = soft_spot(point, vec2<f32>(x_c, wave_center(x_c, time, 0.0)), 0.065);
    let spot_d = soft_spot(point, vec2<f32>(x_d, wave_center(x_d, time, 0.0)), 0.04);
    let embedded = 1.0 - smoothstep(0.025, 0.16, abs(distance));
    let high_accent = 0.52 + params.music.w * 0.32 + params.reactive.z * 0.22;
    color += params.color_d.rgb * (spot_a + spot_c) * embedded * high_accent;
    color += params.color_c.rgb * (spot_b + spot_d) * embedded * high_accent * 0.78;
    return color;
}

fn multi_layer_wave_field(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let travel = phase_time(time, 0.24);
    let layer_count = 3.0 + floor(params.scene.z * 5.0);
    let spacing = 0.16 + (1.0 - params.music.x) * 0.018;
    let amplitude = 0.045 + params.music.y * 0.09 + params.reactive.x * 0.045;
    var color = vec3<f32>(0.0);
    for (var layer = 0; layer < 8; layer += 1) {
        let index = f32(layer);
        let visibility = 1.0 - smoothstep(layer_count - 0.15, layer_count + 0.15, index);
        let centered = index - (layer_count - 1.0) * 0.5;
        let phase = travel + centered * 0.52;
        let wave = centered * spacing
            + sin(uv.x * (2.2 + index * 0.12) + phase) * amplitude
            + sin(uv.x * 4.6 - phase * 0.7) * params.music.z * 0.025;
        let trace = line_glow(uv.y - wave, 0.006 + params.scene.w * 0.003);
        let breath = 0.4 + 0.6 * sin(params.pulse.x * TAU + index * 0.58) * sin(params.pulse.x * TAU + index * 0.58);
        let strength = visibility * (0.28 + params.music.x * 0.28 + breath * params.pulse.y * 0.08);
        color += palette_field(index * 0.085 + uv.x * 0.025) * trace * strength;
    }
    let envelope = 1.0 - smoothstep(0.72, 1.18, abs(uv.y));
    return color * envelope;
}

fn fractal_bloom(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let opening = structure_opening();
    let point = rotate2(uv / opening, phase_time(time, 0.035));
    let radius = length(point);
    let angle = atan2(point.y, point.x);
    let visible_levels = 2.0 + floor(params.scene.z * 3.0);
    var color = vec3<f32>(0.0);
    for (var level = 0; level < 5; level += 1) {
        let index = f32(level);
        let visibility = 1.0 - smoothstep(visible_levels - 0.15, visible_levels + 0.15, index);
        let petals = 6.0 + index * 4.0;
        let base_radius = 0.19 + index * 0.155;
        let unfold = sin(angle * petals + phase_time(time, 0.12) * (1.0 - index * 0.08));
        let target_radius = base_radius + unfold * (0.022 + index * 0.008 + params.music.z * 0.015);
        let trace = line_glow(radius - target_radius, 0.0065 + params.scene.w * 0.0025);
        let hierarchy = 0.56 / (1.0 + index * 0.17);
        color += palette_field(index * 0.11 + angle / TAU) * trace * visibility * hierarchy;
    }
    let core = soft_line(radius - (0.075 + params.pulse.y * 0.025), 42.0);
    let negative_space = smoothstep(0.045, 0.12, radius);
    color += params.color_d.rgb * core * (0.12 + params.pulse.y * 0.22);
    return color * negative_space * (1.0 - smoothstep(0.88, 1.3, radius));
}

fn recursive_tunnel(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let opening = structure_opening();
    let point = rotate2(uv / opening, phase_time(time, 0.055));
    let travel = fract(phase_time(time, 0.05) + params.pulse.x * 0.06);
    let circle_radius = length(point);
    let diamond_radius = (abs(point.x) + abs(point.y)) * 0.72;
    var color = vec3<f32>(0.0);
    for (var level = 0; level < 7; level += 1) {
        let index = f32(level);
        let depth = fract(index / 7.0 + travel);
        let shape = mix(circle_radius, diamond_radius, 0.32 + 0.18 * sin(index * 1.7));
        let target_radius = 0.11 + depth * 1.02;
        let trace = line_glow(shape - target_radius, 0.006 + depth * 0.004);
        let fade = smoothstep(0.0, 0.12, depth) * (1.0 - smoothstep(0.72, 1.0, depth));
        color += palette_field(depth * 0.34 + index * 0.07) * trace * fade * 0.62;
    }
    let center = soft_line(circle_radius, 24.0) * (0.08 + params.pulse.y * 0.2);
    return color + params.color_d.rgb * center;
}

fn ribbon_flow(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let travel = phase_time(time, 0.2);
    let bend = sin(uv.x * 1.85 + travel) * (0.14 + params.music.y * 0.12)
        + sin(uv.x * 4.2 - travel * 0.54) * (0.035 + params.music.z * 0.03);
    let half_width = 0.075 + params.music.y * 0.055 + params.reactive.x * 0.035;
    let distance = uv.y - bend;
    let top = line_glow(distance - half_width, 0.008);
    let bottom = line_glow(distance + half_width, 0.008);
    let fill = 1.0 - smoothstep(half_width * 0.58, half_width, abs(distance));
    var color = palette_field(uv.x * 0.06 + distance * 0.2) * ((top + bottom) * 0.62 + fill * 0.11);
    let second_gate = smoothstep(0.48, 0.78, params.scene.z);
    let second_center = bend * -0.48 + 0.38;
    let second = line_glow(uv.y - second_center, 0.006) * second_gate;
    color += accent_field(uv.x * 0.05) * second * 0.34;
    return color * (1.0 - smoothstep(1.0, 1.5, abs(uv.x)));
}

fn branching_tree(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let sway = sin(phase_time(time, 0.11)) * (0.035 + params.music.z * 0.045);
    let opening = structure_opening();
    let point = vec2<f32>(uv.x / opening, uv.y);
    let root = vec2<f32>(0.0, -0.92);
    let fork = vec2<f32>(sway * 0.2, -0.18);
    var distance = sd_segment(point, root, fork);
    var detail_distance = 10.0;
    for (var side_index = 0; side_index < 2; side_index += 1) {
        let side = f32(side_index) * 2.0 - 1.0;
        let first = vec2<f32>(side * 0.3 + sway, 0.16);
        let outer = vec2<f32>(side * 0.53 + sway * 1.4, 0.49);
        let inner = vec2<f32>(side * 0.14 + sway * 0.7, 0.55);
        let crown_outer = vec2<f32>(side * 0.7 + sway * 1.8, 0.78);
        let crown_inner = vec2<f32>(side * 0.4 + sway, 0.84);
        distance = min(distance, sd_segment(point, fork, first));
        distance = min(distance, sd_segment(point, first, outer));
        distance = min(distance, sd_segment(point, first, inner));
        detail_distance = min(detail_distance, sd_segment(point, outer, crown_outer));
        detail_distance = min(detail_distance, sd_segment(point, outer, crown_inner));
    }
    let trunk = line_glow(distance, 0.009 + params.music.y * 0.004);
    let detail = line_glow(detail_distance, 0.006) * smoothstep(0.3, 0.62, params.scene.z);
    let pulse_height = -0.88 + fract(params.pulse.x + phase_time(time, 0.03)) * 1.68;
    let traveling_pulse = exp(-abs(point.y - pulse_height) * 18.0) * soft_line(distance, 22.0);
    return palette_field(point.y * 0.1) * (trunk * 0.62 + detail * 0.42)
        + params.color_d.rgb * traveling_pulse * (0.14 + params.pulse.y * 0.35);
}

fn contour_field(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let travel = phase_time(time, 0.09);
    let shifted = uv + vec2<f32>(sin(travel) * 0.09, cos(travel * 0.7) * 0.07);
    let broad = sin(shifted.x * 2.1 + travel) * 0.42
        + cos(shifted.y * 2.6 - travel * 0.72) * 0.36;
    let organic = field_fbm(shifted * 1.65 + vec2<f32>(travel * 0.08, -travel * 0.05)) - 0.44;
    let focus = length(shifted - vec2<f32>(sin(travel * 0.5) * 0.24, 0.0));
    let ripple = sin(focus * 7.0 - params.pulse.x * TAU) * params.reactive.x * 0.14;
    let field = broad + organic * (0.52 + params.music.z * 0.18) + ripple;
    let frequency = 5.0 + floor(params.scene.z * 5.0);
    let contours = ridge(field * frequency, 12.0);
    let fade = 1.0 - smoothstep(1.12, 1.65, length(uv * vec2<f32>(0.72, 1.0)));
    return palette_field(field * 0.08) * contours * (0.34 + params.music.z * 0.25) * fade;
}

fn lattice_flow(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let travel = phase_time(time, 0.12);
    let bend_x = sin(uv.y * 2.6 + travel) * (0.07 + params.music.z * 0.08);
    let bend_y = sin(uv.x * 2.2 - travel * 0.76) * (0.06 + params.music.z * 0.07);
    let point = uv + vec2<f32>(bend_x, bend_y);
    let spacing = 0.22 - params.scene.z * 0.035;
    let local_x = (fract(point.x / spacing + 0.5) - 0.5) * spacing;
    let local_y = (fract(point.y / spacing + 0.5) - 0.5) * spacing;
    let vertical = line_glow(local_x, 0.0055);
    let horizontal = line_glow(local_y, 0.0055);
    let intersection = vertical * horizontal;
    let fade = 1.0 - smoothstep(0.72, 1.42, length(uv * vec2<f32>(0.7, 1.0)));
    return palette_field(point.x * 0.04 + point.y * 0.05)
        * (max(vertical, horizontal) * 0.32 + intersection * (0.12 + params.pulse.y * 0.24))
        * fade;
}

fn helix_spiral(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let travel = phase_time(time, 0.22);
    let amplitude = 0.21 + params.music.y * 0.11 + params.reactive.x * 0.055;
    let phase = uv.x * (4.2 + params.scene.z * 1.2) - travel;
    let strand_a_y = sin(phase) * amplitude;
    let strand_b_y = -strand_a_y;
    let depth_a = 0.5 + 0.5 * cos(phase);
    let depth_b = 1.0 - depth_a;
    let strand_a = line_glow(uv.y - strand_a_y, 0.008) * (0.25 + depth_a * 0.55);
    let strand_b = line_glow(uv.y - strand_b_y, 0.008) * (0.25 + depth_b * 0.55);
    let rung_phase = ridge(uv.x * 11.0 - travel * 0.55, 14.0);
    let between = 1.0 - smoothstep(amplitude * 0.72, amplitude + 0.025, abs(uv.y));
    let rungs = rung_phase * between * (0.08 + params.scene.z * 0.18);
    let horizontal_fade = 1.0 - smoothstep(1.02, 1.52, abs(uv.x));
    return (palette_field(uv.x * 0.06) * (strand_a + strand_b)
        + accent_field(uv.x * 0.04) * rungs)
        * horizontal_fade;
}

fn ring_pulse_system(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let opening = structure_opening();
    let point = uv / opening;
    let radius = length(point);
    var color = vec3<f32>(0.0);
    for (var ring_index = 0; ring_index < 4; ring_index += 1) {
        let index = f32(ring_index);
        let ring_radius = 0.2 + index * 0.18 + sin(phase_time(time, 0.11) + index * 0.8) * 0.018;
        let trace = line_glow(radius - ring_radius, 0.007 + index * 0.001);
        let visibility = 1.0 - smoothstep(2.0 + params.scene.z * 2.0, 2.4 + params.scene.z * 2.0, index);
        color += palette_field(index * 0.13) * trace * visibility * (0.38 + index * 0.035);
    }
    let emitted_radius = 0.08 + fract(params.pulse.x + phase_time(time, 0.025)) * 0.86;
    let emitted = line_glow(radius - emitted_radius, 0.01) * (0.08 + params.pulse.y * 0.55);
    return color + params.color_d.rgb * emitted;
}

fn arc_fan(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let origin = vec2<f32>(0.0, -0.94);
    let point = uv - origin;
    let radius = length(point);
    let angle = atan2(point.x, point.y);
    let fan_gate = 1.0 - smoothstep(0.72, 1.03, abs(angle));
    let travel = phase_time(time, 0.12);
    let arc_frequency = 8.0 + floor(params.scene.z * 4.0);
    let curved_radius = radius + sin(angle * 3.0 + travel) * (0.035 + params.music.z * 0.035);
    let arcs = ridge(curved_radius * arc_frequency - travel * 0.65, 13.0);
    let edge = soft_line(abs(angle) - (0.68 + params.music.y * 0.08), 36.0);
    let pulse_arc = line_glow(radius - (0.22 + fract(params.pulse.x) * 1.05), 0.012)
        * params.pulse.y;
    return palette_field(angle * 0.08 + radius * 0.04)
        * (arcs * 0.44 + edge * 0.16 + pulse_arc * 0.3)
        * fan_gate
        * (1.0 - smoothstep(1.45, 1.9, radius));
}

fn fractal_wave_hybrid(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let travel = phase_time(time, 0.2);
    let base_amplitude = 0.11 + params.music.y * 0.13 + params.reactive.x * 0.05;
    var color = vec3<f32>(0.0);
    var amplitude = base_amplitude;
    var frequency = 2.0;
    for (var level = 0; level < 5; level += 1) {
        let index = f32(level);
        let visible = 1.0 - smoothstep(2.0 + params.scene.z * 3.0, 2.35 + params.scene.z * 3.0, index);
        let offset = (index - 2.0) * (0.075 + params.scene.z * 0.012);
        let wave = sin(uv.x * frequency + travel * (1.0 - index * 0.09)) * amplitude
            + sin(uv.x * frequency * 0.5 - travel * 0.4) * amplitude * 0.24;
        let trace = line_glow(uv.y - offset - wave, 0.006 + index * 0.0007);
        color += palette_field(index * 0.1 + uv.x * 0.025) * trace * visible * (0.46 / (1.0 + index * 0.18));
        amplitude *= 0.56;
        frequency *= 1.72;
    }
    return color * (1.0 - smoothstep(0.78, 1.15, abs(uv.y)));
}

fn visual_family(id: u32, uv: vec2<f32>, time: f32) -> vec3<f32> {
    switch id {
        case 0u: { return color_splotch_wave(uv, time); }
        case 1u: { return multi_layer_wave_field(uv, time); }
        case 2u: { return fractal_bloom(uv, time); }
        case 3u: { return recursive_tunnel(uv, time); }
        case 4u: { return ribbon_flow(uv, time); }
        case 5u: { return branching_tree(uv, time); }
        case 6u: { return contour_field(uv, time); }
        case 7u: { return lattice_flow(uv, time); }
        case 8u: { return helix_spiral(uv, time); }
        case 9u: { return ring_pulse_system(uv, time); }
        case 10u: { return arc_fan(uv, time); }
        case 11u: { return fractal_wave_hybrid(uv, time); }
        default: { return color_splotch_wave(uv, time); }
    }
}

fn feedback_frame(screen_uv: vec2<f32>, time: f32) -> vec3<f32> {
    let resolution = max(params.resolution_time.xy, vec2<f32>(1.0));
    let aspect = resolution.x / resolution.y;
    var point = screen_uv * 2.0 - 1.0;
    point.x *= aspect;
    point = rotate2(point, params.feedback.z * sin(time * 0.09));
    point *= 1.0 - params.feedback.y * (1.0 + params.reactive.x * 0.5);
    point += vec2<f32>(
        sin(point.y * 2.8 + time * 0.15),
        cos(point.x * 2.5 - time * 0.13),
    ) * params.feedback.z;
    point.x /= aspect;
    let sample_uv = point * 0.5 + 0.5;
    let edge = smoothstep(0.0, 0.025, sample_uv.x)
        * (1.0 - smoothstep(0.975, 1.0, sample_uv.x))
        * smoothstep(0.0, 0.025, sample_uv.y)
        * (1.0 - smoothstep(0.975, 1.0, sample_uv.y));
    return textureSample(feedback_texture, feedback_sampler, sample_uv).rgb * edge;
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let resolution = max(params.resolution_time.xy, vec2<f32>(1.0));
    let screen_uv = position.xy / resolution;
    let stable_uv = (position.xy * 2.0 - resolution) / resolution.y;
    let time = params.resolution_time.z;
    let beat_zoom = modifier_strength(1u);
    let scale = 1.0
        - params.pulse.y * (0.012 + beat_zoom * 0.045)
        - params.reactive.x * 0.022
        - params.reactive.w * 0.014;
    let uv = stable_uv * scale;

    let primary_id = u32(round(params.style_a.x));
    let secondary_id = u32(round(params.style_a.y));
    var color = visual_family(primary_id, uv, time) * params.style_a.z;
    if params.style_a.w > 0.001 {
        color += visual_family(secondary_id, uv, time) * params.style_a.w;
    }

    let vignette = 1.0 - smoothstep(
        0.82,
        1.62,
        length(stable_uv * vec2<f32>(0.68, 1.0)),
    );
    color *= (0.58 + vignette * 0.42)
        * params.visual.w
        * (0.9 + params.music.x * 0.12 + params.reactive.w * 0.15);

    let impact_bloom = modifier_strength(7u);
    color *= 1.0 + max(params.effects.y * 0.08, impact_bloom * params.pulse.y * 0.18);

    let echo_trails = modifier_strength(4u);
    let feedback_mix = params.feedback.x * (0.16 + echo_trails * 0.72);
    let previous = feedback_frame(screen_uv, time) * 0.92;
    color = mix(color, max(color, previous), feedback_mix);

    color = mix(color, vec3<f32>(1.0), params.pulse.w * 0.58);
    let luminance = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    let luminance_budget = 0.56 + params.scene.w * 0.3;
    if luminance > luminance_budget {
        color *= luminance_budget / max(luminance, 0.001);
    }
    color = mix(
        vec3<f32>(dot(color, vec3<f32>(0.2126, 0.7152, 0.0722))),
        color,
        params.style_b.x,
    );
    color = 1.0 - exp(-max(color, vec3<f32>(0.0)) * 1.24);
    color = pow(max(color, vec3<f32>(0.0)), vec3<f32>(0.95));
    if params.style_b.w > 0.5 {
        color = vec3<f32>(0.0);
    }
    return vec4<f32>(color, 1.0);
}
