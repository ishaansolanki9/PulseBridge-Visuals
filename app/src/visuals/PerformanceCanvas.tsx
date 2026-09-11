import { useEffect, useRef, useState } from "react";

import { envelope, intensityValues, paletteColors } from "./model";
import { beginPreviewProgram, deletePreviewProgram, pollPreviewProgram } from "./previewProgram";
import type { PreviewProgram } from "./previewProgram";
import { previewSize, startPreviewLoop } from "./previewBudget";
import { SpatialPreview } from "./SpatialPreview";
import { tronScenes, spatialScenes, sceneMatchesDimension } from "./types";
import type { VisualSettings } from "./types";

interface PerformanceCanvasProps {
  settings: VisualSettings;
  className?: string;
  paused?: boolean;
}

export function PerformanceCanvas(props: PerformanceCanvasProps) {
  const selected = spatialScenes.find((scene) => scene.id === props.settings.scene);
  const family = selected?.family ?? (props.settings.scene === "auto" && props.settings.dimension === "threeD" ? 49 : undefined);
  if (family !== undefined) return <SpatialPreview settings={props.settings} family={family} className={props.className ?? ""} paused={props.paused ?? false} />;
  return <AmbientShaderCanvas {...props} />;
}

// Scene cycling is a CPU decision. The GPU compiles at most one look at a time,
// and blends two small programs only during a collection dissolve.
export function previewLayers(settings: VisualSettings, clock: number): Array<[number, number]> {
  const family = tronScenes.find((scene) => scene.id === settings.scene)?.family ?? 3;
  if (family !== 32) return [[family, 1]];
  const looks = tronScenes.filter((scene) => scene.family !== 32 && sceneMatchesDimension(scene.family, settings.dimension));
  const chapter = clock / 8;
  const index = Math.floor(chapter);
  const phase = Math.max(0, Math.min(1, ((chapter % 1) - 0.8) / 0.2));
  const blend = phase * phase * (3 - 2 * phase);
  const current = looks[index % looks.length].family;
  return blend > 0 ? [[current, 1 - blend], [looks[(index + 1) % looks.length].family, blend]] : [[current, 1]];
}

function AmbientShaderCanvas({ settings, className = "", paused = false }: PerformanceCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const settingsRef = useRef(settings);
  const [unavailable, setUnavailable] = useState(false);
  settingsRef.current = settings;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || paused || unavailable) return;
    let gl: WebGL2RenderingContext | null = null;
    let vao: WebGLVertexArrayObject | null = null;
    let completionStatus = 0;
    let elapsed = 0;
    let tronClock = 0;
    const programs = new Map<number, PreviewProgram>();
    const smoothed = { colors: paletteColors(settingsRef.current.palette).map((color) => [...color]) };
    const fail = (reason: unknown) => {
      console.warn("Ambient preview disabled:", reason);
      canvas.dataset.previewState = "unavailable";
      setUnavailable(true);
      stopLoop();
    };
    const contextLost = (event: Event) => {
      event.preventDefault();
      fail("Graphics context lost");
    };
    canvas.addEventListener("webglcontextlost", contextLost);
    canvas.dataset.previewState = "loading";

    // Defer initialization so the first paint and React's development-mode
    // setup/cleanup finish before any GPU resources or shaders are allocated.
    const stopLoop = startPreviewLoop(canvas, (_now, delta) => {
      try {
        if (!gl) {
          gl = canvas.getContext("webgl2", { antialias: false, alpha: false, depth: false, powerPreference: "low-power" });
          if (!gl) throw new Error("WebGL 2 unavailable");
          const parallel = gl.getExtension("KHR_parallel_shader_compile");
          if (!parallel) throw new Error("Nonblocking shader compilation unavailable");
          completionStatus = parallel.COMPLETION_STATUS_KHR;
          vao = gl.createVertexArray();
          gl.bindVertexArray(vao);
          gl.enable(gl.BLEND);
          gl.blendFunc(gl.ONE, gl.ONE);
        }
        if (gl.isContextLost()) return;
        elapsed += delta;
        const currentSettings = settingsRef.current;
        tronClock += delta * 0.33 * currentSettings.motion;
        const layers = previewLayers(currentSettings, tronClock);
        for (const pending of programs.values()) pollPreviewProgram(gl, pending, completionStatus);
        const missing = layers.find(([family]) => !programs.has(family));
        if (missing && [...programs.values()].every((program) => program.ready)) {
          // Bound memory and compiler work even when the scene selector is scrubbed.
          if (programs.size >= 3) {
            const unused = [...programs.keys()].find((family) => !layers.some(([id]) => id === family));
            if (unused !== undefined) {
              deletePreviewProgram(gl, programs.get(unused)!);
              programs.delete(unused);
            }
          }
          programs.set(missing[0], beginPreviewProgram(gl, missing[0]));
        }
        const readyLayers = layers.filter(([family]) => programs.get(family)?.ready);
        if (!readyLayers.length) { canvas.dataset.previewState = "loading"; return; }
        const [width, height] = previewSize(canvas.clientWidth, canvas.clientHeight);
        if (canvas.width !== width || canvas.height !== height) { canvas.width = width; canvas.height = height; }
        gl.viewport(0, 0, width, height);
        gl.clearColor(0, 0, 0, 1);
        gl.clear(gl.COLOR_BUFFER_BIT);
        const targetColors = paletteColors(currentSettings.palette);
        smoothed.colors = smoothed.colors.map((color, colorIndex) =>
          color.map((value, channel) => envelope(value, targetColors[colorIndex][channel], delta, 0.85, 0.85)),
        );
        const intensities = intensityValues(currentSettings.intensity);
        const drive = 0.12;
        for (const [family, weight] of readyLayers) {
          const program = programs.get(family)!;
          gl.useProgram(program.program);
          const location = program.uniform;
          const uniforms = {
            resolution: location("u_resolution"),
            time: location("u_time"),
            tronTime: location("u_tronTime"),
            dimension: location("u_dimension"),
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
          gl.uniform1f(location("u_opacity"), readyLayers.length === 1 ? 1 : weight);
          gl.uniform1f(uniforms.tronTime, tronClock);
          const energy = 0.16;
          const bass = 0.14;
          const mids = 0.18;
          const highs = 0.08;
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
          gl.uniform4f(uniforms.styleA, family, family, 1, 0);
          gl.uniform4f(uniforms.styleB, 0.82 + drive * 0.18, drive, 0.37, 0);
          gl.uniform4f(uniforms.effects, 0.12, 0, currentSettings.colorChange * (0.8 + drive * 1.7), drive);
          gl.uniform4f(uniforms.scene, intensities[0], intensities[1], intensities[1] * 0.62, intensities[2] * 0.82);
          gl.uniform4f(uniforms.modifiers, 0, currentSettings.colorChange * 0.28, -1, 0);
          gl.uniform4f(uniforms.reactive, 0, 0, 0, 0);
          gl.drawArrays(gl.TRIANGLES, 0, 3);
        }
        canvas.dataset.previewState = "ready";
      } catch (reason) {
        fail(reason);
      }
    });
    return () => {
      stopLoop();
      canvas.removeEventListener("webglcontextlost", contextLost);
      if (gl) {
        for (const program of programs.values()) deletePreviewProgram(gl, program);
        gl.deleteVertexArray(vao);
      }
    };
  }, [paused, unavailable]);

  return (
    <div className={`performance-canvas ${className}`}>
      <canvas ref={canvasRef} className="performance-canvas" aria-hidden="true" />
      {unavailable && <span className="preview-unavailable" role="status">Preview unavailable · controls are ready</span>}
    </div>
  );
}
