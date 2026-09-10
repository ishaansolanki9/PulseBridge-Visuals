// Dissolve completed scene images; neither scene's effects touch the other.
@group(0) @binding(0) var outgoing: texture_2d<f32>;
@group(0) @binding(1) var incoming: texture_2d<f32>;
@group(0) @binding(2) var image_sampler: sampler;
@group(0) @binding(3) var<uniform> weights: vec4<f32>;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let vertices = array<vec2<f32>, 3>(vec2<f32>(-1.0, -3.0), vec2<f32>(3.0, 1.0), vec2<f32>(-1.0, 1.0));
    return vec4<f32>(vertices[index], 0.0, 1.0);
}
@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = position.xy / vec2<f32>(textureDimensions(outgoing));
    let a = textureSample(outgoing, image_sampler, uv).rgb;
    let b = textureSample(incoming, image_sampler, uv).rgb;
    return vec4<f32>(a * weights.x + b * weights.y, 1.0);
}
