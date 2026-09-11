import { tronHelpers, tronLookShaders } from "./tronShader";

export const vertexShader = `#version 300 es
precision highp float;
void main() {
  vec2 positions[3] = vec2[3](vec2(-1.0, -3.0), vec2(3.0, 1.0), vec2(-1.0, 1.0));
  gl_Position = vec4(positions[gl_VertexID], 0.0, 1.0);
}`;

const ambientShader = `#version 300 es
precision highp float;
out vec4 fragColor;

uniform vec2 u_resolution;
uniform float u_opacity;
uniform float u_time;
uniform float u_tronTime;
uniform float u_dimension;
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

float hash21(vec2 point) {
  point = fract(point * vec2(123.34, 456.21));
  point += dot(point, point + 45.32);
  return fract(point.x * point.y);
}

float modifierStrength(float kind) {
  float first = u_modifiers.x >= 0.0 && abs(round(u_modifiers.x) - kind) < 0.1 ? u_modifiers.y : 0.0;
  float second = u_modifiers.z >= 0.0 && abs(round(u_modifiers.z) - kind) < 0.1 ? u_modifiers.w : 0.0;
  return clamp(max(first, second), 0.0, 1.0);
}

vec3 paletteField(float value) {
  float hitShift = u_reactive.x * 0.07 + u_reactive.z * 0.19 + u_pulse.z * 0.06;
  float scaled = fract(value + u_time * 0.035 * modifierStrength(0.0) + hitShift) * 4.0;
  float local = smoothstep(0.0, 1.0, fract(scaled));
  int segment = int(floor(scaled));
  if (segment == 0) return mix(u_colorA, u_colorB, local);
  if (segment == 1) return mix(u_colorB, u_colorC, local);
  if (segment == 2) return mix(u_colorC, u_colorD, local);
  return mix(u_colorD, u_colorA, local);
}

vec3 tunnelVisual(vec2 uv) {
  float radius = max(length(uv), 0.025);
  float angle = atan(uv.y, uv.x);
  float depth = 1.0 / radius;
  float speed = 0.18 + u_visual.x * 0.38 + u_music.x * 0.3;
  float rings = pow(max(0.0, 1.0 - abs(sin(depth * 2.15 + angle * 4.0 - u_time * speed * TAU))), 6.0);
  float spokes = pow(max(0.0, 1.0 - abs(sin(angle * 6.0 + u_time * 0.16))), 12.0) * 0.15;
  vec3 color = paletteField(angle / TAU + depth * 0.08 - u_time * 0.04 * u_effects.z);
  return color * (rings * (0.42 + u_music.x * 0.58) + spokes) * smoothstep(0.02, 0.38, radius);
}

void main() {
  vec2 resolution = max(u_resolution, vec2(1.0));
  vec2 uv = (gl_FragCoord.xy * 2.0 - resolution) / resolution.y;
  float overdrive = clamp(u_effects.w, 0.0, 1.0);
  float hitForce = clamp(u_pulse.y * 0.85 + u_pulse.z * 0.45 + u_effects.y * 0.35, 0.0, 1.5);
  float beatZoom = modifierStrength(1.0);
  float bassWarp = modifierStrength(2.0);
  float mirrorFold = modifierStrength(5.0);
  float bassHit = clamp(u_reactive.x, 0.0, 1.0);
  float midMotion = clamp(u_reactive.y, 0.0, 1.0);
  float highHit = clamp(u_reactive.z, 0.0, 1.0);
  float energyRise = clamp(u_reactive.w, 0.0, 1.0);
  float sourceRadius = max(length(uv), 0.001);
  vec2 radialDirection = uv / sourceRadius;
  float bassWave = sin(sourceRadius * (12.0 + u_scene.z * 9.0) - u_pulse.x * TAU);
  uv += radialDirection * bassWave * bassHit * (0.026 + u_styleB.y * 0.046);
  vec2 midBend = vec2(
    sin(uv.y * (4.0 + u_music.z * 4.5) + u_time * 0.7),
    sin(uv.x * (3.4 + u_music.z * 3.8) - u_time * 0.56)
  );
  uv += midBend * midMotion * (0.024 + u_music.z * 0.038);
  float sliceRate = 6.0 + floor(u_music.w * 8.0);
  float slice = floor((uv.y + 1.7) * sliceRate);
  float sliceTick = floor(u_time * (6.0 + highHit * 12.0) + u_pulse.x * 4.0);
  uv.x += (hash21(vec2(slice, sliceTick)) - 0.5) * highHit * (0.026 + u_styleB.y * 0.046);
  float spin = overdrive * (sin(u_time * (0.9 + u_music.x * 1.8)) * (0.025 + u_music.x * 0.055) + u_pulse.y * 0.08 - u_pulse.z * 0.045);
  float spinCos = cos(spin);
  float spinSin = sin(spin);
  uv = mat2(spinCos, -spinSin, spinSin, spinCos) * uv;
  uv *= 1.0 - u_pulse.y * (0.025 + beatZoom * 0.09 + overdrive * 0.16) - bassHit * (0.055 + beatZoom * 0.06) - energyRise * 0.028 - u_pulse.z * overdrive * 0.06 - u_pulse.w * 0.1;
  uv += vec2(sin(uv.y * 3.2 + u_time * 1.3), sin(uv.x * 2.7 - u_time * 1.1)) * (u_music.y * (bassWarp + overdrive * 0.85) * (0.12 + overdrive * 0.08) + midMotion * (0.025 + bassWarp * 0.025));
  float jitterTick = floor(u_time * 12.0);
  uv += vec2(hash21(vec2(jitterTick, 17.0)) - 0.5, hash21(vec2(jitterTick, 43.0)) - 0.5) * overdrive * u_pulse.z * 0.065;
  float wildFold = overdrive * clamp(u_pulse.z * 0.55 + u_pulse.y * 0.25 + u_music.x * 0.12, 0.0, 0.72);
  uv.x = mix(uv.x, abs(uv.x) - 0.28, max(mirrorFold, wildFold));
  vec3 color = tunnelVisual(uv) * u_styleA.z;
  if (u_styleA.w > 0.001) color += tunnelVisual(uv) * u_styleA.w;
  float vignette = smoothstep(1.48, 0.22, length(uv * vec2(0.68, 1.0)));
  color *= 0.3 + vignette * 0.82;
  color *= u_visual.w * (0.88 + u_pulse.y * 0.14 + bassHit * 0.16 + energyRise * 0.3) * (1.0 + overdrive * (0.12 + hitForce * 0.38));
  color += paletteField(u_time * 0.035 * u_effects.z) * u_pulse.z * (0.045 + overdrive * 0.2);
  color += paletteField(u_time * 0.014) * u_effects.x * (0.018 + overdrive * 0.055);
  float sparkle = modifierStrength(3.0);
  float sparkleDrive = max(sparkle, overdrive * (0.18 + u_music.w * 0.82));
  vec2 sparkleCell = floor((uv + u_time * vec2(0.17, -0.11)) * (34.0 + overdrive * 18.0));
  float sparkleSeed = hash21(sparkleCell);
  float sparkleMask = step(0.985 - u_music.w * 0.02 - overdrive * 0.028, sparkleSeed) * pow(max(0.0, sin(u_time * (8.0 + overdrive * 8.0) + sparkleSeed * TAU)), 10.0);
  color += paletteField(sparkleSeed) * sparkleMask * sparkleDrive * (0.08 + u_music.w * 0.22 + overdrive * 0.18);
  float trails = modifierStrength(4.0);
  float trailsDrive = max(trails, overdrive * (0.12 + u_music.y * 0.52 + u_pulse.y * 0.3));
  float trailBand = exp(-abs(sin((uv.x - uv.y) * (5.0 + overdrive * 3.0) - u_time * (1.4 + overdrive * 2.8))) * (8.0 - overdrive * 2.0));
  color += paletteField(uv.x * 0.1 - u_time * (0.025 + overdrive * 0.08)) * trailBand * trailsDrive * (0.055 + overdrive * 0.12);
  float chromatic = modifierStrength(6.0);
  float chromaticDrive = max(chromatic, overdrive * (0.18 + u_pulse.z * 0.7 + u_music.w * 0.3));
  float edge = min(0.28, length(fwidth(color)));
  color += vec3(edge, edge * 0.18, edge * 0.82) * chromaticDrive * (0.35 + u_music.w * 0.3 + overdrive * 0.5);
  float impactBloom = modifierStrength(7.0);
  float impactLevel = clamp(u_effects.y, 0.0, 1.0);
  float impactDrive = max(impactBloom, overdrive * smoothstep(0.28, 0.82, impactLevel));
  float impactFront = exp(-abs(length(uv) - (0.18 + impactLevel * 0.9)) * 18.0);
  float impactEcho = exp(-abs(length(uv) - (0.1 + impactLevel * 0.62)) * 26.0);
  color += paletteField(length(uv) * 0.25 + u_time * 0.02) * (impactFront + impactEcho * overdrive * 0.72) * impactDrive * (0.28 + overdrive * 0.34);
  float responseRadius = length(uv);
  float responseAngle = atan(uv.y, uv.x);
  float bassFront = exp(-abs(responseRadius - (0.12 + fract(u_pulse.x + bassHit * 0.08) * 1.18)) * 20.0);
  color += paletteField(responseAngle / TAU + responseRadius * 0.42) * bassFront * bassHit * (0.22 + u_styleB.y * 0.32);
  float midRibs = pow(max(0.0, 1.0 - abs(sin((uv.x + uv.y * 0.74) * (7.0 + u_music.z * 7.0) + u_time * 0.9))), 10.0);
  color += paletteField(uv.x * 0.21 - uv.y * 0.13 + u_music.z * 0.24) * midRibs * midMotion * (0.08 + u_music.z * 0.13);
  float shardCount = 16.0 + floor(u_music.w * 14.0);
  float highRayPhase = responseAngle * shardCount + sin(responseRadius * 7.0 - u_time) * 1.15 + u_time * (2.2 + highHit * 3.4);
  float highRay = pow(max(0.0, 1.0 - abs(sin(highRayPhase))), 16.0) * (1.0 - smoothstep(0.72, 1.9, fwidth(highRayPhase)));
  float highGatePhase = responseRadius * 19.0 - u_time * 1.8 + u_styleB.z * TAU;
  float highGate = 0.28 + pow(max(0.0, 1.0 - abs(sin(highGatePhase))), 11.0) * (1.0 - smoothstep(0.72, 1.9, fwidth(highGatePhase))) * 0.72;
  float highShard = highRay * highGate * smoothstep(0.08, 0.34, responseRadius) * (1.0 - smoothstep(1.1, 1.58, responseRadius));
  color += paletteField(responseAngle / TAU * 4.0 + responseRadius * 0.3) * highShard * highHit * (0.24 + u_styleB.y * 0.24);
  vec3 spectralTint = paletteField(responseAngle / TAU + u_music.y * 0.12 + u_music.z * 0.28 + u_music.w * 0.46);
  float colorReaction = clamp(bassHit * 0.16 + midMotion * 0.12 + highHit * 0.27 + u_pulse.z * 0.18, 0.0, 0.55);
  color = mix(color, color * (0.52 + spectralTint * 1.58), colorReaction);
  float radius = length(uv);
  float angle = atan(uv.y, uv.x);
  float wildRays = pow(max(0.0, sin(angle * 12.0 + u_time * (2.8 + u_music.x * 4.0))), 10.0);
  float wildRings = pow(max(0.0, 1.0 - abs(sin(radius * (10.0 + u_scene.z * 8.0) - u_time * (2.0 + u_music.x * 5.0) - u_pulse.x * TAU))), 9.0);
  float wildGeometry = max(wildRays * 0.65, wildRings) * overdrive * (0.04 + u_music.x * 0.18 + hitForce * 0.18);
  color += paletteField(angle / TAU * 3.0 + radius * 0.4 + u_time * 0.08 * u_effects.z) * wildGeometry;
  color = mix(color, color.gbr, overdrive * u_pulse.z * 0.16);
  color = mix(color, vec3(1.0), u_pulse.w * (0.68 + overdrive * 0.16));
  float luminance = dot(color, vec3(0.2126, 0.7152, 0.0722));
  float luminanceBudget = 0.68 + u_scene.w * 0.36 + overdrive * 0.14;
  if (luminance > luminanceBudget) color *= luminanceBudget / max(luminance, 0.001);
  color = mix(vec3(dot(color, vec3(0.2126, 0.7152, 0.0722))), color, u_styleB.x);
  color = 1.0 - exp(-max(color, vec3(0.0)) * (1.2 + overdrive * 0.28));
  color = pow(max(color, vec3(0.0)), vec3(0.94));
  if (u_styleB.w > 0.5) color = vec3(0.0);
  fragColor = vec4(color * u_opacity, 1.0);
}`;

// The Auto preview only uses the ambient tunnel. Held/cycling Tron previews
// compile a single specialized look with one call site, never the full library.
export function previewFragmentShader(family: number): string {
  const look = tronLookShaders[family - 33];
  if (!look) return ambientShader;
  return `#version 300 es
precision highp float;
out vec4 fragColor;
uniform vec2 u_resolution;
uniform float u_tronTime;
uniform vec4 u_visual;
uniform float u_opacity;
const vec4 u_reactive = vec4(0.0);
${tronHelpers}
${look}
void main() {
  vec2 uv = (gl_FragCoord.xy * 2.0 - u_resolution) / max(u_resolution.y, 1.0);
  vec3 neon = tron_look(vec2(uv.x, -uv.y), u_tronTime) * u_visual.w * 1.6;
  vec3 color = pow(1.0 - exp(-max(neon, vec3(0.0)) * 1.22), vec3(0.94));
  fragColor = vec4(color * u_opacity, 1.0);
}`;
}
