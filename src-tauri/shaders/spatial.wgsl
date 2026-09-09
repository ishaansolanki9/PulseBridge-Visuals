// Native spatial library. All distances and lights are evaluated in world space.
// spatial: integrated motion clock, transformation envelope, quality, reserved.
// Fixed bounds and conservative steps keep the fields usable on integrated GPUs.
fn turn_y(p: vec3<f32>, a: f32) -> vec3<f32> {
    let q = rotate2(p.xz, a);
    return vec3<f32>(q.x, p.y, q.y);
}
fn turn_x(p: vec3<f32>, a: f32) -> vec3<f32> {
    let q = rotate2(p.yz, a);
    return vec3<f32>(p.x, q.x, q.y);
}
fn box_distance(p: vec3<f32>, b: vec3<f32>) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}
fn soft_union(a: f32, b: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}
fn spatial_palette(t: f32) -> vec3<f32> {
    // Stable material colors: accents travel slowly, independently of transients.
    let wave = 0.5 + 0.5 * sin(t * TAU + params.spatial.x * 0.12 * params.effects.z * (1.0 + modifier_strength(0u) * 0.5));
    return mix(params.color_b.rgb, params.color_c.rgb, wave) * 0.78 + params.color_d.rgb * 0.22;
}
fn swarm_field(p0: vec3<f32>) -> vec2<f32> {
    let clock = params.spatial.x;
    let burst = params.spatial.y;
    let p = turn_x(turn_y(p0, clock * 0.24), 0.5 + sin(clock * 0.18) * 0.45);
    let angle = atan2(p.z, p.x);
    let ring_count = 40.0;
    let cell_angle = round(angle * ring_count / TAU) * TAU / ring_count;
    let ring_radius = 1.55 + burst * 0.7;
    let ring_wave = sin(cell_angle * 4.0 - clock * 2.0) * params.reactive.x * 0.16;
    let radial = length(p.xz) - ring_radius;
    let tube_angle = atan2(p.y, radial);
    let tube_count = 12.0;
    let tube_cell = round((tube_angle - clock * 0.20 - cell_angle * 2.0) * tube_count / TAU) * TAU / tube_count
        + clock * 0.20 + cell_angle * 2.0;
    let scatter = burst * (0.20 + 0.18 * sin(cell_angle * 11.0 + tube_cell * 3.0));
    let tube_radius = 0.48 + ring_wave + scatter;
    let center = vec3<f32>(cos(cell_angle) * (ring_radius + cos(tube_cell) * tube_radius),
        sin(tube_cell) * tube_radius, sin(cell_angle) * (ring_radius + cos(tube_cell) * tube_radius));
    let bead = 0.027 + params.scene.z * 0.018;
    return vec2<f32>(length(p - center) - bead, fract(cell_angle / TAU + tube_cell / TAU));
}
fn relic_field(p0: vec3<f32>) -> vec2<f32> {
    let clock = params.spatial.x;
    let p = turn_y(turn_x(p0, 0.25 + sin(clock * 0.18) * 0.25), clock * 0.22);
    let q = turn_y(p, p.y * (0.25 + params.reactive.y * 0.58));
    let rounded = length(q * vec3<f32>(0.90, 1.08, 0.90)) - 1.16;
    let angular = (abs(q.x) + abs(q.y) + abs(q.z) - 1.65) * 0.57735;
    let morph = 0.5 + 0.5 * sin(clock * 0.42);
    let ripples = sin(q.x * 4.0 + clock) * sin(q.y * 4.5 - clock * 0.8) * sin(q.z * 4.0)
        * (0.04 + params.reactive.x * 0.13);
    var d = mix(rounded, angular, morph) + ripples;
    let expansion = params.spatial.y;
    // Six satellites detach, orbit, then merge back into the central sculpture.
    for (var i = 0u; i < 6u; i += 1u) {
        let a = f32(i) * TAU / 6.0 + clock * 0.28;
        let c = vec3<f32>(cos(a), sin(a * 2.0) * 0.6, sin(a)) * (0.62 + expansion * 1.7);
        let satellite = length(p - c) - (0.38 + expansion * 0.07);
        d = soft_union(d + expansion * 0.006, satellite, 0.3);
    }
    return vec2<f32>(d, q.y * 0.15 + morph * 0.3);
}
fn architecture_field(p: vec3<f32>) -> vec2<f32> {
    let clock = params.spatial.x;
    let expansion = params.spatial.y;
    let cell = floor((p.z + 2.5) / 5.0);
    let z = p.z - cell * 5.0;
    let span = 2.8 + expansion * 1.4;
    let offset = sin(cell * 0.8 + clock * 0.12) * 0.35;
    let q = vec3<f32>(p.x - offset, p.y, z);
    let pillar = box_distance(vec3<f32>(abs(q.x) - span, q.y - 0.1, q.z), vec3<f32>(0.22, 2.2, 0.48));
    let lintel = box_distance(q - vec3<f32>(0.0, 2.35, 0.0), vec3<f32>(span + 0.22, 0.18, 0.48));
    let slab = box_distance(q - vec3<f32>(sin(cell * 2.1) * 1.6, 0.8 + sin(cell + clock * 0.3) * 0.35, 2.1), vec3<f32>(0.65, 0.10, 0.60));
    let floor_d = p.y + 2.15;
    return vec2<f32>(min(min(pillar, lintel), min(slab, floor_d)), cell * 0.17 + p.y * 0.05);
}
fn kinetic_field(p0: vec3<f32>) -> vec2<f32> {
    let clock = params.spatial.x;
    let p = turn_y(turn_x(p0, 0.25), clock * 0.18);
    var best = vec2<f32>(100.0, 0.0);
    for (var i = 0u; i < 9u; i += 1u) {
        let f = f32(i) - 4.0;
        let a = f * 0.37 + clock * 0.55;
        let center = vec3<f32>(sin(a) * 0.4, f * 0.36, cos(a) * 0.25);
        let q = turn_y(p - center, a + params.reactive.y * 0.4);
        let d = box_distance(q, vec3<f32>(1.15 + params.reactive.x * sin(f - clock * 3.0) * 0.14, 0.075, 0.20)) - 0.055;
        if d < best.x { best = vec2<f32>(d, f * 0.10); }
    }
    return best;
}
fn ocean_height(p: vec2<f32>) -> f32 {
    let clock = params.spatial.x;
    return sin(p.x * 0.7 + clock * 0.35) * 0.38
        + sin(p.y * 0.9 - clock * 0.6) * (0.32 + params.reactive.x * 0.15)
        + sin(p.x * 1.6 + p.y * 0.5 + clock * 0.3) * 0.16;
}
fn spatial_field(id: u32, p: vec3<f32>) -> vec2<f32> {
    switch id {
        case 26u: { return swarm_field(p); }
        case 27u: { return relic_field(p); }
        case 28u: { return architecture_field(p); }
        case 30u: { return kinetic_field(p); }
        default: { return vec2<f32>((p.y + 1.6 - ocean_height(p.xz)) * 0.65, p.y * 0.8); }
    }
}
fn aurora_scene(uv: vec2<f32>) -> vec3<f32> {
    let clock = params.spatial.x;
    var color = params.color_a.rgb * 0.10;
    // Back-to-front translucent curtains at different depths, with an unlit sky.
    for (var i = 0u; i < 6u; i += 1u) {
        let f = f32(i);
        let depth = 1.0 + f * 0.28;
        let x = uv.x * depth + clock * (0.10 + f * 0.007);
        let crest = sin(x * 1.35 + f * 1.1) * 0.26 + sin(x * 2.9 - clock * 0.35) * 0.07;
        let y = uv.y + 0.18 + f * 0.045 - crest;
        let ribbon = exp(-abs(y) * (7.0 + f)) * (0.48 + 0.52 * pow(0.5 + 0.5 * sin(x * 10.0 + f), 3.0));
        let silk = ribbon * (0.24 + params.scene.z * 0.22 + params.reactive.y * 0.15);
        color = color * (1.0 - silk * 0.32) + spatial_palette(f * 0.14 + x * 0.025) * silk * 1.8;
    }
    let star_cell = floor(uv * 160.0);
    let star = step(0.998, hash21(star_cell)) * pow(max(0.0, 1.0 - length(fract(uv * 160.0) - 0.5) * 2.0), 3.0);
    return color + params.color_d.rgb * star * (0.25 + params.reactive.z * 0.35);
}
fn spatial_scene(id: u32, uv: vec2<f32>) -> vec3<f32> {
    if id == 29u { return aurora_scene(uv); }
    let clock = params.spatial.x;
    var ro = vec3<f32>(0.0, 0.2, -6.4);
    if id == 26u { ro.y = 1.25; }
    var look_at = vec3<f32>(0.0);
    var far = 12.0;
    if id == 28u {
        ro = vec3<f32>(sin(clock * 0.13) * 0.8, -0.05, clock * 0.5);
        look_at = ro + vec3<f32>(sin(clock * 0.09) * 0.2, 0.13, 4.0);
        far = 32.0;
    } else if id == 31u {
        ro = vec3<f32>(sin(clock * 0.1) * 0.7, 1.25, clock * 0.45);
        look_at = ro + vec3<f32>(0.0, -0.8, 3.2);
        far = 30.0;
    } else {
        ro = turn_y(ro, sin(clock * 0.16) * 0.35);
    }
    let forward = normalize(look_at - ro);
    let right = normalize(cross(forward, vec3<f32>(0.0, 1.0, 0.0)));
    let up = cross(right, forward);
    let ray = normalize(forward * 1.85 + right * uv.x - up * uv.y);
    let background = params.color_a.rgb * 0.035 + spatial_palette(0.65) * (0.004 + 0.012 * pow(max(0.0, dot(ray, normalize(vec3<f32>(0.4, 0.3, 1.0)))), 16.0));
    var distance = 0.0;
    if id == 31u {
        // Enter the tight height slab first; grazing rays otherwise waste the
        // bounded step budget in the empty sky and leave cracks on ridge edges.
        if ray.y >= -0.002 { return background; }
        distance = max(0.0, (ro.y + 0.50) / -ray.y);
    }
    var hit = false;
    var material = 0.0;
    let steps = u32(mix(48.0, 92.0, params.spatial.z));
    let pixel = 2.0 / max(params.resolution_time.y, 1.0);
    for (var i = 0u; i < 96u; i += 1u) {
        if i >= steps || distance > far { break; }
        let p = ro + ray * distance;
        let sample_value = spatial_field(id, p);
        let epsilon = max(0.002, distance * pixel * 0.6);
        if sample_value.x < epsilon { hit = true; material = sample_value.y; break; }
        // Twists, ripples and repeated beads are estimators, not exact SDFs.
        distance += max(sample_value.x * select(0.62, 1.15, id == 31u), 0.002);
    }
    if !hit && id == 28u && ray.y < -0.001 {
        // The floor has an exact intersection. At low quality, a grazing ray
        // must not punch a hole in it when the object-march budget is exhausted.
        let floor_distance = (-2.15 - ro.y) / ray.y;
        if floor_distance > 0.0 && floor_distance < far {
            distance = floor_distance;
            material = spatial_field(id, ro + ray * distance).y;
            hit = true;
        }
    }
    if !hit && id == 31u && distance < far {
        let residual = spatial_field(id, ro + ray * distance);
        if residual.x < max(0.004, distance * pixel * 1.5) {
            hit = true;
            material = residual.y;
        }
    }
    if !hit { return background; }
    let p = ro + ray * distance;
    let e = max(0.002, distance * pixel * 0.5);
    let n = normalize(vec3<f32>(
        spatial_field(id, p + vec3<f32>(e, 0.0, 0.0)).x - spatial_field(id, p - vec3<f32>(e, 0.0, 0.0)).x,
        spatial_field(id, p + vec3<f32>(0.0, e, 0.0)).x - spatial_field(id, p - vec3<f32>(0.0, e, 0.0)).x,
        spatial_field(id, p + vec3<f32>(0.0, 0.0, e)).x - spatial_field(id, p - vec3<f32>(0.0, 0.0, e)).x));
    let light = normalize(vec3<f32>(-0.5, 0.9, -0.65));
    let diffuse = max(0.0, dot(n, light));
    let rim = pow(1.0 - max(0.0, dot(n, -ray)), 3.0);
    let specular = pow(max(0.0, dot(reflect(-light, n), -ray)), 38.0);
    let base = spatial_palette(material);
    var occlusion = 1.0;
    if params.spatial.z > 0.25 && id != 26u {
        occlusion = clamp(1.0 - (0.18 - spatial_field(id, p + n * 0.18).x) * 2.0, 0.25, 1.0);
    }
    var color = base * (0.08 + diffuse * 1.0) * occlusion + params.color_d.rgb * rim * 0.65 + vec3<f32>(specular) * 0.9;
    if id == 26u {
        color = base * (1.8 + diffuse + params.reactive.z * 0.8) + params.color_d.rgb * specular;
    } else if id == 27u {
        let reflection = reflect(ray, n);
        let light_strip = pow(0.5 + 0.5 * sin(reflection.y * 12.0 + reflection.x * 3.0), 14.0);
        color = base * (0.06 + diffuse * 0.35) * occlusion
            + spatial_palette(reflection.y * 0.25) * light_strip * 1.05
            + params.color_d.rgb * rim * 0.9 + specular * 0.7;
    } else if id == 28u {
        let edge = pow(0.5 + 0.5 * sin(p.z * 1.25), 24.0);
        color = base * (0.12 + diffuse * 0.85) * occlusion + params.color_d.rgb * edge * 0.35;
    } else if id == 31u {
        let contour = 0.5 + 0.5 * sin(ocean_height(p.xz) * 23.0);
        // Analytic footprint attenuates unresolved ridges without derivatives in a divergent ray loop.
        let visibility = 1.0 - smoothstep(0.03, 0.22, distance * pixel);
        color = base * (0.15 + diffuse * 0.9) + params.color_d.rgb * pow(contour, 12.0) * visibility * 0.55 + specular * 0.5;
    }
    let fog = 1.0 - exp(-distance * select(0.016, 0.055, id == 28u || id == 31u));
    return mix(color, background, fog);
}
