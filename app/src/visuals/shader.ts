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

float hash21(vec2 point) {
  point = fract(point * vec2(123.34, 456.21));
  point += dot(point, point + 45.32);
  return fract(point.x * point.y);
}

vec2 rotate2(vec2 point, float angle) {
  float cosine = cos(angle);
  float sine = sin(angle);
  return mat2(cosine, -sine, sine, cosine) * point;
}

float hash11(float value) {
  return fract(sin(value * 127.1 + u_styleB.z * 311.7) * 43758.5453);
}

float modifierStrength(float kind) {
  float first = u_modifiers.x >= 0.0 && abs(round(u_modifiers.x) - kind) < 0.1 ? u_modifiers.y : 0.0;
  float second = u_modifiers.z >= 0.0 && abs(round(u_modifiers.z) - kind) < 0.1 ? u_modifiers.w : 0.0;
  return clamp(max(first, second), 0.0, 1.0);
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
  float hitShift = u_reactive.x * 0.07 + u_reactive.z * 0.19 + u_pulse.z * 0.06;
  float scaled = fract(value + u_time * 0.035 * modifierStrength(0.0) + hitShift) * 4.0;
  float local = smoothstep(0.0, 1.0, fract(scaled));
  int segment = int(floor(scaled));
  if (segment == 0) return mix(u_colorA, u_colorB, local);
  if (segment == 1) return mix(u_colorB, u_colorC, local);
  if (segment == 2) return mix(u_colorC, u_colorD, local);
  return mix(u_colorD, u_colorA, local);
}

vec3 fluidVisual(vec2 uv) {
  float warpA = fbm(uv * (1.15 + u_scene.y * 0.5) + vec2(u_time * 0.075, -u_time * 0.052));
  float warpB = fbm(uv * 1.7 + vec2(-u_time * 0.043, u_time * 0.064) + warpA * 1.8);
  vec2 warped = uv + vec2(warpA - 0.5, warpB - 0.5) * (0.35 + u_music.y * 0.45);
  float field = fbm(warped * (1.05 + u_scene.z * 0.55) + u_time * u_visual.x * 0.07);
  vec3 color = paletteField(field + u_time * 0.018 * u_effects.z);
  return color * (0.08 + smoothstep(0.3, 0.88, field) * (0.34 + u_music.x * 0.5));
}

vec3 wavesVisual(vec2 uv) {
  float density = 2.6 + u_scene.z * 3.4 + u_music.w * 1.5;
  vec2 movingUv = uv * u_visual.z;
  float bend = sin(movingUv.x * 2.1 + u_time * 0.22) * (0.16 + u_music.y * 0.26);
  float wave = sin((movingUv.y + bend) * density - u_time * (0.55 + u_visual.x));
  float second = sin((movingUv.x * 0.72 - movingUv.y * 0.84) * density * 0.72 + u_time * 0.72);
  float band = pow(max(0.0, 1.0 - abs(wave)), 5.0) + pow(max(0.0, 1.0 - abs(second)), 7.0) * 0.42;
  vec3 color = mix(paletteField(movingUv.x * 0.16 + u_time * 0.026), paletteField(movingUv.y * 0.2 + 0.45), sin(length(movingUv) * 7.0 - u_time * 1.3) * 0.5 + 0.5);
  return color * band * (0.48 + u_music.x * 0.5);
}

vec3 pulseVisual(vec2 uv) {
  float radius = length(uv);
  float angle = atan(uv.y, uv.x);
  float expansion = radius * (2.4 - u_pulse.y * 0.22 - u_music.y * 0.18);
  float rings = pow(max(0.0, 1.0 - abs(sin(expansion * 5.0 - u_time * (0.65 + u_visual.x)))), 6.0);
  float core = exp(-radius * (3.0 - u_music.y * 0.5));
  float petals = sin(angle * 4.0 + u_time * 0.24 + radius * 4.0) * 0.5 + 0.5;
  vec3 color = mix(paletteField(radius * 0.38 - u_time * 0.018), paletteField(angle / TAU + u_time * 0.02), petals * 0.5);
  return color * (core * (0.32 + u_pulse.y * 0.42) + rings * (0.2 + u_music.x * 0.48));
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

vec3 bloomVisual(vec2 uv) {
  float radius = length(uv);
  float angle = atan(uv.y, uv.x);
  float opening = 0.62 + u_music.y * 0.5 + u_pulse.y * 0.22;
  float petals = pow(max(0.0, cos(angle * 6.0 + u_time * 0.38) * 0.5 + 0.5), 3.0);
  float petalRing = exp(-abs(radius - (0.28 + petals * 0.3) * opening) * 13.0);
  float fold = pow(max(0.0, 1.0 - abs(sin(radius * 8.0 - u_time * 0.52))), 7.0);
  float core = exp(-radius * (4.0 - u_music.y * 0.9));
  vec3 color = mix(paletteField(angle / TAU * 2.0 + u_time * 0.012 * u_effects.z), paletteField(radius * 0.5 + u_music.y * 0.18), core);
  return color * (core * 0.22 + petalRing * (0.3 + u_music.x * 0.38) + fold * petals * 0.12);
}

vec3 auroraVisual(vec2 uv) {
  float drift = u_time * (0.045 + u_scene.x * 0.035);
  float curtainNoise = fbm(vec2(uv.x * 0.72 + drift, uv.y * 0.18 - drift * 0.3));
  float center = sin(uv.x * 1.45 + curtainNoise * 2.2 + drift) * 0.28;
  float curtain = exp(-abs(uv.y - center) * (2.2 + u_scene.z * 2.2));
  float secondCenter = -0.42 + sin(uv.x * 1.05 - drift * 0.7) * 0.22;
  float second = exp(-abs(uv.y - secondCenter) * 4.0) * 0.45;
  float veil = (curtain + second) * smoothstep(1.5, 0.1, abs(uv.x));
  return paletteField(uv.x * 0.11 + curtainNoise * 0.24 + u_time * 0.008) * veil * (0.34 + u_music.x * 0.34);
}

vec3 prismBeamsVisual(vec2 uv) {
  vec3 light = vec3(0.0);
  int count = 3 + int(round(u_scene.z * 2.0));
  for (int index = 0; index < 5; index++) {
    if (index < count) {
      float fi = float(index);
      float angle = -1.05 + fi * 0.46 + (hash11(fi + 2.0) - 0.5) * 0.18 + sin(u_time * 0.08 + fi) * 0.04;
      vec2 direction = vec2(cos(angle), sin(angle));
      vec2 normal = vec2(-direction.y, direction.x);
      vec2 origin = vec2(-1.35 + fi * 0.16, -0.72 + hash11(fi + 8.0) * 0.35);
      float along = dot(uv - origin, direction);
      float across = abs(dot(uv - origin, normal));
      float width = 0.018 + fi * 0.004 + u_music.w * 0.012;
      float beam = smoothstep(width * 3.5, width, across) * smoothstep(-0.08, 0.18, along) * smoothstep(3.0, 0.5, along);
      light += paletteField(fi * 0.17 + u_time * 0.006) * beam * (0.24 + u_pulse.y * 0.2);
    }
  }
  return light;
}

vec3 kaleidoscopeVisual(vec2 uv) {
  float radius = length(uv);
  float turns = atan(uv.y, uv.x) / TAU;
  float folded = abs(fract(turns * 6.0 + 0.5) - 0.5) * 2.0;
  float petal = pow(max(0.0, 1.0 - abs(sin(folded * 3.14159265 + radius * 7.0 - u_time * 0.24))), 7.0);
  float ring = pow(max(0.0, 1.0 - abs(sin(radius * 8.0 - u_time * 0.34))), 9.0);
  float mask = petal * (0.35 + ring * 0.65) * smoothstep(1.35, 0.2, radius);
  return paletteField(turns * 2.0 + radius * 0.24 + u_time * 0.01) * mask * (0.38 + u_music.x * 0.46);
}

vec3 starTrailsVisual(vec2 uv) {
  vec3 light = vec3(0.0);
  for (int index = 0; index < 18; index++) {
    float fi = float(index);
    float phase = hash11(fi + 1.0);
    float speed = 0.025 + hash11(fi + 7.0) * 0.035;
    float x = fract(phase + u_time * speed) * 3.6 - 1.8;
    float y = hash11(fi + 13.0) * 1.9 - 0.95 + sin(u_time * 0.06 + fi) * 0.08;
    vec2 delta = uv - vec2(x, y);
    float point = exp(-dot(delta, delta) * 1200.0);
    float trail = exp(-abs(delta.y) * 85.0) * smoothstep(0.22, 0.0, delta.x) * smoothstep(-0.02, -0.42, delta.x);
    light += paletteField(phase + u_time * 0.004) * (point + trail * 0.08) * (0.22 + u_music.w * 0.35);
  }
  return light;
}

vec3 ribbonFlowVisual(vec2 uv) {
  vec3 light = vec3(0.0);
  for (int index = 0; index < 3; index++) {
    float fi = float(index);
    float center = sin(uv.x * (1.2 + fi * 0.33) + u_time * (0.16 + fi * 0.025) + fi * 2.1) * (0.28 + fi * 0.04) + (fi - 1.0) * 0.22;
    float distance = abs(uv.y - center);
    float ribbon = smoothstep(0.085 + u_scene.z * 0.03, 0.012, distance);
    float edge = smoothstep(0.08, 0.035, distance) - smoothstep(0.035, 0.012, distance);
    light += paletteField(uv.x * 0.1 + fi * 0.24 + u_time * 0.008) * (ribbon * 0.16 + edge * 0.42);
  }
  return light * (0.52 + u_music.y * 0.36);
}

vec3 technoGridVisual(vec2 uv) {
  float turn = u_time * (0.34 + u_visual.x * 0.24);
  float floorDepth = max(uv.y + 1.08, 0.09);
  vec2 projected = vec2(uv.x / floorDepth, 1.0 / floorDepth + turn * 0.8);
  float floorGate = smoothstep(-1.18, -0.72, uv.y) * (1.0 - smoothstep(-0.05, 0.38, uv.y));
  float columns = pow(max(0.0, 1.0 - abs(sin(projected.x * 3.9))), 12.0);
  float rows = pow(max(0.0, 1.0 - abs(sin(projected.y * 2.15))), 12.0);
  float grid = max(columns * 0.72, rows) * floorGate;
  float sweep = sin(turn * 0.72) * 0.3;
  float leftBeam = exp(-abs(uv.x + 0.86 - (uv.y + 0.92) * (0.48 + sweep)) * 64.0);
  float rightBeam = exp(-abs(uv.x - 0.86 + (uv.y + 0.92) * (0.48 - sweep)) * 64.0);
  float centerBeam = exp(-abs(uv.x - sin(turn) * (uv.y + 1.05) * 0.38) * 82.0);
  float upperGate = smoothstep(-1.2, -0.78, uv.y) * (1.0 - smoothstep(0.94, 1.42, uv.y));
  float lasers = ((leftBeam + rightBeam) * 0.58 + centerBeam * 0.42) * upperGate;
  float frameWidth = 0.58 + sin(turn * 0.35) * 0.08;
  float frame = exp(-abs(abs(uv.x) - frameWidth) * 72.0) * smoothstep(-0.78, -0.38, uv.y) * (1.0 - smoothstep(0.64, 1.02, uv.y));
  return paletteField(projected.y * 0.045 + projected.x * 0.025) * grid * 0.48
    + paletteField(0.24 + uv.y * 0.12 + turn * 0.02) * lasers * 0.44
    + paletteField(0.68 + turn * 0.01) * frame * 0.5;
}

float subjectMusicScale() {
  float beatBreath = sin(u_pulse.x * TAU) * u_music.y * 0.025;
  return clamp(
    0.91 + u_music.x * 0.065 + u_music.y * 0.045 + u_pulse.y * 0.09
      + u_reactive.x * 0.11 + u_effects.y * 0.055 + beatBreath,
    0.88,
    1.28
  );
}

vec3 spinningAlienVisual(vec2 uv) {
  vec2 point = uv / subjectMusicScale();
  float turn = u_time * (0.22 + u_visual.x * 0.12);
  float facing = cos(turn);
  float side = sin(turn);
  float frontGate = smoothstep(-0.18, 0.24, facing);
  float faceWidth = 0.27 + abs(facing) * 0.73;
  vec2 face = vec2(point.x / faceWidth, point.y + 0.015);
  float chinTaper = 1.0 + max(-face.y + 0.03, 0.0) * 1.08;
  float headDistance = length(vec2(face.x * chinTaper, (face.y - 0.055) * 0.92));
  float head = 1.0 - smoothstep(0.555, 0.59, headDistance);
  float rim = exp(-abs(headDistance - 0.573) * 58.0) * head;
  float surfaceLight = clamp(0.56 + face.y * 0.16 - face.x * side * 0.25 + facing * 0.055, 0.2, 0.88);
  float backShade = 0.62 + frontGate * 0.38;
  vec3 skin = paletteField(0.27 + face.y * 0.1 - side * 0.055);
  vec3 color = skin * head * surfaceLight * backShade * (0.45 + u_styleB.y * 0.1);
  color += mix(u_colorC, u_colorD, 0.42 + side * 0.18) * rim * 0.2;
  float featureShift = side * 0.028;
  vec2 leftPoint = rotate2(face - vec2(-0.205 + featureShift, 0.075), -0.2);
  vec2 rightPoint = rotate2(face - vec2(0.205 + featureShift, 0.075), 0.2);
  float leftEyeShape = 1.0 - smoothstep(0.145, 0.175, length(leftPoint * vec2(1.15, 0.78)));
  float rightEyeShape = 1.0 - smoothstep(0.145, 0.175, length(rightPoint * vec2(1.15, 0.78)));
  float leftEye = leftEyeShape * clamp(0.78 - side * 0.5, 0.08, 1.0) * frontGate;
  float rightEye = rightEyeShape * clamp(0.78 + side * 0.5, 0.08, 1.0) * frontGate;
  float eyes = clamp(leftEye + rightEye, 0.0, 1.0) * head;
  color = mix(color, vec3(0.006, 0.008, 0.014), eyes * 0.985);
  float leftGloss = exp(-length(leftPoint - vec2(-0.045, 0.035)) * 86.0) * leftEye;
  float rightGloss = exp(-length(rightPoint - vec2(-0.045, 0.035)) * 86.0) * rightEye;
  color += vec3(0.74, 0.88, 1.0) * (leftGloss + rightGloss) * 0.52;
  vec2 mouthPoint = face - vec2(featureShift * 0.4, -0.285);
  float mouth = (1.0 - smoothstep(0.055, 0.085, length(mouthPoint * vec2(1.0, 1.75)))) * frontGate * head;
  color = mix(color, vec3(0.012, 0.015, 0.02), mouth * 0.92);
  float crownGloss = exp(-length(face - vec2(-0.16, 0.34)) * 8.5) * head * (0.35 + frontGate * 0.65);
  color += vec3(0.22, 0.34, 0.31) * crownGloss * 0.11;
  return color;
}

vec3 spinningSkullVisual(vec2 uv) {
  vec2 point = uv / subjectMusicScale();
  float turn = u_time * (0.19 + u_visual.x * 0.1);
  float facing = cos(turn);
  float side = sin(turn);
  float frontGate = smoothstep(-0.2, 0.23, facing);
  float faceWidth = 0.31 + abs(facing) * 0.69;
  vec2 face = vec2(point.x / faceWidth, point.y + 0.005);
  float craniumDistance = length(vec2(face.x * 0.86, (face.y - 0.14) * 1.04));
  float cranium = 1.0 - smoothstep(0.5, 0.535, craniumDistance);
  float cheekWidth = 0.34 - max(-face.y - 0.08, 0.0) * 0.22;
  float cheekDistance = max(abs(face.x) - cheekWidth, abs(face.y + 0.18) - 0.25);
  float cheek = 1.0 - smoothstep(0.0, 0.035, cheekDistance);
  float jawWidth = 0.245 - max(-face.y - 0.32, 0.0) * 0.18;
  float jawDistance = max(abs(face.x) - jawWidth, abs(face.y + 0.36) - 0.16);
  float jaw = 1.0 - smoothstep(0.0, 0.035, jawDistance);
  float silhouette = max(cranium, max(cheek, jaw));
  float rim = max(exp(-abs(craniumDistance - 0.518) * 58.0) * cranium, exp(-abs(min(cheekDistance, jawDistance)) * 58.0) * max(cheek, jaw));
  vec3 bone = paletteField(0.52 + face.y * 0.085 - side * 0.045);
  float surfaceLight = clamp(0.52 + face.y * 0.18 - face.x * side * 0.28, 0.18, 0.86);
  vec3 color = bone * silhouette * surfaceLight * (0.36 + frontGate * 0.16 + u_styleB.y * 0.06);
  color += mix(u_colorC, u_colorD, 0.5 + side * 0.17) * rim * 0.18;
  float featureShift = side * 0.026;
  vec2 leftPoint = face - vec2(-0.205 + featureShift, 0.1);
  vec2 rightPoint = face - vec2(0.205 + featureShift, 0.1);
  float leftSocket = (1.0 - smoothstep(0.13, 0.165, length(leftPoint * vec2(1.0, 1.28)))) * clamp(0.8 - side * 0.5, 0.08, 1.0) * frontGate;
  float rightSocket = (1.0 - smoothstep(0.13, 0.165, length(rightPoint * vec2(1.0, 1.28)))) * clamp(0.8 + side * 0.5, 0.08, 1.0) * frontGate;
  float nose = (1.0 - smoothstep(0.055, 0.092, abs(face.x - featureShift * 0.45) + abs(face.y + 0.09) * 0.63)) * frontGate;
  vec2 mouthPoint = face - vec2(featureShift * 0.32, -0.33);
  float mouthCavity = (1.0 - smoothstep(0.13, 0.165, length(mouthPoint * vec2(1.0, 2.1)))) * frontGate * jaw;
  float cavities = clamp(leftSocket + rightSocket + nose + mouthCavity, 0.0, 1.0) * silhouette;
  color = mix(color, vec3(0.006, 0.007, 0.011), cavities * 0.97);
  float teethGate = mouthCavity * smoothstep(-0.405, -0.34, face.y);
  float toothBars = step(0.52, fract((face.x - featureShift * 0.32 + 0.2) * 15.0));
  color += bone * toothBars * teethGate * 0.12;
  float crownGloss = exp(-length(face - vec2(-0.14, 0.34)) * 8.0) * cranium * (0.3 + frontGate * 0.7);
  color += vec3(0.38, 0.42, 0.46) * crownGloss * 0.1;
  return color;
}

vec3 watchingEyeVisual(vec2 uv) {
  float turn = u_time * (0.14 + u_visual.x * 0.08);
  vec2 p = rotate2(uv, sin(turn * 0.37) * 0.08);
  float normalizedX = p.x / 0.84;
  float lidHeight = 0.31 * sqrt(max(0.0, 1.0 - normalizedX * normalizedX));
  float horizontalGate = 1.0 - smoothstep(0.8, 0.86, abs(p.x));
  float eye = (1.0 - smoothstep(lidHeight, lidHeight + 0.028, abs(p.y))) * horizontalGate;
  float lid = exp(-abs(abs(p.y) - lidHeight) * 58.0) * horizontalGate;
  vec2 gaze = vec2(sin(turn) * 0.16, sin(turn * 0.73) * 0.075);
  float irisDistance = length(p - gaze);
  float iris = (1.0 - smoothstep(0.205, 0.245, irisDistance)) * eye;
  float pupil = (1.0 - smoothstep(0.065, 0.105, irisDistance)) * eye;
  float highlight = exp(-length(p - gaze - vec2(-0.07, 0.075)) * 92.0) * iris;
  return paletteField(0.08 + p.x * 0.08) * eye * (1.0 - iris * 0.68) * (1.0 - pupil * 0.9) * 0.2
    + paletteField(0.62 + irisDistance * 0.4 - turn * 0.012) * iris * (1.0 - pupil) * 0.56
    + mix(u_colorC, u_colorD, 0.5) * lid * 0.42
    + vec3(0.72, 0.86, 1.0) * highlight * 0.3;
}

vec3 triangleWeights(vec2 point, vec2 first, vec2 second, vec2 third) {
  vec2 edgeA = second - first;
  vec2 edgeB = third - first;
  vec2 local = point - first;
  float denominator = edgeA.x * edgeB.y - edgeB.x * edgeA.y;
  float secondWeight = (local.x * edgeB.y - edgeB.x * local.y) / denominator;
  float thirdWeight = (edgeA.x * local.y - local.x * edgeA.y) / denominator;
  return vec3(1.0 - secondWeight - thirdWeight, secondWeight, thirdWeight);
}

vec3 morphingPyramidVisual(vec2 uv) {
  float turn = u_time * (0.16 + u_visual.x * 0.09);
  vec2 p = rotate2(uv, sin(turn * 0.31) * 0.12);
  vec2 apex = vec2(sin(turn) * 0.16, 0.68);
  vec2 left = vec2(-0.72, -0.56 + sin(turn * 0.73) * 0.035);
  vec2 right = vec2(0.72, -0.56 - sin(turn * 0.73) * 0.035);
  vec3 weights = triangleWeights(p, apex, left, right);
  float boundary = min(weights.x, min(weights.y, weights.z));
  float pyramid = smoothstep(-0.02, 0.018, boundary);
  float outerEdge = exp(-abs(boundary) * 48.0) * pyramid;
  float innerScale = 0.42 + sin(turn * 0.62) * 0.045;
  vec3 innerWeights = triangleWeights(p, apex * innerScale + vec2(0.0, -0.03), left * innerScale + vec2(0.0, -0.03), right * innerScale + vec2(0.0, -0.03));
  float innerBoundary = min(innerWeights.x, min(innerWeights.y, innerWeights.z));
  float hollow = smoothstep(-0.02, 0.02, innerBoundary);
  float sideMix = smoothstep(-0.06, 0.06, p.x - apex.x * 0.35);
  vec3 face = mix(u_colorB, u_colorC, sideMix);
  face = mix(face, u_colorD, (1.0 - smoothstep(-0.48, -0.16, p.y)) * 0.55);
  return face * pyramid * (1.0 - hollow * 0.78) * 0.4
    + paletteField(0.72 + turn * 0.01) * outerEdge * 0.42
    + paletteField(0.24 - turn * 0.008) * exp(-abs(innerBoundary) * 52.0) * pyramid * 0.24;
}

vec3 tumblingCubeVisual(vec2 uv) {
  float turn = u_time * (0.13 + u_visual.x * 0.08);
  vec2 p = rotate2(uv, turn * 0.24 + sin(turn * 0.4) * 0.08);
  float size = 0.56;
  float hexField = max(abs(p.y), abs(p.x) * 0.866 + abs(p.y) * 0.5);
  float cube = 1.0 - smoothstep(size, size + 0.035, hexField);
  float upperFace = smoothstep(-0.025, 0.025, p.y - abs(p.x) * 0.575);
  float rightFace = smoothstep(-0.025, 0.025, p.x);
  vec3 face = mix(u_colorB, u_colorC, rightFace);
  face = mix(face, u_colorD, upperFace * 0.82);
  float outerEdge = exp(-abs(hexField - size) * 48.0);
  float upperSeam = exp(-abs(p.x) * 72.0) * smoothstep(-0.02, 0.16, p.y);
  float lowerSeam = exp(-abs(p.y + abs(p.x) * 0.575) * 68.0) * (1.0 - smoothstep(-0.04, 0.08, p.y));
  return face * cube * 0.32
    + paletteField(0.3 + p.y * 0.14 + turn * 0.01) * outerEdge * 0.48
    + paletteField(0.78 - p.x * 0.1) * (upperSeam + lowerSeam) * cube * 0.2;
}

vec3 kineticBarsVisual(vec2 uv) {
  float turn = u_time * (0.28 + u_visual.x * 0.2);
  float broadBend = sin(uv.y * 3.2 + turn * 0.62) * 0.19;
  float fineBend = sin(uv.y * 8.5 - turn * 0.94) * 0.045;
  float phase = (uv.x + broadBend + fineBend) * (15.0 + u_scene.z * 7.0);
  float bars = smoothstep(0.42, 0.72, sin(phase) * 0.5 + 0.5);
  float edge = pow(max(0.0, 1.0 - abs(sin(phase))), 15.0);
  float counter = pow(max(0.0, 1.0 - abs(sin((uv.x - broadBend * 0.45) * 8.0 + uv.y * 3.0 + turn * 0.36))), 13.0);
  return paletteField(uv.y * 0.14 + broadBend * 0.32 + turn * 0.008) * (bars * 0.32 + edge * 0.42)
    + paletteField(0.55 + uv.x * 0.1 - turn * 0.006) * counter * (1.0 - bars) * 0.18;
}

vec3 bulgingCheckerVisual(vec2 uv) {
  float turn = u_time * (0.25 + u_visual.x * 0.18);
  float radius = length(uv);
  float bulge = 1.0 + exp(-radius * radius * 2.35) * (0.54 + sin(turn * 0.42) * 0.09);
  vec2 p = uv * bulge;
  float scale = 4.5 + u_scene.z * 2.6;
  vec2 cell = fract(p * scale + vec2(turn * 0.07, -turn * 0.055));
  float tile = abs(step(0.5, cell.x) - step(0.5, cell.y));
  float edgeX = pow(max(0.0, 1.0 - abs(sin(p.x * scale * 3.14159265 + turn * 0.22))), 14.0);
  float edgeY = pow(max(0.0, 1.0 - abs(sin(p.y * scale * 3.14159265 - turn * 0.18))), 14.0);
  float lens = exp(-abs(radius - (0.46 + sin(turn * 0.38) * 0.08)) * 27.0);
  vec3 low = paletteField(0.62 - radius * 0.12 - turn * 0.006) * 0.08;
  vec3 high = paletteField(tile * 0.3 + radius * 0.16 + turn * 0.008) * 0.48;
  return mix(low, high, tile) + paletteField(p.x * 0.09 - p.y * 0.07) * (max(edgeX, edgeY) * 0.24 + lens * 0.32);
}

vec3 visualFamily(int id, vec2 uv) {
  if (id == 0) return wavesVisual(uv);
  if (id == 1) return bloomVisual(uv);
  if (id == 2) return pulseVisual(uv);
  if (id == 3) return tunnelVisual(uv);
  if (id == 4) return ribbonFlowVisual(uv);
  if (id == 5) return prismBeamsVisual(uv);
  if (id == 6) return starTrailsVisual(uv);
  if (id == 7) return kaleidoscopeVisual(uv);
  if (id == 8) return technoGridVisual(uv);
  if (id == 9) return spinningAlienVisual(uv);
  if (id == 10) return kineticBarsVisual(uv);
  if (id == 11) return bulgingCheckerVisual(uv);
  if (id == 12) return spinningSkullVisual(uv);
  if (id == 13) return watchingEyeVisual(uv);
  if (id == 14) return morphingPyramidVisual(uv);
  if (id == 15) return tumblingCubeVisual(uv);
  return technoGridVisual(uv);
}

bool isAnchoredSubject(int id) {
  return id == 9 || id == 12;
}

void main() {
  vec2 resolution = max(u_resolution, vec2(1.0));
  vec2 stableUv = (gl_FragCoord.xy * 2.0 - resolution) / resolution.y;
  vec2 uv = stableUv;
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
  int primaryId = int(round(u_styleA.x));
  int secondaryId = int(round(u_styleA.y));
  vec2 primaryUv = isAnchoredSubject(primaryId) ? stableUv : uv;
  vec2 secondaryUv = isAnchoredSubject(secondaryId) ? stableUv : uv;
  vec3 color = visualFamily(primaryId, primaryUv) * u_styleA.z;
  if (u_styleA.w > 0.001) color += visualFamily(secondaryId, secondaryUv) * u_styleA.w;
  float anchoredWeight = clamp(
    (primaryId == 9 || primaryId == 12 ? u_styleA.z : 0.0)
      + (secondaryId == 9 || secondaryId == 12 ? u_styleA.w : 0.0),
    0.0,
    1.0
  );
  float freeScene = 1.0 - anchoredWeight;
  vec2 presentationUv = mix(uv, stableUv, anchoredWeight);
  float vignette = smoothstep(1.48, 0.22, length(presentationUv * vec2(0.68, 1.0)));
  color *= 0.3 + vignette * 0.82;
  color *= u_visual.w * (0.88 + u_pulse.y * 0.14 + bassHit * 0.16 + energyRise * 0.3) * (1.0 + overdrive * (0.12 + hitForce * 0.38));
  color += paletteField(u_time * 0.035 * u_effects.z) * u_pulse.z * (0.045 + overdrive * 0.2) * freeScene;
  color += paletteField(u_time * 0.014) * u_effects.x * (0.018 + overdrive * 0.055) * freeScene;
  float sparkle = modifierStrength(3.0);
  float sparkleDrive = max(sparkle, overdrive * (0.18 + u_music.w * 0.82));
  vec2 sparkleCell = floor((uv + u_time * vec2(0.17, -0.11)) * (34.0 + overdrive * 18.0));
  float sparkleSeed = hash21(sparkleCell);
  float sparkleMask = step(0.985 - u_music.w * 0.02 - overdrive * 0.028, sparkleSeed) * pow(max(0.0, sin(u_time * (8.0 + overdrive * 8.0) + sparkleSeed * TAU)), 10.0);
  color += paletteField(sparkleSeed) * sparkleMask * sparkleDrive * (0.08 + u_music.w * 0.22 + overdrive * 0.18) * freeScene;
  float chromatic = modifierStrength(6.0);
  float chromaticDrive = max(chromatic, overdrive * (0.18 + u_pulse.z * 0.7 + u_music.w * 0.3));
  float edge = min(0.28, length(fwidth(color)));
  color += vec3(edge, edge * 0.18, edge * 0.82) * chromaticDrive * (0.35 + u_music.w * 0.3 + overdrive * 0.5) * freeScene;
  float impactBloom = modifierStrength(7.0);
  float impactLevel = clamp(u_effects.y, 0.0, 1.0);
  float impactDrive = max(impactBloom, overdrive * smoothstep(0.28, 0.82, impactLevel));
  float impactFront = exp(-abs(length(uv) - (0.18 + impactLevel * 0.9)) * 18.0);
  float impactEcho = exp(-abs(length(uv) - (0.1 + impactLevel * 0.62)) * 26.0);
  color += paletteField(length(uv) * 0.25 + u_time * 0.02) * (impactFront + impactEcho * overdrive * 0.72) * impactDrive * (0.28 + overdrive * 0.34) * freeScene;
  float responseRadius = length(uv);
  float responseAngle = atan(uv.y, uv.x);
  float bassFront = exp(-abs(responseRadius - (0.12 + fract(u_pulse.x + bassHit * 0.08) * 1.18)) * 20.0);
  color += paletteField(responseAngle / TAU + responseRadius * 0.42) * bassFront * bassHit * (0.22 + u_styleB.y * 0.32) * freeScene;
  float midRibs = pow(max(0.0, 1.0 - abs(sin((uv.x + uv.y * 0.74) * (7.0 + u_music.z * 7.0) + u_time * 0.9))), 10.0);
  color += paletteField(uv.x * 0.21 - uv.y * 0.13 + u_music.z * 0.24) * midRibs * midMotion * (0.08 + u_music.z * 0.13) * freeScene;
  float shardCount = 16.0 + floor(u_music.w * 14.0);
  float highRayPhase = responseAngle * shardCount + sin(responseRadius * 7.0 - u_time) * 1.15 + u_time * (2.2 + highHit * 3.4);
  float highRay = pow(max(0.0, 1.0 - abs(sin(highRayPhase))), 16.0) * (1.0 - smoothstep(0.72, 1.9, fwidth(highRayPhase)));
  float highGatePhase = responseRadius * 19.0 - u_time * 1.8 + u_styleB.z * TAU;
  float highGate = 0.28 + pow(max(0.0, 1.0 - abs(sin(highGatePhase))), 11.0) * (1.0 - smoothstep(0.72, 1.9, fwidth(highGatePhase))) * 0.72;
  float highShard = highRay * highGate * smoothstep(0.08, 0.34, responseRadius) * (1.0 - smoothstep(1.1, 1.58, responseRadius));
  color += paletteField(responseAngle / TAU * 4.0 + responseRadius * 0.3) * highShard * highHit * (0.24 + u_styleB.y * 0.24) * freeScene;
  vec3 spectralTint = paletteField(responseAngle / TAU + u_music.y * 0.12 + u_music.z * 0.28 + u_music.w * 0.46);
  float colorReaction = clamp(bassHit * 0.16 + midMotion * 0.12 + highHit * 0.27 + u_pulse.z * 0.18, 0.0, 0.55);
  color = mix(color, color * (0.52 + spectralTint * 1.58), colorReaction);
  float radius = length(uv);
  float angle = atan(uv.y, uv.x);
  float wildRays = pow(max(0.0, sin(angle * 12.0 + u_time * (2.8 + u_music.x * 4.0))), 10.0);
  float wildRings = pow(max(0.0, 1.0 - abs(sin(radius * (10.0 + u_scene.z * 8.0) - u_time * (2.0 + u_music.x * 5.0) - u_pulse.x * TAU))), 9.0);
  float wildGeometry = max(wildRays * 0.65, wildRings) * overdrive * (0.04 + u_music.x * 0.18 + hitForce * 0.18);
  color += paletteField(angle / TAU * 3.0 + radius * 0.4 + u_time * 0.08 * u_effects.z) * wildGeometry * freeScene;
  color = mix(color, color.gbr, overdrive * u_pulse.z * 0.16 * freeScene);
  color = mix(color, vec3(1.0), u_pulse.w * (0.68 + overdrive * 0.16));
  float luminance = dot(color, vec3(0.2126, 0.7152, 0.0722));
  float luminanceBudget = 0.68 + u_scene.w * 0.36 + overdrive * 0.14;
  if (luminance > luminanceBudget) color *= luminanceBudget / max(luminance, 0.001);
  color = mix(vec3(dot(color, vec3(0.2126, 0.7152, 0.0722))), color, u_styleB.x);
  color = 1.0 - exp(-max(color, vec3(0.0)) * (1.2 + overdrive * 0.28));
  color = pow(max(color, vec3(0.0)), vec3(0.94));
  if (u_styleB.w > 0.5) color = vec3(0.0);
  fragColor = vec4(color, 1.0);
}`;
