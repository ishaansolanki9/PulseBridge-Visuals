import { useEffect, useRef } from "react";

import { envelope, intensityValues, paletteColors } from "./model";
import { fragmentShader, vertexShader } from "./shader";
import type { VisualSettings } from "./types";

interface PerformanceCanvasProps {
  settings: VisualSettings;
  className?: string;
  paused?: boolean;
}

interface SmoothedState {
  primaryFamily: number;
  secondaryFamily: number;
  transition: number;
  colors: number[][];
}

const ambientPreviewFamilies = [8, 9, 10, 11] as const;

export function PerformanceCanvas({ settings, className = "", paused = false }: PerformanceCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const settingsRef = useRef(settings);
  settingsRef.current = settings;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || paused) return;
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
      scene: location("u_scene"),
      modifiers: location("u_modifiers"),
      reactive: location("u_reactive"),
    };
    const startedAt = performance.now();
    let lastFrame = startedAt;
    let animationFrame = 0;
    const smoothed: SmoothedState = {
      primaryFamily: ambientPreviewFamilies[0],
      secondaryFamily: ambientPreviewFamilies[0],
      transition: 0,
      colors: paletteColors(settings.palette).map((color) => [...color]),
    };

    const render = (now: number) => {
      if (now - lastFrame < 50) {
        animationFrame = requestAnimationFrame(render);
        return;
      }
      const elapsed = (now - startedAt) / 1000;
      const delta = Math.max(0.001, Math.min(0.1, (now - lastFrame) / 1000));
      lastFrame = now;
      const currentSettings = settingsRef.current;
      const targetFamily = ambientPreviewFamilies[Math.floor(elapsed / 10) % ambientPreviewFamilies.length];
      const targetColors = paletteColors(currentSettings.palette);
      const intensities = intensityValues(currentSettings.intensity);
      const drive = 0.12;

      if (targetFamily !== smoothed.primaryFamily && targetFamily !== smoothed.secondaryFamily) {
        smoothed.secondaryFamily = targetFamily;
        smoothed.transition = 0;
      }
      if (smoothed.secondaryFamily !== smoothed.primaryFamily) {
        smoothed.transition = envelope(smoothed.transition, 1, delta, 0.8, 0.8);
        if (smoothed.transition > 0.995) {
          smoothed.primaryFamily = smoothed.secondaryFamily;
          smoothed.transition = 0;
        }
      }
      smoothed.colors = smoothed.colors.map((color, colorIndex) =>
        color.map((value, channel) => envelope(value, targetColors[colorIndex][channel], delta, 0.85, 0.85)),
      );

      const isOutput = Boolean(canvas.closest(".performance-output"));
      const pixelRatio = Math.min(window.devicePixelRatio, isOutput ? 1.25 : 1);
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
        (0.12 + drive * 1.38) * intensities[0] * currentSettings.motion,
        (0.22 + drive * 0.78) * intensities[1],
        0.92 + drive * 0.34 + bass * 0.12,
        (0.3 + drive * 0.7) * intensities[2] * currentSettings.brightness,
      );
      gl.uniform3fv(uniforms.colorA, smoothed.colors[0]);
      gl.uniform3fv(uniforms.colorB, smoothed.colors[1]);
      gl.uniform3fv(uniforms.colorC, smoothed.colors[2]);
      gl.uniform3fv(uniforms.colorD, smoothed.colors[3]);
      const secondaryMix = smoothed.secondaryFamily === smoothed.primaryFamily ? 0 : smoothed.transition;
      gl.uniform4f(uniforms.styleA, smoothed.primaryFamily, smoothed.secondaryFamily, 1 - secondaryMix, secondaryMix);
      gl.uniform4f(uniforms.styleB, 0.82 + drive * 0.18, drive, 0.37, 0);
      gl.uniform4f(uniforms.effects, 0.12, 0, currentSettings.colorChange * (0.8 + drive * 1.7), drive);
      gl.uniform4f(uniforms.scene, intensities[0], intensities[1], intensities[1] * 0.62, intensities[2] * 0.82);
      gl.uniform4f(uniforms.modifiers, 0, currentSettings.colorChange * 0.28, -1, 0);
      gl.uniform4f(uniforms.reactive, 0, 0, 0, 0);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
      animationFrame = requestAnimationFrame(render);
    };

    animationFrame = requestAnimationFrame(render);
    return () => {
      cancelAnimationFrame(animationFrame);
      gl.deleteVertexArray(vao);
      gl.deleteProgram(program);
    };
  }, [paused]);

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
