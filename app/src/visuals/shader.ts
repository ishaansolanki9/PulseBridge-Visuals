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
uniform vec4 u_scene;
uniform vec4 u_modifiers;
uniform vec4 u_reactive;

const float TAU = 6.28318530718;

float modifierStrength(float kind) {
  float first = u_modifiers.x >= 0.0 && abs(round(u_modifiers.x) - kind) < 0.1 ? u_modifiers.y : 0.0;
  float second = u_modifiers.z >= 0.0 && abs(round(u_modifiers.z) - kind) < 0.1 ? u_modifiers.w : 0.0;
  return clamp(max(first, second), 0.0, 1.0);
}

vec2 rotate2(vec2 point, float angle) {
  float cosine = cos(angle);
  float sine = sin(angle);
  return mat2(cosine, -sine, sine, cosine) * point;
}

vec3 paletteField(float value) {
  float drift = u_time * (0.004 + u_effects.z * (0.006 + modifierStrength(0.0) * 0.02));
  float amount = 0.5 + 0.5 * sin((value + drift) * TAU);
  return mix(u_colorB, u_colorC, amount);
}

float phaseTime(float rate) {
  return u_time * rate * (0.55 + clamp(u_visual.x, 0.2, 1.8) * 0.45);
}

float lineGlow(float distance, float width) {
  float antialias = max(fwidth(distance) * 1.4, 0.0015);
  return 1.0 - smoothstep(width, width + antialias, abs(distance));
}

float softLine(float distance, float sharpness) {
  return exp(-abs(distance) * sharpness);
}

float softSpot(vec2 point, vec2 center, float radius) {
  vec2 scaled = (point - center) / vec2(radius * 1.35, radius);
  return exp(-dot(scaled, scaled) * 2.6);
}

float structureOpening() {
  return 1.0 + u_music.y * 0.16 + u_reactive.x * 0.1 + u_reactive.w * 0.12 + u_pulse.y * 0.045;
}

float waveCenter(float x, float offset) {
  float travel = phaseTime(0.28) + offset;
  float amplitude = 0.105 + u_music.y * 0.16 + u_reactive.x * 0.075 + u_reactive.w * 0.055;
  return sin(x * 2.45 + travel) * amplitude
    + sin(x * 5.1 - travel * 0.63 + offset * 1.7) * (0.026 + u_music.z * 0.035);
}

vec3 colorSplotchWave(vec2 uv) {
  float shake = sin(u_pulse.x * TAU) * u_pulse.y * 0.038
    + sin(u_time * 7.0) * u_effects.y * 0.026
    + sin(u_time * 3.7) * u_reactive.x * 0.022;
  vec2 point = uv + vec2(0.0, shake);
  float center = waveCenter(point.x, 0.0);
  float distance = point.y - center;
  float width = 0.009 + u_scene.z * 0.005;
  float mainTrace = lineGlow(distance, width);
  float body = softLine(distance, 8.5) * (1.0 - smoothstep(0.18, 0.42, abs(distance)));
  float echoSpacing = 0.07 + u_music.y * 0.045;
  float echoGate = smoothstep(0.22, 0.62, u_scene.z);
  float echoes = (lineGlow(distance - echoSpacing, width * 0.72)
    + lineGlow(distance + echoSpacing, width * 0.72)) * echoGate;
  vec3 color = paletteField(point.x * 0.08) * (mainTrace * 0.9 + body * 0.12 + echoes * 0.22);

  float xA = -0.78 + sin(u_time * 0.11) * 0.09;
  float xB = -0.24 + sin(u_time * 0.09 + 1.8) * 0.08;
  float xC = 0.33 + sin(u_time * 0.1 + 3.1) * 0.1;
  float xD = 0.86 + sin(u_time * 0.08 + 4.4) * 0.07;
  float spotA = softSpot(point, vec2(xA, waveCenter(xA, 0.0)), 0.055);
  float spotB = softSpot(point, vec2(xB, waveCenter(xB, 0.0)), 0.045);
  float spotC = softSpot(point, vec2(xC, waveCenter(xC, 0.0)), 0.065);
  float spotD = softSpot(point, vec2(xD, waveCenter(xD, 0.0)), 0.04);
  float embedded = 1.0 - smoothstep(0.025, 0.16, abs(distance));
  color += u_colorD * (spotA + spotC) * embedded * 0.62;
  color += u_colorC * (spotB + spotD) * embedded * 0.48;
  return color;
}

vec3 multiLayerWaveField(vec2 uv) {
  float travel = phaseTime(0.24);
  float layerCount = 3.0 + floor(u_scene.z * 5.0);
  float spacing = 0.16 + (1.0 - u_music.x) * 0.018;
  float amplitude = 0.045 + u_music.y * 0.09 + u_reactive.x * 0.045;
  vec3 color = vec3(0.0);
  for (int layer = 0; layer < 8; layer++) {
    float index = float(layer);
    float visibility = 1.0 - smoothstep(layerCount - 0.15, layerCount + 0.15, index);
    float centered = index - (layerCount - 1.0) * 0.5;
    float phase = travel + centered * 0.52;
    float wave = centered * spacing
      + sin(uv.x * (2.2 + index * 0.12) + phase) * amplitude
      + sin(uv.x * 4.6 - phase * 0.7) * u_music.z * 0.025;
    float trace = lineGlow(uv.y - wave, 0.006 + u_scene.w * 0.003);
    color += paletteField(index * 0.085 + uv.x * 0.025) * trace * visibility * (0.34 + u_music.x * 0.28);
  }
  return color * (1.0 - smoothstep(0.72, 1.18, abs(uv.y)));
}

vec3 fractalBloom(vec2 uv) {
  vec2 point = rotate2(uv / structureOpening(), phaseTime(0.035));
  float radius = length(point);
  float angle = atan(point.y, point.x);
  float visibleLevels = 2.0 + floor(u_scene.z * 3.0);
  vec3 color = vec3(0.0);
  for (int level = 0; level < 5; level++) {
    float index = float(level);
    float visibility = 1.0 - smoothstep(visibleLevels - 0.15, visibleLevels + 0.15, index);
    float petals = 6.0 + index * 4.0;
    float targetRadius = 0.19 + index * 0.155
      + sin(angle * petals + phaseTime(0.12) * (1.0 - index * 0.08))
      * (0.022 + index * 0.008 + u_music.z * 0.015);
    float trace = lineGlow(radius - targetRadius, 0.0065 + u_scene.w * 0.0025);
    color += paletteField(index * 0.11 + angle / TAU) * trace * visibility * (0.56 / (1.0 + index * 0.17));
  }
  float core = softLine(radius - (0.075 + u_pulse.y * 0.025), 42.0);
  color += u_colorD * core * (0.12 + u_pulse.y * 0.22);
  return color * smoothstep(0.045, 0.12, radius) * (1.0 - smoothstep(0.88, 1.3, radius));
}

vec3 recursiveTunnel(vec2 uv) {
  vec2 point = rotate2(uv / structureOpening(), phaseTime(0.055));
  float travel = fract(phaseTime(0.05) + u_pulse.x * 0.06);
  float circleRadius = length(point);
  float diamondRadius = (abs(point.x) + abs(point.y)) * 0.72;
  vec3 color = vec3(0.0);
  for (int level = 0; level < 7; level++) {
    float index = float(level);
    float depth = fract(index / 7.0 + travel);
    float shape = mix(circleRadius, diamondRadius, 0.32 + 0.18 * sin(index * 1.7));
    float trace = lineGlow(shape - (0.11 + depth * 1.02), 0.006 + depth * 0.004);
    float fade = smoothstep(0.0, 0.12, depth) * (1.0 - smoothstep(0.72, 1.0, depth));
    color += paletteField(depth * 0.34 + index * 0.07) * trace * fade * 0.62;
  }
  return color + u_colorD * softLine(circleRadius, 24.0) * (0.08 + u_pulse.y * 0.2);
}

vec3 visualFamily(int id, vec2 uv) {
  if (id == 0) return colorSplotchWave(uv);
  if (id == 1) return multiLayerWaveField(uv);
  if (id == 2) return fractalBloom(uv);
  if (id == 3) return recursiveTunnel(uv);
  return colorSplotchWave(uv);
}

void main() {
  vec2 resolution = max(u_resolution, vec2(1.0));
  vec2 stableUv = (gl_FragCoord.xy * 2.0 - resolution) / resolution.y;
  float scale = 1.0 - u_pulse.y * 0.012 - u_reactive.x * 0.022 - u_reactive.w * 0.014;
  vec2 uv = stableUv * scale;
  int primaryId = int(round(u_styleA.x));
  int secondaryId = int(round(u_styleA.y));
  vec3 color = visualFamily(primaryId, uv) * u_styleA.z;
  if (u_styleA.w > 0.001) color += visualFamily(secondaryId, uv) * u_styleA.w;
  float vignette = 1.0 - smoothstep(0.82, 1.62, length(stableUv * vec2(0.68, 1.0)));
  color *= (0.58 + vignette * 0.42) * u_visual.w * (0.9 + u_music.x * 0.12);
  float luminance = dot(color, vec3(0.2126, 0.7152, 0.0722));
  float budget = 0.56 + u_scene.w * 0.3;
  if (luminance > budget) color *= budget / max(luminance, 0.001);
  color = mix(vec3(dot(color, vec3(0.2126, 0.7152, 0.0722))), color, u_styleB.x);
  color = 1.0 - exp(-max(color, vec3(0.0)) * 1.24);
  color = pow(max(color, vec3(0.0)), vec3(0.95));
  if (u_styleB.w > 0.5) color = vec3(0.0);
  fragColor = vec4(color, 1.0);
}`;
