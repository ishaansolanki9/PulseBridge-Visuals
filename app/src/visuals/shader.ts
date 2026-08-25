export const vertexShader = `#version 300 es
precision highp float;
void main() {
  vec2 positions[3] = vec2[3](vec2(-1.0, -3.0), vec2(3.0, 1.0), vec2(-1.0, 1.0));
  gl_Position = vec4(positions[gl_VertexID], 0.0, 1.0);
}`;

export const fragmentShader = `#version 300 es
precision highp float;
out vec4 fragColor;

uniform vec2 u_resolution;
uniform float u_time;
uniform vec4 u_music;
uniform vec4 u_pulse;
uniform vec4 u_visual;
uniform vec3 u_colorA;
uniform vec3 u_colorB;
uniform vec3 u_colorC;
uniform vec3 u_colorD;
uniform vec4 u_styleA;
uniform vec4 u_styleB;
uniform vec4 u_effects;

float hash21(vec2 point) {
  point = fract(point * vec2(123.34, 456.21));
  point += dot(point, point + 45.32);
  return fract(point.x * point.y);
}

float noise(vec2 point) {
  vec2 cell = floor(point);
  vec2 local = fract(point);
  vec2 smoothLocal = local * local * (3.0 - 2.0 * local);
  return mix(
    mix(hash21(cell), hash21(cell + vec2(1.0, 0.0)), smoothLocal.x),
    mix(hash21(cell + vec2(0.0, 1.0)), hash21(cell + vec2(1.0, 1.0)), smoothLocal.x),
    smoothLocal.y
  );
}

float fbm(vec2 point) {
  float value = 0.0;
  float amplitude = 0.5;
  for (int octave = 0; octave < 4; octave++) {
    value += noise(point) * amplitude;
    point = mat2(1.62, 1.18, -1.18, 1.62) * point + 0.17;
    amplitude *= 0.5;
  }
  return value;
}

vec3 paletteField(float value) {
  float wrapped = fract(value);
  if (wrapped < 0.3333) return mix(u_colorA, u_colorB, smoothstep(0.0, 0.3333, wrapped));
  if (wrapped < 0.6666) return mix(u_colorB, u_colorC, smoothstep(0.3333, 0.6666, wrapped));
  return mix(u_colorC, u_colorD, smoothstep(0.6666, 1.0, wrapped));
}

vec3 fluidVisual(vec2 uv) {
  float warpA = fbm(uv * (1.25 + u_visual.y * 0.45) + vec2(u_time * 0.075, -u_time * 0.052));
  float warpB = fbm(uv * 1.7 + vec2(-u_time * 0.043, u_time * 0.064) + warpA * 1.8);
  vec2 warped = uv + vec2(warpA - 0.5, warpB - 0.5) * (0.42 + u_music.y * 0.48);
  float field = fbm(warped * (1.18 + u_visual.y * 0.5) + u_time * u_visual.x * 0.08);
  vec3 color = paletteField(field + u_time * 0.018 * u_effects.z + u_pulse.y * 0.06);
  float glow = smoothstep(0.22, 0.9, field) * (0.48 + u_music.x * 0.58);
  return color * (0.28 + glow);
}

vec3 wavesVisual(vec2 uv) {
  float density = 3.0 + u_music.w * 4.2 + u_music.x * 1.8;
  vec2 movingUv = uv * u_visual.z;
  float bend = sin(movingUv.x * 2.1 + u_time * 0.22) * (0.18 + u_music.y * 0.3);
  float wave = sin((movingUv.y + bend) * density - u_time * (0.55 + u_visual.x * 1.1));
  float second = sin((movingUv.x * 0.72 - movingUv.y * 0.84) * (density * 0.72) + u_time * 0.72);
  float band = pow(1.0 - abs(wave), 3.2) + pow(1.0 - abs(second), 5.0) * 0.58;
  float sweep = sin(length(movingUv) * 7.0 - u_time * 1.3 - u_pulse.y * 1.4) * 0.5 + 0.5;
  vec3 color = mix(paletteField(movingUv.x * 0.16 + u_time * 0.026), paletteField(movingUv.y * 0.2 + 0.45), sweep);
  return color * (0.16 + band * (0.62 + u_music.x * 0.58));
}

vec3 pulseVisual(vec2 uv) {
  float radius = length(uv);
  float angle = atan(uv.y, uv.x);
  float expansion = radius * (2.4 - u_pulse.y * 0.22 - u_music.y * 0.18);
  float rings = pow(1.0 - abs(sin(expansion * 5.0 - u_time * (0.65 + u_visual.x))), 5.0);
  float core = exp(-radius * (2.3 - u_music.y * 0.62));
  float petals = sin(angle * 4.0 + u_time * 0.24 + radius * 4.0) * 0.5 + 0.5;
  vec3 color = mix(paletteField(radius * 0.38 - u_time * 0.018), paletteField(angle / 6.28318 + u_time * 0.02 + 0.5), petals * 0.56);
  return color * (0.12 + core * (0.48 + u_pulse.y * 0.5) + rings * (0.24 + u_music.x * 0.55));
}

vec3 tunnelVisual(vec2 uv) {
  float radius = max(length(uv), 0.025);
  float angle = atan(uv.y, uv.x);
  float depth = 1.0 / radius;
  float speed = 0.18 + u_visual.x * 0.42 + u_music.x * 0.35;
  float spiral = angle / 6.28318 + depth * 0.34 - u_time * speed;
  float rings = pow(1.0 - abs(sin(spiral * 12.0 + u_music.y * 1.8)), 5.5);
  float spokes = pow(1.0 - abs(sin(angle * 4.0 + u_time * 0.16)), 10.0) * 0.18;
  vec3 color = paletteField(spiral * 0.16 + u_time * 0.012 * u_effects.z);
  float center = smoothstep(0.02, 0.42, radius);
  return color * (0.08 + rings * (0.46 + u_music.x * 0.72) + spokes) * center;
}

vec3 burstVisual(vec2 uv) {
  float radius = length(uv);
  float angle = atan(uv.y, uv.x);
  float rays = pow(max(0.0, sin(angle * 9.0 + fbm(uv * 2.1) * 4.0)), 6.0);
  float impactFront = exp(-abs(radius - (0.14 + u_effects.y * 0.95)) * 14.0);
  float bassFront = exp(-abs(radius - fract(u_time * 0.08 + u_pulse.y * 0.22)) * 8.0);
  float core = exp(-radius * (2.8 - u_music.y * 0.9));
  vec3 color = mix(paletteField(angle / 6.28318 + u_time * 0.01 * u_effects.z + 0.5), paletteField(radius * 0.4 + u_effects.y * 0.2), core);
  return color * (0.08 + core * 0.28 + rays * 0.24 + impactFront * (0.4 + u_effects.y) + bassFront * u_pulse.y * 0.2);
}

void main() {
  vec2 resolution = max(u_resolution, vec2(1.0));
  vec2 uv = (gl_FragCoord.xy * 2.0 - resolution) / resolution.y;
  uv *= 1.0 - u_pulse.y * 0.035 - u_pulse.w * 0.12;
  vec3 color = fluidVisual(uv) * u_styleA.x
    + wavesVisual(uv) * u_styleA.y
    + pulseVisual(uv) * u_styleA.z
    + tunnelVisual(uv) * u_styleA.w
    + burstVisual(uv) * u_styleB.x;
  float vignette = smoothstep(1.38, 0.25, length(uv * vec2(0.72, 1.0)));
  color *= 0.38 + vignette * 0.78;
  color *= u_visual.w * (0.86 + u_pulse.y * 0.18);
  color += paletteField(u_time * 0.035 * u_effects.z) * u_pulse.z * 0.08;
  color += paletteField(u_time * 0.014) * u_effects.x * 0.035;
  color = mix(color, vec3(1.0), u_pulse.w * 0.72);
  float luminance = dot(color, vec3(0.2126, 0.7152, 0.0722));
  color = mix(vec3(luminance), color, u_styleB.y);
  color = 1.0 - exp(-color * 1.28);
  color = pow(max(color, vec3(0.0)), vec3(0.92));
  if (u_styleB.w > 0.5) color = vec3(0.0);
  fragColor = vec4(color, 1.0);
}`;
