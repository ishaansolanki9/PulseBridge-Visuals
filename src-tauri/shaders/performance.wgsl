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
};

@group(0) @binding(0) var<uniform> params: VisualParams;

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

fn noise(point: vec2<f32>) -> f32 {
    let cell = floor(point);
    let local = fract(point);
    let smooth_local = local * local * (3.0 - 2.0 * local);
    return mix(
        mix(hash21(cell), hash21(cell + vec2<f32>(1.0, 0.0)), smooth_local.x),
        mix(hash21(cell + vec2<f32>(0.0, 1.0)), hash21(cell + vec2<f32>(1.0, 1.0)), smooth_local.x),
        smooth_local.y,
    );
}

fn fbm(input_point: vec2<f32>) -> f32 {
    var point = input_point;
    var value = 0.0;
    var amplitude = 0.5;
    for (var octave = 0; octave < 4; octave += 1) {
        value += noise(point) * amplitude;
        point = mat2x2<f32>(1.62, 1.18, -1.18, 1.62) * point + 0.17;
        amplitude *= 0.5;
    }
    return value;
}

fn palette_field(value: f32) -> vec3<f32> {
    let wrapped = fract(value);
    if wrapped < 0.3333 {
        return mix(params.color_a.rgb, params.color_b.rgb, smoothstep(0.0, 0.3333, wrapped));
    }
    if wrapped < 0.6666 {
        return mix(params.color_b.rgb, params.color_c.rgb, smoothstep(0.3333, 0.6666, wrapped));
    }
    return mix(params.color_c.rgb, params.color_d.rgb, smoothstep(0.6666, 1.0, wrapped));
}

fn fluid_visual(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let motion = params.visual.x;
    let turbulence = params.visual.y;
    let bass = params.music.y;
    let warp_a = fbm(uv * (1.25 + turbulence * 0.45) + vec2<f32>(time * 0.075, -time * 0.052));
    let warp_b = fbm(uv * 1.7 + vec2<f32>(-time * 0.043, time * 0.064) + warp_a * 1.8);
    let warped = uv + vec2<f32>(warp_a - 0.5, warp_b - 0.5) * (0.42 + bass * 0.48);
    let field = fbm(warped * (1.18 + turbulence * 0.5) + time * motion * 0.08);
    let color = palette_field(field + time * 0.018 * params.effects.z + params.pulse.y * 0.06);
    let glow = smoothstep(0.22, 0.9, field) * (0.48 + params.music.x * 0.58);
    return color * (0.28 + glow);
}

fn tunnel_visual(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let radius = max(length(uv), 0.025);
    let angle = atan2(uv.y, uv.x);
    let depth = 1.0 / radius;
    let speed = 0.18 + params.visual.x * 0.42 + params.music.x * 0.35;
    let spiral = angle / 6.28318 + depth * 0.34 - time * speed;
    let rings = pow(1.0 - abs(sin(spiral * 12.0 + params.music.y * 1.8)), 5.5);
    let spokes = pow(1.0 - abs(sin(angle * 4.0 + time * 0.16)), 10.0) * 0.18;
    let color = palette_field(spiral * 0.16 + time * 0.012 * params.effects.z);
    let center = smoothstep(0.02, 0.42, radius);
    return color * (0.08 + rings * (0.46 + params.music.x * 0.72) + spokes) * center;
}

fn burst_visual(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let radius = length(uv);
    let angle = atan2(uv.y, uv.x);
    let rays = pow(max(0.0, sin(angle * 9.0 + fbm(uv * 2.1) * 4.0)), 6.0);
    let impact_front = exp(-abs(radius - (0.14 + params.effects.y * 0.95)) * 14.0);
    let bass_front = exp(-abs(radius - fract(time * 0.08 + params.pulse.y * 0.22)) * 8.0);
    let center = exp(-radius * (2.8 - params.music.y * 0.9));
    let color = mix(
        palette_field(angle / 6.28318 + time * 0.01 * params.effects.z + 0.5),
        palette_field(radius * 0.4 + params.effects.y * 0.2),
        center,
    );
    return color * (0.08 + center * 0.28 + rays * 0.24 + impact_front * (0.4 + params.effects.y) + bass_front * params.pulse.y * 0.2);
}

fn waves_visual(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let energy = params.music.x;
    let bass = params.music.y;
    let density = 3.0 + params.music.w * 4.2 + energy * 1.8;
    let moving_uv = uv * params.visual.z;
    let bend = sin(moving_uv.x * 2.1 + time * 0.22) * (0.18 + bass * 0.3);
    let wave = sin((moving_uv.y + bend) * density - time * (0.55 + params.visual.x * 1.1));
    let second = sin((moving_uv.x * 0.72 - moving_uv.y * 0.84) * (density * 0.72) + time * 0.72);
    let band = pow(1.0 - abs(wave), 3.2) + pow(1.0 - abs(second), 5.0) * 0.58;
    let sweep = sin(length(moving_uv) * 7.0 - time * 1.3 - params.pulse.y * 1.4) * 0.5 + 0.5;
    let color = mix(
        palette_field(moving_uv.x * 0.16 + time * 0.026),
        palette_field(moving_uv.y * 0.2 + 0.45),
        sweep,
    );
    return color * (0.16 + band * (0.62 + energy * 0.58));
}

fn pulse_visual(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let radius = length(uv);
    let angle = atan2(uv.y, uv.x);
    let beat = params.pulse.y;
    let bass = params.music.y;
    let expansion = radius * (2.4 - beat * 0.22 - bass * 0.18);
    let rings = pow(1.0 - abs(sin(expansion * 5.0 - time * (0.65 + params.visual.x))), 5.0);
    let core = exp(-radius * (2.3 - bass * 0.62));
    let petals = sin(angle * 4.0 + time * 0.24 + radius * 4.0) * 0.5 + 0.5;
    let color = mix(
        palette_field(radius * 0.38 - time * 0.018),
        palette_field(angle / 6.28318 + time * 0.02 + 0.5),
        petals * 0.56,
    );
    return color * (0.12 + core * (0.48 + beat * 0.5) + rings * (0.24 + params.music.x * 0.55));
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let resolution = max(params.resolution_time.xy, vec2<f32>(1.0));
    var uv = (position.xy * 2.0 - resolution) / resolution.y;
    let time = params.resolution_time.z;
    let pulse_scale = 1.0 - params.pulse.y * 0.035 - params.pulse.w * 0.12;
    uv *= pulse_scale;

    let fluid = fluid_visual(uv, time);
    let waves = waves_visual(uv, time);
    let pulse = pulse_visual(uv, time);
    let tunnel = tunnel_visual(uv, time);
    let burst = burst_visual(uv, time);
    var color = fluid * params.style_a.x
        + waves * params.style_a.y
        + pulse * params.style_a.z
        + tunnel * params.style_a.w
        + burst * params.style_b.x;

    let vignette = smoothstep(1.38, 0.25, length(uv * vec2<f32>(0.72, 1.0)));
    color *= 0.38 + vignette * 0.78;
    color *= params.visual.w * (0.86 + params.pulse.y * 0.18);
    color += palette_field(time * 0.035 * params.effects.z) * params.pulse.z * 0.08;
    color += palette_field(time * 0.014) * params.effects.x * 0.035;
    color = mix(color, vec3<f32>(1.0), params.pulse.w * 0.72);
    let luminance = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    color = mix(vec3<f32>(luminance), color, params.style_b.y);
    color = 1.0 - exp(-color * 1.28);
    color = pow(max(color, vec3<f32>(0.0)), vec3<f32>(0.92));
    if params.style_b.w > 0.5 {
        color = vec3<f32>(0.0);
    }
    return vec4<f32>(color, 1.0);
}
