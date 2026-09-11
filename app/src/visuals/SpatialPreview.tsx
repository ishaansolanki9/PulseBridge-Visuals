import { useEffect, useRef } from "react";
import { paletteColors } from "./model";
import { previewSize, startPreviewLoop } from "./previewBudget";
import type { VisualSettings } from "./types";

const TAU = Math.PI * 2;
const rotate = (x: number, y: number, a: number): [number, number] => [Math.cos(a) * x - Math.sin(a) * y, Math.sin(a) * x + Math.cos(a) * y];

// Quiet camera/geometry counterparts of spatial.wgsl. The controller intentionally
// has no audio source; live deformation is rendered by the native GPU output.
function point(family: number, strand: number, u: number, clock: number): [number, number, number] {
  const x = (u * 2 - 1) * 6.8;
  const row = strand / 47 * 2 - 1;
  if (family === 26) return strand >= 32 ? [(strand - 32) / 15 * 13.6 - 6.8, (u * 2 - 1) * 3.6, 6.5] : [x, (strand / 31 * 2 - 1) * 3.6, 6.5];
  if (family === 27) {
    const a = strand / 48 * TAU + x * 0.58 + clock * 0.3;
    return [x, Math.sin(a) * 0.9, 6.2 + Math.cos(a) * 1.15];
  }
  if (family === 28) {
    const a = u * TAU, c = Math.cos(a), s = Math.sin(a), m = Math.max(Math.abs(c), Math.abs(s));
    return [c / m * 3.5, s / m * 2.5, 2.6 + strand * 0.63];
  }
  if (family === 29) return [x, row * 2.4 + Math.sin(x * 0.68 + strand * 0.09 + clock * 0.25) * 0.2, 6 + strand / 48 * 2.2];
  if (family === 30) {
    const a = u * TAU, sector = (a + Math.PI / 8) % (TAU / 8) - Math.PI / 8;
    const r = (0.36 + strand * 0.075) / Math.cos(sector);
    const xy = rotate(Math.cos(a) * r, Math.sin(a) * r, clock * 0.08);
    return [xy[0] * 1.5, xy[1], 6 + strand * 0.018];
  }
  if (family === 31) {
    const gx = strand >= 32 ? (strand - 32) / 15 * 20.4 - 10.2 : x * 1.5;
    const gz = strand >= 32 ? 2 + u * 23.5 : 2 + strand * 0.5;
    return [gx, -1.8 + Math.sin(gx * 0.65 + gz * 0.5 - clock * 0.25) * 0.15, gz];
  }
  if (family === 49) {
    const ring = Math.floor(strand / 4), rail = strand % 4, a = u * TAU, r = 1.4 + rail * 0.07;
    const xy = rotate(Math.cos(a) * r, Math.sin(a) * r, ring * 0.4 + clock * 0.16);
    return [xy[0], xy[1] * Math.cos(ring * 0.6), 5.8 + xy[1] * Math.sin(ring * 0.6) + (ring - 5.5) * 0.2];
  }
  if (family === 50) {
    const a = strand * 2.39996 + clock * 0.12, b = Math.acos(1 - (strand + 0.5) / 24), r = 0.3 + u * 2;
    return [Math.cos(a) * Math.sin(b) * r, Math.sin(a) * Math.sin(b) * r, 5.6 + Math.cos(b) * r];
  }
  if (family === 51) {
    const cable = Math.floor(strand / 12), fiber = strand % 12, a = fiber / 12 * TAU + x * 0.8 + clock * 0.3;
    const braid = x * 0.55 + cable * Math.PI * 0.5;
    return [x, Math.sin(braid) * 1.1 + Math.sin(a) * 0.22, 6.5 + Math.cos(braid) * 1.7 + Math.cos(a) * 0.22];
  }
  const cage = Math.floor(strand / 4), rib = strand % 4, a = u * TAU, c = Math.cos(a), s = Math.sin(a);
  const xy = rotate(c / Math.max(Math.abs(c), Math.abs(s)) * 0.65, s / Math.max(Math.abs(c), Math.abs(s)) * 0.65, cage * 0.38 + clock * 0.18);
  return [xy[0] + Math.sin(cage * 2.4 + clock * 0.1) * 1.8 + (rib - 1.5) * 0.18, xy[1] + Math.cos(cage * 2.4) * 1.8, 3.5 + cage * 0.6 + (rib - 1.5) * 0.18];
}

export function SpatialPreview({ settings, family, className, paused }: { settings: VisualSettings; family: number; className: string; paused: boolean }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const current = useRef({ settings, family });
  current.current = { settings, family };
  useEffect(() => {
    const element = canvas.current;
    if (!element || paused) return;
    const ctx = element.getContext("2d");
    if (!ctx) return;
    let clock = 0;
    return startPreviewLoop(element, (_now, delta) => {
      const { settings: active, family: id } = current.current;
      clock += delta * 0.33 * active.motion;
      const [width, height] = previewSize(element.clientWidth, element.clientHeight);
      if (element.width !== width || element.height !== height) { element.width = width; element.height = height; }
      ctx.fillStyle = "#02050c"; ctx.fillRect(0, 0, width, height);
      const colors = paletteColors(active.palette).slice(1);
      for (let strand = 0; strand < 48; strand++) {
        const color = colors[Math.floor((strand / 16 + clock * active.colorChange * 0.1) % colors.length)];
        ctx.strokeStyle = `rgb(${color.map((v) => Math.round(v * 255)).join(",")})`;
        ctx.shadowColor = ctx.strokeStyle; ctx.shadowBlur = 4; ctx.lineWidth = 1.15;
        ctx.globalAlpha = Math.min(1, active.brightness * 0.85);
        ctx.beginPath();
        for (let segment = 0; segment <= 96; segment++) {
          const [x, y, z] = point(id, strand, segment / 96, clock);
          const px = width * 0.5 + x * 0.9 * height / Math.max(z, 0.8);
          const py = height * 0.5 - y * 0.9 * height / Math.max(z, 0.8);
          if (segment === 0) ctx.moveTo(px, py); else ctx.lineTo(px, py);
        }
        ctx.stroke();
      }
      ctx.shadowBlur = 0; ctx.globalAlpha = 1;
    });
  }, [paused]);
  return <canvas ref={canvas} className={`performance-canvas ${className}`} aria-label="Ambient 3D scene preview" />;
}
