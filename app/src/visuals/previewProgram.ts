import { previewFragmentShader, vertexShader } from "./shader";

export interface PreviewProgram {
  program: WebGLProgram;
  shaders: WebGLShader[];
  startedAt: number;
  ready: boolean;
  uniform: (name: string) => WebGLUniformLocation | null;
}

// Never fall back to synchronous status queries: Windows ANGLE can spend a long
// time translating shaders while the controller's JS thread waits on the driver.
export function beginPreviewProgram(gl: WebGL2RenderingContext, family: number): PreviewProgram {
  const program = gl.createProgram();
  if (!program) throw new Error("Unable to create preview program");
  const shaders: WebGLShader[] = [];
  try {
    for (const [type, source] of [[gl.VERTEX_SHADER, vertexShader], [gl.FRAGMENT_SHADER, previewFragmentShader(family)]] as const) {
      const shader = gl.createShader(type);
      if (!shader) throw new Error("Unable to create preview shader");
      shaders.push(shader);
      gl.shaderSource(shader, source);
      gl.compileShader(shader);
      gl.attachShader(program, shader);
    }
    gl.linkProgram(program);
  } catch (error) {
    shaders.forEach((shader) => gl.deleteShader(shader));
    gl.deleteProgram(program);
    throw error;
  }
  const locations = new Map<string, WebGLUniformLocation | null>();
  return {
    program, shaders, ready: false, startedAt: performance.now(),
    uniform(name) {
      if (!locations.has(name)) locations.set(name, gl.getUniformLocation(program, name));
      return locations.get(name) ?? null;
    },
  };
}

export function pollPreviewProgram(gl: WebGL2RenderingContext, pending: PreviewProgram, completionStatus: number): boolean {
  if (pending.ready) return true;
  if (!gl.getProgramParameter(pending.program, completionStatus)) {
    if (performance.now() - pending.startedAt > 8000) throw new Error("Preview shader compilation timed out");
    return false;
  }
  if (!gl.getProgramParameter(pending.program, gl.LINK_STATUS)) {
    throw new Error(gl.getProgramInfoLog(pending.program) ?? "Preview shader link failed");
  }
  pending.shaders.forEach((shader) => {
    gl.detachShader(pending.program, shader);
    gl.deleteShader(shader);
  });
  pending.shaders = [];
  pending.ready = true;
  return true;
}

export function deletePreviewProgram(gl: WebGL2RenderingContext, pending: PreviewProgram) {
  pending.shaders.forEach((shader) => gl.deleteShader(shader));
  gl.deleteProgram(pending.program);
}
