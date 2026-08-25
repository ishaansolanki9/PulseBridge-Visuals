import { useEffect, useRef } from "react";

import { envelope, intensityValues, paletteColors, styleWeights } from "./model";
import { fragmentShader, vertexShader } from "./shader";
import type { VisualSettings } from "./types";

interface PerformanceCanvasProps {
  settings: VisualSettings;
  className?: string;
}

interface SmoothedState {
  styles: [number, number, number, number, number];
  colors: number[][];
}

export function PerformanceCanvas({ settings, className = "" }: PerformanceCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const settingsRef = useRef(settings);
  settingsRef.current = settings;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const gl = canvas.getContext("webgl2", {
      antialias: false,
      alpha: false,
      depth: false,
      powerPreference: "high-performance",
    });
    if (!gl) throw new Error("WebGL 2 is required for the ambient preview");

    const program = createProgram(gl, vertexShader, fragmentShader);
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    gl.useProgram(program);
    const location = (name: string) => gl.getUniformLocation(program, name);
    const uniforms = {
      resolution: location("u_resolution"),
      time: location("u_time"),
      music: location("u_music"),
      pulse: location("u_pulse"),
      visual: location("u_visual"),
      colorA: location("u_colorA"),
      colorB: location("u_colorB"),
      colorC: location("u_colorC"),
      colorD: location("u_colorD"),
      styleA: location("u_styleA"),
      styleB: location("u_styleB"),
      effects: location("u_effects"),
    };
    const startedAt = performance.now();
    let lastFrame = startedAt;
    let animationFrame = 0;
    const smoothed: SmoothedState = {
      styles: styleWeights(settings.style),
      colors: paletteColors(settings.palette).map((color) => [...color]),
    };

    const render = (now: number) => {
      const elapsed = (now - startedAt) / 1000;
      const delta = Math.max(0.001, Math.min(0.1, (now - lastFrame) / 1000));
      lastFrame = now;
      const currentSettings = settingsRef.current;
      const targetStyles = styleWeights(currentSettings.style);
      const targetColors = paletteColors(currentSettings.palette);
      const intensities = intensityValues(currentSettings.intensity);

      smoothed.styles = smoothed.styles.map((value, index) =>
        envelope(value, targetStyles[index], delta, 0.45, 1.1),
      ) as SmoothedState["styles"];
      const styleTotal = smoothed.styles.reduce((sum, value) => sum + value, 0) || 1;
      smoothed.styles = smoothed.styles.map((value) => value / styleTotal) as SmoothedState["styles"];
      smoothed.colors = smoothed.colors.map((color, colorIndex) =>
        color.map((value, channel) => envelope(value, targetColors[colorIndex][channel], delta, 0.85, 0.85)),
      );

      const isOutput = Boolean(canvas.closest(".performance-output"));
      const pixelRatio = Math.min(window.devicePixelRatio, isOutput ? 2 : 1.35);
      const width = Math.max(1, Math.round(canvas.clientWidth * pixelRatio));
      const height = Math.max(1, Math.round(canvas.clientHeight * pixelRatio));
      if (canvas.width !== width || canvas.height !== height) {
        canvas.width = width;
        canvas.height = height;
      }

      const energy = 0.16;
      const bass = 0.14;
      const mids = 0.18;
      const highs = 0.08;
      gl.viewport(0, 0, width, height);
      gl.uniform2f(uniforms.resolution, width, height);
      gl.uniform1f(uniforms.time, elapsed);
      gl.uniform4f(uniforms.music, energy, bass, mids, highs);
      gl.uniform4f(uniforms.pulse, 0, 0, 0, 0);
      gl.uniform4f(
        uniforms.visual,
        (0.18 + energy * 0.82) * intensities[0] * currentSettings.motion,
        (0.15 + highs * 0.85) * intensities[1],
        0.9 + bass * 0.28,
        (0.34 + energy * 0.72) * intensities[2] * currentSettings.brightness,
      );
      gl.uniform3fv(uniforms.colorA, smoothed.colors[0]);
      gl.uniform3fv(uniforms.colorB, smoothed.colors[1]);
      gl.uniform3fv(uniforms.colorC, smoothed.colors[2]);
      gl.uniform3fv(uniforms.colorD, smoothed.colors[3]);
      gl.uniform4f(uniforms.styleA, ...smoothed.styles.slice(0, 4) as [number, number, number, number]);
      gl.uniform4f(uniforms.styleB, smoothed.styles[4], 0.78 + energy * 0.34, 0, 0);
      gl.uniform4f(uniforms.effects, 0.12, 0, currentSettings.colorChange, 0);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
      animationFrame = requestAnimationFrame(render);
    };

    animationFrame = requestAnimationFrame(render);
    return () => {
      cancelAnimationFrame(animationFrame);
      gl.deleteVertexArray(vao);
      gl.deleteProgram(program);
    };
  }, []);

  return <canvas ref={canvasRef} className={`performance-canvas ${className}`} aria-hidden="true" />;
}

function createProgram(gl: WebGL2RenderingContext, vertexSource: string, fragmentSource: string) {
  const compile = (type: number, source: string) => {
    const shader = gl.createShader(type);
    if (!shader) throw new Error("Unable to allocate GPU shader");
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
      const message = gl.getShaderInfoLog(shader) ?? "Unknown shader compilation error";
      gl.deleteShader(shader);
      throw new Error(message);
    }
    return shader;
  };
  const vertex = compile(gl.VERTEX_SHADER, vertexSource);
  const fragment = compile(gl.FRAGMENT_SHADER, fragmentSource);
  const program = gl.createProgram();
  if (!program) throw new Error("Unable to create GPU program");
  gl.attachShader(program, vertex);
  gl.attachShader(program, fragment);
  gl.linkProgram(program);
  gl.deleteShader(vertex);
  gl.deleteShader(fragment);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    const message = gl.getProgramInfoLog(program) ?? "Unknown GPU program link error";
    gl.deleteProgram(program);
    throw new Error(message);
  }
  return program;
}
