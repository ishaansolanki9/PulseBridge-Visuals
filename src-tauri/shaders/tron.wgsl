// Shared with the ambient preview via scripts/sync-tron-shader.py.
// Analytic distance fields: bounded work, filtered edges, no textures or particles.
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
        let horizon: f32 = p.y + 0.18;
        let depth: f32 = 1.0 / max(abs(horizon), 0.035);
        let world: vec2<f32> = vec2<f32>(p.x * depth, depth + tick);
        let cell: vec2<f32> = abs(fract(world * 0.5) - 0.5);
        let grid: f32 = tron_glow(min(cell.x, cell.y), 0.012);
        let fade: f32 = smoothstep(0.035, 0.25, horizon) / (1.0 + depth * 0.035);
        light += cyan * grid * fade * (0.55 + bass * 0.6);
        light += blue * exp(-abs(horizon) * 20.0) * 0.35;
        if look == 0u {
            let sun: f32 = tron_glow(length(p - vec2<f32>(0.0, -0.43)) - 0.22 - bass * 0.025, 0.009);
            light += orange * sun;
        }
        if look == 1u {
            for (var i: u32 = 0u; i < 6u; i += 1u) {
                let fi: f32 = f32(i);
                let lane: f32 = (fi - 2.5) * 1.65 + sin(t * 0.3 + fi) * 0.35;
                let trail: f32 = tron_glow(world.x - lane, 0.028) * (0.35 + 0.65 * pow(fract(world.y * 0.08 - t * 0.2 + fi * 0.17), 3.0));
                light += mix(cyan, orange, f32(i % 2u)) * trail * fade;
            }
        }
        if look == 8u {
            for (var i: u32 = 0u; i < 18u; i += 1u) {
                let fi: f32 = f32(i);
                let x: f32 = (fi - 8.5) * 0.19;
                let height: f32 = 0.12 + 0.5 * fract(sin(fi * 32.7) * 437.1) + mids * 0.07;
                let box: vec2<f32> = abs(p - vec2<f32>(x, -0.18 - height * 0.5)) - vec2<f32>(0.066, height * 0.5);
                let edge: f32 = abs(max(box.x, box.y));
                light += mix(cyan, orange, f32(i % 3u) * 0.35) * tron_glow(edge, 0.003) * 0.75;
            }
        }
    } else if look == 2u || look == 3u || look == 9u {
        // Rectangular gates, hexagonal corridor, and a twisting laser helix.
        for (var i: u32 = 0u; i < 18u; i += 1u) {
            let fi: f32 = f32(i);
            let z: f32 = fract((fi - t * 1.1) / 18.0);
            let scale: f32 = 0.08 + z * z * 2.5;
            let rotation: f32 = mids * sin(fi * 0.4 + t) * 0.12;
            var q: vec2<f32> = rotate2(p, rotation);
            var d: f32 = max(abs(q.x) * 0.72, abs(q.y)) - scale;
            if look == 3u {
                d = max(abs(q.y), abs(q.x) * 0.866025 + abs(q.y) * 0.5) - scale;
            }
            if look == 9u {
                q = rotate2(p, fi * 0.19 + t * 0.22);
                d = max(abs(q.x), abs(q.y)) - scale;
            }
            light += mix(cyan, orange, f32(i % 4u) / 3.0) * tron_glow(d - bass * z * 0.06, 0.003 + z * 0.003) * smoothstep(0.0, 0.15, z) * (1.0 - z * 0.6);
        }
    } else if look == 4u {
        // Identity discs: orbiting double rings with broken orange arcs.
        for (var i: u32 = 0u; i < 5u; i += 1u) {
            let fi: f32 = f32(i);
            let center: vec2<f32> = vec2<f32>(cos(fi * 2.4 + t * 0.24), sin(fi * 2.4 + t * 0.24)) * fi * 0.18;
            let q: vec2<f32> = p - center;
            let radius: f32 = 0.19 + fi * 0.027 + bass * 0.025;
            let edge: f32 = min(abs(length(q) - radius), abs(length(q) - radius * 0.84));
            light += mix(cyan, orange, f32(i % 2u)) * tron_glow(edge, 0.004);
        }
    } else if look == 5u {
        // Circuit board with right-angle traces and moving signal nodes.
        for (var i: u32 = 0u; i < 16u; i += 1u) {
            let fi: f32 = f32(i);
            let y: f32 = (fi - 7.5) * 0.13;
            let bend: f32 = sin(fi * 4.7) * 0.85;
            let a: vec2<f32> = vec2<f32>(-2.0, y);
            let b: vec2<f32> = vec2<f32>(bend, y);
            let c: vec2<f32> = vec2<f32>(bend + 0.22, y + 0.1);
            let d: f32 = min(tron_segment(p, a, b), tron_segment(p, b, c));
            let trace: f32 = min(d, tron_segment(p, c, vec2<f32>(2.0, y + 0.1)));
            let packet: f32 = exp(-pow((p.x - (fract(t * 0.12 + fi * 0.137) * 4.0 - 2.0)) * 12.0, 2.0));
            light += mix(cyan, orange, f32(i % 3u) * 0.5) * tron_glow(trace, 0.0025) * (0.25 + packet * (1.0 + highs));
        }
    } else if look == 6u {
        // Arena: concentric floor rings and a crown of laser pillars.
        let q: vec2<f32> = vec2<f32>(p.x, (p.y - 0.3) * 2.5);
        light += cyan * tron_glow(abs(fract(length(q) * 3.0 - t * 0.3) - 0.5), 0.016) * exp(-length(q) * 0.8);
        for (var i: u32 = 0u; i < 17u; i += 1u) {
            let fi: f32 = f32(i);
            let x: f32 = (fi - 8.0) * 0.16;
            let height: f32 = 0.18 + 0.45 * (0.5 + 0.5 * sin(fi * 0.8 + t)) + bass * 0.23;
            light += mix(cyan, orange, f32(i % 2u)) * tron_glow(tron_segment(p, vec2<f32>(x, 0.1), vec2<f32>(x, -height)), 0.004);
        }
    } else if look == 7u {
        // Solar sail: opposing fans of thin laser beams.
        for (var i: u32 = 0u; i < 24u; i += 1u) {
            let fi: f32 = f32(i);
            let end: vec2<f32> = vec2<f32>((fi - 11.5) * 0.16, -0.7 + sin(fi * 0.3 + t) * (0.12 + mids * 0.25));
            light += mix(cyan, orange, fi / 23.0) * tron_glow(tron_segment(p, vec2<f32>(sin(t * 0.2) * 0.5, 0.8), end), 0.0025) * 0.65;
        }
    } else if look == 10u {
        // Data rain: segmented falling columns, each with its own speed.
        let column: f32 = floor(p.x * 22.0);
        let seed: f32 = fract(sin(column * 73.1) * 437.3);
        let head: f32 = fract(p.y * 0.35 - t * (0.12 + seed * 0.18) + seed);
        let x: f32 = abs(fract(p.x * 22.0) - 0.5);
        let dash: f32 = 1.0 - smoothstep(0.26, 0.42, abs(fract(p.y * 24.0) - 0.5));
        light += mix(cyan, orange, step(0.8, seed)) * tron_glow(x, 0.07) * dash * pow(head, 8.0) * (1.0 + highs);
    } else {
        // Reactor iris: concentric machinery and rotating segmented spokes.
        let q: vec2<f32> = rotate2(p, t * 0.18);
        let spokes: f32 = abs(sin(atan2(q.y, q.x) * 12.0)) * max(r, 0.03);
        let ring: f32 = min(abs(r - 0.3 - bass * 0.04), min(abs(r - 0.58), abs(r - 0.75)));
        light += cyan * tron_glow(ring, 0.006);
        light += orange * tron_glow(spokes, 0.007) * smoothstep(0.32, 0.38, r) * (1.0 - smoothstep(0.68, 0.74, r));
        light += blue * exp(-r * 8.0) * (0.5 + bass);
    }
    return light * (0.9 + params.reactive.w * 0.35) + orange * light.b * highs * 0.15;
}

fn tron_scene(id: u32, uv: vec2<f32>) -> vec3<f32> {
    let t: f32 = params.spatial.x;
    let p: vec2<f32> = rotate2(uv, params.reactive.y * 0.06) * (1.0 - params.reactive.x * 0.045);
    if id != 32u { return tron_look(p, id - 33u, t); }
    // Long dwell, then a gentle dissolve; integrated motion never jumps on hits.
    let chapter: f32 = t / 8.0;
    let current: u32 = u32(floor(chapter)) % 12u;
    let blend: f32 = smoothstep(0.8, 1.0, fract(chapter));
    let outgoing: vec3<f32> = tron_look(p, current, t);
    if blend <= 0.0 { return outgoing; }
    return mix(outgoing, tron_look(p, (current + 1u) % 12u, t), blend);
}
