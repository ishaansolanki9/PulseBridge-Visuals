import type { IntensityProfile, PaletteName } from "./types";

export function intensityValues(profile: IntensityProfile): [number, number, number, number] {
  if (profile === "chill") return [0.66, 0.62, 0.72, 0.45];
  if (profile === "wild") return [1.35, 1.4, 1.25, 1];
  return [1.04, 0.98, 1, 0.78];
}

export function paletteColors(name: PaletteName): number[][] {
  const resolved = name === "auto" ? "ocean" : name;
  const palettes: Record<Exclude<PaletteName, "auto">, number[][]> = {
    electric: [[0.01, 0.08, 0.16], [0, 0.95, 0.88], [0.14, 0.24, 1], [0.74, 0.05, 1]],
    neon: [[0.04, 0, 0.12], [1, 0.02, 0.5], [0.08, 0.96, 0.86], [0.58, 0.1, 1]],
    sunset: [[0.12, 0.01, 0.12], [1, 0.12, 0.08], [1, 0.62, 0.02], [0.8, 0.04, 0.48]],
    ocean: [[0, 0.03, 0.1], [0, 0.28, 0.68], [0, 0.8, 0.78], [0.22, 0.08, 0.72]],
    infrared: [[0.06, 0, 0], [0.68, 0, 0.02], [1, 0.16, 0], [1, 0.62, 0.04]],
    purpleBlue: [[0.015, 0, 0.09], [0.18, 0.08, 0.82], [0.48, 0.12, 1], [0.02, 0.5, 1]],
    warm: [[0.09, 0.015, 0], [0.82, 0.08, 0.01], [1, 0.38, 0.02], [1, 0.78, 0.18]],
    monochrome: [[0.005, 0.008, 0.012], [0.08, 0.1, 0.13], [0.5, 0.56, 0.62], [0.92, 0.96, 1]],
    rainbowFlow: [[0.08, 0, 0.18], [0.95, 0.05, 0.45], [0.02, 0.86, 0.75], [0.98, 0.62, 0.03]],
  };
  return palettes[resolved];
}

export function envelope(current: number, target: number, delta: number, attack: number, release: number) {
  const timeConstant = target > current ? attack : release;
  const amount = 1 - Math.exp(-delta / Math.max(0.001, timeConstant));
  return current + (target - current) * amount;
}
