// Shared with the ambient preview via scripts/sync-tron-shader.py.
// Analytic distance fields: bounded work, filtered edges, no textures or particles.
// Position-delayed audio keeps a hit traveling after its live envelope has fallen.
fn tron_signal(age: f32) -> vec4<f32> {
    return clamp(recent_signal(age) * 1.15 + params.reactive * 0.25, vec4<f32>(0.0), vec4<f32>(1.0));
}

fn tron_glow(distance: f32, width: f32) -> f32 {
    let aa: f32 = max(fwidth(distance), 0.0012);
    return (1.0 - smoothstep(width, width + aa, abs(distance)))
        + 0.32 * exp(-abs(distance) / (width * 4.0 + aa));
}

fn tron_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let v: vec2<f32> = b - a;
    return length(p - a - v * clamp(dot(p - a, v) / max(dot(v, v), 0.0001), 0.0, 1.0));
}

fn tron_look(p: vec2<f32>, look: u32, t: f32) -> vec3<f32> {
    let bass: f32 = params.reactive.x;
    let mids: f32 = params.reactive.y;
    let highs: f32 = params.reactive.z;
    let cyan: vec3<f32> = vec3<f32>(0.015, 0.78, 1.0);
    let orange: vec3<f32> = vec3<f32>(1.0, 0.24, 0.025);
    let blue: vec3<f32> = vec3<f32>(0.08, 0.18, 0.95);
    var light: vec3<f32> = vec3<f32>(0.001, 0.004, 0.012);
    let r: f32 = length(p);
    let tick: f32 = t * 1.6;
    if look == 0u || look == 1u || look == 8u {
        // Grid highway, light-cycle tracks, and an elevated neon city.
        let road_signal: vec4<f32> = tron_signal(clamp((1.0 - p.y) * 0.52, 0.0, 0.95));
        let lift: f32 = road_signal.x * sin(p.y * 5.0 - t * 0.5) * 0.19;
        let horizon: f32 = p.y + 0.18 + lift;
        let depth: f32 = 1.0 / max(abs(horizon), 0.035);
        let steering: f32 = road_signal.y * sin(depth * 0.24 + t * 0.35) * 1.7;
        let world: vec2<f32> = vec2<f32>(p.x * depth + steering, depth + tick + road_signal.x * 2.6);
        let fracture: f32 = sin(floor(world.y * 0.5) * 2.1) * road_signal.z * 1.2;
        let cell: vec2<f32> = abs(fract((world + vec2<f32>(fracture, 0.0)) * 0.5) - 0.5);
        let grid: f32 = tron_glow(min(cell.x, cell.y), 0.012);
        let fade: f32 = smoothstep(0.035, 0.25, horizon) / (1.0 + depth * 0.035);
        light += cyan * grid * fade * (0.55 + bass * 0.6);
        light += blue * exp(-abs(horizon) * 20.0) * 0.35;
        if look == 0u {
            let sun_center: vec2<f32> = vec2<f32>(sin(t * 0.3) * mids * 0.3, -0.43);
            let sun: f32 = tron_glow(length(p - sun_center) - 0.22, 0.009);
            light += orange * sun;
            let wake: f32 = tron_glow(road_signal.x - 0.45, 0.05) * road_signal.x;
            light += cyan * wake * fade * 0.55;
        }
        if look == 1u {
            for (var i: u32 = 0u; i < 6u; i += 1u) {
                let fi: f32 = f32(i);
                let rider: vec4<f32> = tron_signal(fi * 0.12);
                let lane: f32 = (fi - 2.5) * 1.65 + sin(t * 0.3 + fi) * 0.35
                    + sign(sin(fi * 7.0)) * smoothstep(0.15, 0.75, rider.y) * 1.8
                    + rider.z * sin(fi * 2.5) * 0.9;
                let head: f32 = fract(world.y * 0.08 - t * 0.2 + fi * 0.17 + rider.x * 0.24);
                let trail: f32 = tron_glow(world.x - lane, 0.028) * (0.18 + 0.82 * pow(head, 3.0));
                let bike: f32 = tron_glow(world.x - lane, 0.1) * exp(-pow((head - 0.94) * 35.0, 2.0));
                light += mix(cyan, orange, f32(i % 2u)) * (trail + bike * (0.8 + rider.z)) * fade;
            }
        }
        if look == 8u {
            for (var i: u32 = 0u; i < 18u; i += 1u) {
                let fi: f32 = f32(i);
                let tower: vec4<f32> = tron_signal(abs(fi - 8.5) * 0.1);
                let x: f32 = (fi - 8.5) * 0.19 + tower.y * sin(fi * 1.7) * 0.1;
                let height: f32 = 0.12 + 0.5 * fract(sin(fi * 32.7) * 437.1) + tower.x * 0.38;
                let tower_p: vec2<f32> = p + vec2<f32>(sin(p.y * 8.0 + fi) * tower.y * 0.035, tower.z * sin(fi * 5.0) * 0.1);
                let box: vec2<f32> = abs(tower_p - vec2<f32>(x, -0.18 - height * 0.5)) - vec2<f32>(0.066, height * 0.5);
                let edge: f32 = abs(max(box.x, box.y));
                light += mix(cyan, orange, f32(i % 3u) * 0.35) * tron_glow(edge, 0.003) * 0.75;
            }
        }
    } else if look == 2u || look == 3u || look == 9u {
        // Rectangular gates, hexagonal corridor, and a twisting laser helix.
        for (var i: u32 = 0u; i < 18u; i += 1u) {
            let fi: f32 = f32(i);
            let z: f32 = fract((fi - t * 1.1) / 18.0);
            let gate: vec4<f32> = tron_signal((1.0 - z) * 0.92);
            let scale: f32 = 0.08 + z * z * 2.5 + gate.x * z * 0.65;
            let rotation: f32 = gate.y * sin(fi * 0.4 + t) * 0.85;
            let center: vec2<f32> = vec2<f32>(sin(z * 5.0 + t * 0.25), cos(z * 4.0)) * gate.y * z * 0.45;
            var q: vec2<f32> = rotate2(p - center, rotation);
            q += vec2<f32>(sign(q.y), sign(q.x)) * gate.z * z * 0.13;
            var d: f32 = max(abs(q.x) * 0.72, abs(q.y)) - scale;
            if look == 3u {
                d = max(abs(q.y), abs(q.x) * 0.866025 + abs(q.y) * 0.5) - scale;
            }
            if look == 9u {
                q = rotate2(q, fi * 0.19 + t * 0.22 + gate.y * z * 1.8);
                d = max(abs(q.x), abs(q.y)) - scale;
            }
            light += mix(cyan, orange, f32(i % 4u) / 3.0) * tron_glow(d, 0.003 + z * 0.003) * smoothstep(0.0, 0.15, z) * (1.0 - z * 0.6);
        }
    } else if look == 4u {
        // Thrown discs accelerate across the arena, with position-delayed echoes.
        for (var i: u32 = 0u; i < 7u; i += 1u) {
            let fi: f32 = f32(i);
            for (var echo: u32 = 0u; echo < 3u; echo += 1u) {
                let lag: f32 = f32(echo) * 0.09;
                let disc: vec4<f32> = tron_signal(fi * 0.08 + lag);
                let orbit: f32 = fi * 2.4 + (t - lag) * 0.4 + disc.y * 1.5;
                let flight: f32 = fi * 0.12 + disc.x * 0.7;
                let center: vec2<f32> = vec2<f32>(cos(orbit) * 1.55, sin(orbit)) * flight;
                var q: vec2<f32> = rotate2(p - center, orbit);
                let shard_angle: f32 = floor(atan2(q.y, q.x) * 3.0) / 3.0;
                q -= vec2<f32>(cos(shard_angle), sin(shard_angle)) * disc.z * 0.16;
                let radius: f32 = 0.14 + fi * 0.017;
                let disc_r: f32 = length(q * vec2<f32>(1.0, 1.0 + disc.y * 1.7));
                let edge: f32 = min(abs(disc_r - radius), abs(disc_r - radius * 0.84));
                let cut: f32 = 1.0 - disc.z * smoothstep(0.25, 0.6, sin(atan2(q.y, q.x) * 5.0 + fi));
                light += mix(cyan, orange, f32(i % 2u)) * tron_glow(edge, 0.0035) * cut / (1.0 + f32(echo) * 3.0);
            }
        }
    } else if look == 5u {
        // Circuit board with right-angle traces and moving signal nodes.
        for (var i: u32 = 0u; i < 16u; i += 1u) {
            let fi: f32 = f32(i);
            let circuit: vec4<f32> = tron_signal(fi * 0.052);
            let y: f32 = (fi - 7.5) * 0.13 + circuit.x * sin(fi * 2.0) * 0.065 + circuit.y * sin(fi * 0.7) * 0.18;
            let trace_p: vec2<f32> = p + vec2<f32>(0.0, circuit.z * sin(p.x * 6.0 + fi) * 0.09);
            let bend: f32 = sin(fi * 4.7) * 0.85 + circuit.y * sin(fi * 1.3) * 0.8;
            let a: vec2<f32> = vec2<f32>(-2.0, y);
            let b: vec2<f32> = vec2<f32>(bend, y);
            let c: vec2<f32> = vec2<f32>(bend + 0.22 + circuit.y * 0.28, y + 0.1 + circuit.x * 0.15);
            let d: f32 = min(tron_segment(trace_p, a, b), tron_segment(trace_p, b, c));
            let trace: f32 = min(d, tron_segment(trace_p, c, vec2<f32>(2.0, y + 0.1 + circuit.x * 0.15)));
            let packet: f32 = exp(-pow((p.x - (fract(t * 0.12 + fi * 0.137 + circuit.z * 0.42) * 4.0 - 2.0)) * 12.0, 2.0));
            light += mix(cyan, orange, f32(i % 3u) * 0.5) * tron_glow(trace, 0.0025) * (0.25 + packet * (1.0 + highs));
        }
    } else if look == 6u {
        // Arena: concentric floor rings and a crown of laser pillars.
        let arena: vec4<f32> = tron_signal(clamp(length(p) * 0.65, 0.0, 0.95));
        let q: vec2<f32> = vec2<f32>(p.x + arena.y * sin(p.y * 6.0) * 0.4, (p.y - 0.3) * 2.5);
        light += cyan * tron_glow(abs(fract(length(q) * 3.0 - t * 0.3) - 0.5), 0.016) * exp(-length(q) * 0.8);
        for (var i: u32 = 0u; i < 17u; i += 1u) {
            let fi: f32 = f32(i);
            let pillar: vec4<f32> = tron_signal(abs(fi - 8.0) * 0.1);
            let x: f32 = (fi - 8.0) * 0.16;
            let height: f32 = 0.18 + 0.18 * (0.5 + 0.5 * sin(fi * 0.8 + t)) + pillar.x * 0.65;
            let tip: vec2<f32> = vec2<f32>(x + pillar.y * sin(fi) * 0.65, -height);
            let beam: f32 = tron_glow(tron_segment(p, vec2<f32>(x, 0.1), tip), 0.004);
            let bolt: f32 = tron_glow(tron_segment(p, tip, tip + vec2<f32>(sin(fi), -1.0) * pillar.z * 0.55), 0.008) * pillar.z;
            light += mix(cyan, orange, f32(i % 2u)) * (beam + bolt);
        }
    } else if look == 7u {
        // Solar sail: opposing fans of thin laser beams.
        for (var i: u32 = 0u; i < 24u; i += 1u) {
            let fi: f32 = f32(i);
            let sail: vec4<f32> = tron_signal(fi * 0.035);
            let end: vec2<f32> = vec2<f32>((fi - 11.5) * (0.16 + sail.x * 0.07), -0.7 + sin(fi * 0.3 + t) * (0.12 + sail.y * 0.6));
            let origin: vec2<f32> = vec2<f32>(sin(t * 0.2) * 0.5 + sail.y * sin(fi * 0.35) * 0.9, 0.8 - sail.x * 0.35);
            let tear: f32 = 1.0 - sail.z * smoothstep(0.2, 0.5, sin(p.y * 30.0 + fi));
            light += mix(cyan, orange, fi / 23.0) * tron_glow(tron_segment(p, origin, end), 0.0025) * 0.65 * tear;
        }
    } else if look == 10u {
        // Data rain: segmented falling columns, each with its own speed.
        let rain: vec4<f32> = tron_signal(clamp((p.y + 1.0) * 0.42, 0.0, 0.9));
        let swept: f32 = p.x + rain.y * sin(p.y * 4.0 + t) * 0.3;
        let column: f32 = floor(swept * 22.0);
        let seed: f32 = fract(sin(column * 73.1) * 437.3);
        let head: f32 = fract(p.y * 0.35 - t * (0.12 + seed * 0.18) + seed - rain.x * (0.15 + seed * 0.35));
        let x: f32 = abs(fract((swept + rain.z * sin(column) * 0.045) * 22.0) - 0.5);
        let dash: f32 = 1.0 - smoothstep(0.26, 0.42, abs(fract((p.y - rain.z * seed * 0.4) * 24.0) - 0.5));
        light += mix(cyan, orange, step(0.8, seed)) * tron_glow(x, 0.07) * dash * pow(head, 8.0) * (1.0 + highs);
    } else if look == 11u {
        // Mechanical iris: independent articulated blades, opening and latching.
        light += cyan * tron_glow(abs(r - 0.75), 0.006) * 0.45;
        for (var i: u32 = 0u; i < 24u; i += 1u) {
            let fi: f32 = f32(i);
            let blade: vec4<f32> = tron_signal(fi * 0.035);
            let a: f32 = fi / 24.0 * 6.2831853 + t * 0.18 + blade.y * 0.7;
            let opening: f32 = 0.22 + blade.x * 0.3;
            let hinge: f32 = a + blade.y * 0.85;
            let eject: vec2<f32> = vec2<f32>(cos(a + 0.8), sin(a + 0.8)) * blade.z * 0.3;
            let inner: vec2<f32> = vec2<f32>(cos(hinge), sin(hinge)) * opening + eject;
            let outer: vec2<f32> = vec2<f32>(cos(a), sin(a)) * (0.68 + blade.z * 0.24) + eject;
            let rib: f32 = tron_glow(tron_segment(p, inner, outer), 0.005);
            let ring_tip: f32 = tron_glow(length(p - inner) - 0.014, 0.003);
            light += (orange * rib + cyan * ring_tip) * 1.6;
        }
        light += blue * exp(-r * 8.0) * 0.12;
    } else if look == 12u {
        // Neon Mandala: articulated petals open in a traveling circular wave.
        for (var i: u32 = 0u; i < 18u; i += 1u) {
            let fi: f32 = f32(i);
            let signal: vec4<f32> = tron_signal(fi * 0.045);
            let a: f32 = fi * 6.2831853 / 18.0 + t * 0.12 + signal.y * 0.5;
            let q: vec2<f32> = rotate2(p, a);
            let bend: f32 = signal.y * sin(q.x * 5.0 + fi) * 0.18;
            let petal: vec2<f32> = vec2<f32>(q.x - 0.4 - signal.x * 0.3, (q.y + bend) * (3.0 + signal.z * 2.5));
            let edge: f32 = length(petal) - 0.34;
            light += mix(cyan, orange, f32(i % 3u) * 0.5) * tron_glow(edge, 0.004) * 0.75;
        }
    } else if look == 13u {
        // Plasma Weave: crossing ribbons carry independent spectral ripples.
        for (var i: u32 = 0u; i < 14u; i += 1u) {
            let fi: f32 = f32(i);
            let signal: vec4<f32> = tron_signal(clamp((p.x + 1.8) * 0.23 + fi * 0.013, 0.0, 0.95));
            let wave: f32 = sin(p.x * 2.6 + fi * 0.45 + t * 0.4) * (0.14 + signal.x * 0.25);
            let fold: f32 = sin(p.x * 5.0 - fi * 0.6) * signal.y * 0.2;
            let tear: f32 = sin(p.x * 16.0 + fi) * signal.z * 0.06;
            let y: f32 = (fi - 6.5) * 0.1 + wave + fold + tear;
            light += mix(cyan, vec3<f32>(0.9, 0.03, 0.75), fi / 13.0) * tron_glow(p.y - y, 0.005);
            light += orange * tron_glow(p.x - (fi - 6.5) * 0.15 - sin(p.y * 4.0 + fi + t * 0.25) * (0.2 + signal.y * 0.25), 0.003) * 0.35;
        }
    } else if look == 14u {
        // Spectrum Bloom: nested flowers shear and split in different directions.
        let angle: f32 = atan2(p.y, p.x);
        for (var i: u32 = 0u; i < 12u; i += 1u) {
            let fi: f32 = f32(i);
            let signal: vec4<f32> = tron_signal(fi * 0.075);
            let petal: f32 = sin(angle * 6.0 + fi * 0.4 + t * 0.3 + signal.y * 2.5);
            let shape: f32 = 0.13 + fi * 0.07 + petal * (0.045 + signal.x * 0.12) + sin(angle * 18.0 + fi) * signal.z * 0.055;
            light += mix(vec3<f32>(0.95, 0.04, 0.5), cyan, fi / 11.0) * tron_glow(r - shape, 0.004);
        }
    } else {
        // Chromatic Moire: offset luminous interference sheets shear past each other.
        let signal: vec4<f32> = tron_signal(clamp((p.y + 1.0) * 0.42, 0.0, 0.95));
        let q: vec2<f32> = rotate2(p, 0.35 + signal.y * 0.65);
        let warp: f32 = sin(q.x * 3.0 + t * 0.25) * (0.12 + signal.x * 0.3);
        let a: f32 = sin((q.y + warp) * 27.0 + signal.z * sin(q.x * 11.0) * 2.0);
        let b: f32 = sin((q.x + sin(q.y * 2.3 - t * 0.2) * (0.2 + signal.y * 0.3)) * 24.0);
        light += cyan * tron_glow(a, 0.05) * 0.65;
        light += vec3<f32>(0.95, 0.03, 0.55) * tron_glow(b, 0.05) * 0.65;
    }
    return light * (0.9 + params.reactive.w * 0.35) + orange * light.b * highs * 0.15;
}

fn tron_variant(index: u32, dimension: u32) -> u32 {
    if dimension == 1u {
        let i: u32 = index % 9u;
        if i == 0u { return 4u; }
        if i == 1u { return 5u; }
        if i == 2u { return 7u; }
        if i == 3u { return 10u; }
        if i == 4u { return 11u; }
        return i + 7u;
    }
    if dimension == 2u {
        let i: u32 = index % 7u;
        if i < 4u { return i; }
        if i == 4u { return 6u; }
        if i == 5u { return 8u; }
        return 9u;
    }
    return index % 16u;
}

fn tron_scene(id: u32, uv: vec2<f32>) -> vec3<f32> {
    let t: f32 = params.spatial.x;
    if id != 32u { return tron_look(uv, id - 33u, t); }
    let dimension: u32 = u32(round(params.chromatic.w));
    let chapter: f32 = t / 8.0;
    let current: u32 = u32(floor(chapter));
    let blend: f32 = smoothstep(0.8, 1.0, fract(chapter));
    let outgoing: vec3<f32> = tron_look(uv, tron_variant(current, dimension), t);
    if blend <= 0.0 { return outgoing; }
    return mix(outgoing, tron_look(uv, tron_variant(current + 1u, dimension), t), blend);
}
