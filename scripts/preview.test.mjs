import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { fileURLToPath } from "node:url";
import { after, before, test } from "node:test";
import { chromium } from "playwright";

const root = fileURLToPath(new URL("../", import.meta.url));
const port = 1427;
const url = `http://127.0.0.1:${port}`;
let server;
let browser;

before(async () => {
  server = spawn(process.execPath, ["../node_modules/vite/bin/vite.js", "--host", "127.0.0.1", "--port", String(port)], {
    cwd: `${root}/app`, stdio: "pipe",
  });
  let serverOutput = "";
  server.stderr.on("data", (data) => { serverOutput += data; });
  const deadline = Date.now() + 15000;
  while (true) {
    try { if ((await fetch(url)).ok) break; } catch { /* Server still starting. */ }
    if (server.exitCode !== null || Date.now() > deadline) throw new Error(`Preview server failed: ${serverOutput}`);
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  browser = await chromium.launch({
    channel: process.env.PULSEBRIDGE_TEST_BROWSER || undefined,
    headless: true,
    args: ["--enable-unsafe-swiftshader"],
  });
});

after(async () => {
  await browser?.close();
  if (server && server.exitCode === null) {
    const exited = once(server, "exit");
    server.kill();
    await exited;
  }
});

async function openPreview(mode = "normal", settings = {}) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 600 } });
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.addInitScript(({ mode, settings }) => {
    localStorage.setItem("pulsebridge-visual-settings", JSON.stringify(settings));
    const probe = window.previewProbe = { shaders: [], draws: 0, contexts: 0, blockingQueries: 0, programs: 0, maxPrograms: 0, stall: mode === "stall", audioRequests: 0, settingsWrites: 0 };
    if (mode === "slow-audio") {
      window.__TAURI_INTERNALS__ = {
        invoke: async (command) => {
          if (command === "get_audio_sources") { probe.audioRequests++; return new Promise(() => {}); }
          if (command === "get_displays") {
            // Runtime polling can finish before startup display discovery does.
            await new Promise((resolve) => setTimeout(resolve, 900));
            return [{ id: 0, name: "Test display", width: 1920, height: 1080, scaleFactor: 1, isPrimary: true }];
          }
          if (command === "get_previous_run_report") return null;
          if (command === "update_visual_settings") { probe.settingsWrites++; return; }
          if (command === "get_runtime_state") {
            const { defaultSettings, stoppedCapture } = await import("/src/visuals/types.ts");
            return {
              running: false, lifecycle: "stopped", lastError: null, logPath: null,
              settings: { ...defaultSettings, dimension: "twoD", scene: "neonMandala" }, audio: stoppedCapture,
              phrase: { provenance: "unavailable", beatConfidence: null },
              renderer: { state: "stopped" }, outputMode: "ambient", reactive: false, audioAgeMs: null,
            };
          }
          throw new Error(`Unexpected native command ${command}`);
        },
      };
    }
    const originalContext = HTMLCanvasElement.prototype.getContext;
    HTMLCanvasElement.prototype.getContext = function (type, ...args) {
      if (type === "webgl2") {
        probe.contexts++;
        if (mode === "no-webgl") return null;
      }
      return originalContext.call(this, type, ...args);
    };
    const prototype = WebGL2RenderingContext.prototype;
    const originalExtension = prototype.getExtension;
    prototype.getExtension = function (name) {
      if (name === "KHR_parallel_shader_compile" && mode === "no-parallel") return null;
      return originalExtension.call(this, name);
    };
    const completed = new WeakSet();
    const originalStatus = prototype.getProgramParameter;
    prototype.getProgramParameter = function (program, parameter) {
      if (parameter === 0x91B1 && probe.stall) return false;
      if (parameter === this.LINK_STATUS && !completed.has(program)) probe.blockingQueries++;
      const value = originalStatus.call(this, program, parameter);
      if (parameter === 0x91B1 && value) completed.add(program);
      if (parameter === this.LINK_STATUS && mode === "link-failure") return false;
      return value;
    };
    const originalShaderStatus = prototype.getShaderParameter;
    prototype.getShaderParameter = function (shader, parameter) {
      if (parameter === this.COMPILE_STATUS) probe.blockingQueries++;
      return originalShaderStatus.call(this, shader, parameter);
    };
    const originalSource = prototype.shaderSource;
    prototype.shaderSource = function (shader, source) {
      probe.shaders.push(source);
      return originalSource.call(this, shader, source);
    };
    const originalDraw = prototype.drawArrays;
    prototype.drawArrays = function (...args) { probe.draws++; return originalDraw.apply(this, args); };
    const originalCreate = prototype.createProgram;
    prototype.createProgram = function () {
      probe.maxPrograms = Math.max(probe.maxPrograms, ++probe.programs);
      return originalCreate.call(this);
    };
    const originalDelete = prototype.deleteProgram;
    prototype.deleteProgram = function (program) { probe.programs--; return originalDelete.call(this, program); };
  }, { mode, settings });
  await page.goto(url);
  return { page, errors, close: () => context.close() };
}

test("cold startup compiles only Auto; every held look and dimension stays interactive", async () => {
  const { page, errors, close } = await openPreview();
  try {
    await page.waitForSelector("canvas[data-preview-state=ready]");
    let probe = await page.evaluate(() => window.previewProbe);
    assert.equal(probe.contexts, 1, "StrictMode must not compile the preview twice");
    assert.equal(probe.shaders.length, 2, "Only the selected vertex/fragment pair compiles");
    assert.ok(!probe.shaders.join("").includes("tron_look"), "Auto must not compile Tron");
    const scenes = await page.locator("select").first().locator("option").evaluateAll((options) => options.map((option) => option.value));
    for (const scene of scenes) {
      await page.locator("select").first().selectOption(scene);
      if (scene !== "auto" && !["magneticSwarm", "liquidRelic", "impossibleArchitecture", "auroraVeil", "kineticSculpture", "topographicOcean", "orbitFoundry", "synapseBloom", "gravityBraids", "prismConveyor"].includes(scene)) {
        await page.waitForFunction(() => window.previewProbe.draws > 0 && document.querySelector("canvas[data-preview-state=ready]"));
        // Wait for a frame with the newly selected program, not the prior canvas state.
        const draws = await page.evaluate(() => window.previewProbe.draws);
        await page.waitForFunction((before) => window.previewProbe.draws > before + 1, draws);
      }
      assert.equal(await page.getByRole("status").count(), 0, `Scene ${scene} failed`);
    }
    for (const dimension of ["2D", "3D", "Combined"]) {
      await page.getByRole("button", { name: dimension, exact: true }).click();
      await page.getByRole("button", { name: dimension, exact: true }).getAttribute("aria-pressed").then((value) => assert.equal(value, "true"));
    }
    probe = await page.evaluate(() => window.previewProbe);
    assert.equal(probe.blockingQueries, 0);
    assert.ok(probe.maxPrograms <= 3, "Shader cache must stay bounded");
    assert.ok(probe.shaders.every((source) => !source.includes("tron_scene(")), "No monolithic scene dispatch");
    assert.deepEqual(errors, []);
  } finally { await close(); }
});

test("a saved expensive look uses one specialized program on startup", async () => {
  const { page, errors, close } = await openPreview("normal", { scene: "neonMandala" });
  try {
    await page.waitForSelector("canvas[data-preview-state=ready]");
    const probe = await page.evaluate(() => window.previewProbe);
    assert.equal(probe.shaders.length, 2);
    const fragment = probe.shaders[1];
    assert.ok(fragment.includes("Neon Mandala"));
    assert.ok(!fragment.includes("Grid highway"));
    assert.ok(fragment.length < 5000);
    assert.equal(probe.blockingQueries, 0);
    assert.deepEqual(errors, []);
  } finally { await close(); }
});

for (const mode of ["no-webgl", "no-parallel", "link-failure", "stall"]) {
  test(`${mode}: controls work while the preview is unavailable or compiling`, async () => {
    const { page, errors, close } = await openPreview(mode);
    try {
      await page.getByRole("button", { name: "2D", exact: true }).click();
      await page.locator("select").first().selectOption("neonMandala");
      await page.getByRole("button", { name: "Wild", exact: true }).click();
      await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight));
      assert.ok(await page.evaluate(() => window.scrollY > 0));
      await page.evaluate(() => window.scrollTo(0, 0));
      await page.getByRole("status").waitFor({ timeout: 12000 });
      const probe = await page.evaluate(() => window.previewProbe);
      assert.equal(probe.blockingQueries, 0);
      if (mode === "no-parallel" || mode === "no-webgl") assert.equal(probe.shaders.length, 0);
      if (mode === "stall") assert.equal(probe.shaders.length, 2, "A stalled compiler cannot accumulate more jobs");
      assert.deepEqual(errors, []);
    } finally { await close(); }
  });
}

test("preview draw budget stops offscreen and context loss leaves controls usable", async () => {
  const { page, errors, close } = await openPreview();
  try {
    await page.waitForSelector("canvas[data-preview-state=ready]");
    const canvas = page.locator("canvas");
    assert.ok(await canvas.evaluate((element) => element.width <= 640 && element.height <= 360));
    await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight));
    await page.waitForTimeout(200);
    const draws = await page.evaluate(() => window.previewProbe.draws);
    await page.waitForTimeout(350);
    assert.equal(await page.evaluate(() => window.previewProbe.draws), draws);
    await page.evaluate(() => window.scrollTo(0, 0));
    await page.waitForFunction((before) => window.previewProbe.draws > before, draws);
    await canvas.evaluate((element) => element.getContext("webgl2").getExtension("WEBGL_lose_context").loseContext());
    await page.getByRole("status").waitFor();
    await page.getByRole("button", { name: "2D", exact: true }).click();
    assert.deepEqual(errors, []);
  } finally { await close(); }
});

test("Tron cycle selection covers each compatible look including wraparound", async () => {
  const { page, close } = await openPreview();
  try {
    const result = await page.evaluate(async () => {
      const { previewLayers } = await import("/src/visuals/PerformanceCanvas.tsx");
      const { defaultSettings } = await import("/src/visuals/types.ts");
      return ["combined", "twoD", "threeD"].map((dimension) => {
        const settings = { ...defaultSettings, scene: "tron", dimension };
        const count = dimension === "combined" ? 16 : dimension === "twoD" ? 9 : 7;
        return {
          dimension,
          looks: Array.from({ length: count }, (_, index) => previewLayers(settings, index * 8)[0][0]),
          wrap: previewLayers(settings, count * 8)[0][0],
          fade: previewLayers(settings, (count - 1) * 8 + 7.2),
        };
      });
    });
    assert.deepEqual(result[0].looks, Array.from({ length: 16 }, (_, index) => index + 33));
    assert.deepEqual(result[1].looks, [37, 38, 40, 43, 44, 45, 46, 47, 48]);
    assert.deepEqual(result[2].looks, [33, 34, 35, 36, 39, 41, 42]);
    for (const cycle of result) {
      assert.equal(cycle.wrap, cycle.looks[0]);
      assert.deepEqual(cycle.fade.map(([family]) => family), [cycle.looks.at(-1), cycle.looks[0]]);
      assert.ok(Math.abs(cycle.fade[0][1] + cycle.fade[1][1] - 1) < 1e-9);
    }
  } finally { await close(); }
});

test("slow audio discovery cannot hold startup state or accumulate refresh workers", async () => {
  const { page, errors, close } = await openPreview("slow-audio");
  try {
    await page.waitForSelector("canvas[data-preview-state=ready]");
    assert.equal(await page.getByRole("button", { name: "2D", exact: true }).getAttribute("aria-pressed"), "true");
    assert.equal(await page.getByRole("option", { name: /Test display/ }).count(), 1);
    await page.getByRole("button", { name: "Refresh audio sources" }).click();
    await page.getByRole("button", { name: "Combined", exact: true }).click();
    await page.waitForTimeout(1700);
    const probe = await page.evaluate(() => window.previewProbe);
    assert.equal(probe.audioRequests, 1);
    assert.equal(probe.settingsWrites, 1);
    assert.equal(probe.shaders.length, 2, "Runtime polling must not start Auto before saved settings load");
    assert.ok(probe.shaders[1].includes("Neon Mandala"));
    assert.deepEqual(errors, []);
  } finally { await close(); }
});
