// B039: the Excalidraw comparison page's entry. scripts/ink-excalidraw-build.sh bundles this
// with the pinned @excalidraw/excalidraw 0.18.1 and React 18.3.1 (esbuild from nixpkgs) into
// dist/, so the page runs offline from the static server test/run-chrome.mjs starts. The page
// mounts the editor, exposes its API on window.openlogosExcalidraw, drives the freedraw tool
// with the same synthesized pointer sequence the ink prototype measures, and reads back the
// freedraw elements to show how Excalidraw's stroke model maps onto the ink-document schema.
import React from "react";
import { createRoot } from "react-dom/client";
import { Excalidraw, exportToSvg, serializeAsJSON } from "@excalidraw/excalidraw";
import "@excalidraw/excalidraw/index.css";

window.EXCALIDRAW_ASSET_PATH = "./dist/";

// Excalidraw calls setPointerCapture on pointerdown; for a synthesized PointerEvent there is no
// active pointer and the browser throws, which aborts the handler. The synthesized runs need
// the capture calls to be no-ops; a real pointer never reaches these (the ids exist). This is
// the one page-level change for the measurement and it does not touch Excalidraw's timing.
if (new URLSearchParams(location.search).get("autorun") === "1") {
  Element.prototype.setPointerCapture = function () {};
  Element.prototype.releasePointerCapture = function () {};
}

const params = new URLSearchParams(location.search);
const results = { ready: false };
window.openlogosExcalidraw = results;

function App() {
  return (
    <Excalidraw
      excalidrawAPI={(api) => { results.api = api; results.ready = true; }}
      langCode={params.get("lang") || "en"}
      theme={params.get("theme") === "light" ? "light" : "dark"}
      initialData={{ appState: { currentItemStrokeColor: "#1d4ed8", currentItemStrokeWidth: 1, viewBackgroundColor: "transparent", zenModeEnabled: false } }}
      UIOptions={{ canvasActions: { loadScene: false, saveToActiveFile: false } }}
    />
  );
}

createRoot(document.getElementById("app")).render(<App />);

const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));

function interactiveCanvas() {
  return document.querySelector("canvas.excalidraw__canvas.interactive") || document.querySelector(".excalidraw__canvas");
}

function dispatch(canvas, type, x, y, pressure, pointerType = "pen", pointerId = 7) {
  const rect = canvas.getBoundingClientRect();
  const init = { pointerType, pointerId, isPrimary: true, clientX: rect.left + x, clientY: rect.top + y, screenX: rect.left + x, screenY: rect.top + y, pressure, button: 0, buttons: type === "pointerup" ? 0 : 1, bubbles: true, cancelable: true, composed: true, view: window };
  const event = new PointerEvent(type, init);
  // Dispatched on the interactive canvas and bubbling to window, where Excalidraw's drag
  // listeners live; the React handlers on the canvas see it on the way.
  canvas.dispatchEvent(event);
  return event;
}

function percentile(values, p) { if (!values.length) return null; const s = [...values].sort((a, b) => a - b); return s[Math.min(s.length - 1, Math.floor(p * s.length))]; }
function stats(values) { const v = values.filter(Number.isFinite); const r = (x) => (x == null ? null : Math.round(x * 100) / 100); return { n: v.length, p50: r(percentile(v, 0.5)), p95: r(percentile(v, 0.95)), max: v.length ? r(Math.max(...v)) : null }; }

/** The same loop as apps/web/ink/ink.js measure(): 120 pen samples with pressure, 4 ms apart.
 *  Excalidraw renders on its own rAF; we record dispatch time and the rAF timestamps after it. */
async function measure(points = 120) {
  const api = results.api;
  api.setActiveTool({ type: "freedraw" });
  await sleep(100);
  const canvas = interactiveCanvas();
  const rect = canvas.getBoundingClientRect();
  const cx = rect.width * 0.5, cy = rect.height * 0.45, rx = Math.min(rect.width, rect.height) * 0.3, ry = rx * 0.6;
  const at = (i) => { const a = (i / points) * Math.PI * 2.4; return [cx + Math.cos(a) * rx + i * 0.6, cy + Math.sin(a) * ry, 0.3 + 0.5 * Math.sin((i / points) * Math.PI)]; };
  const samples = [];
  let frameTimes = [];
  let running = true;
  const onFrame = (t) => { frameTimes.push(t); if (running) requestAnimationFrame(onFrame); };
  requestAnimationFrame(onFrame);
  dispatch(canvas, "pointerdown", ...at(0));
  for (let i = 1; i < points; i++) {
    await sleep(4);
    const t0 = performance.now();
    dispatch(canvas, "pointermove", ...at(i));
    samples.push({ dispatched: t0, handlerDone: performance.now() });
  }
  dispatch(canvas, "pointerup", ...at(points - 1));
  await new Promise((ok) => requestAnimationFrame(() => requestAnimationFrame(ok)));
  await sleep(50);
  running = false;
  // For each sample: the first rAF timestamp after dispatch is when Excalidraw's frame began;
  // the one after that is the earliest the drawn pixels can be on screen (as in ink.js).
  const after = (t, n) => { const i = frameTimes.findIndex((f) => f > t); return i >= 0 && i + n - 1 < frameTimes.length ? frameTimes[i + n - 1] : null; };
  return {
    label: "excalidraw freedraw, synthesized pen",
    samples: samples.length,
    frames_observed: frameTimes.length,
    dispatch_to_handler_return_ms: stats(samples.map((s) => s.handlerDone - s.dispatched)),
    input_to_first_frame_ms: stats(samples.map((s) => { const f = after(s.dispatched, 1); return f == null ? NaN : f - s.dispatched; })),
    input_to_next_frame_ms: stats(samples.map((s) => { const f = after(s.dispatched, 2); return f == null ? NaN : f - s.dispatched; })),
    note: "Excalidraw handles pointermove synchronously (handler return time includes its element update) and paints on its own requestAnimationFrame; input_to_next_frame is comparable to ink.js's input_to_next_frame (the frame after the first one that could draw), observed from outside the component.",
  };
}

/** The freedraw elements as Excalidraw stores them, and how they map onto schemas/ink-document.schema.json. */
function model() {
  const elements = results.api.getSceneElements().filter((e) => e.type === "freedraw");
  const first = elements[0];
  const keys = first ? Object.keys(first) : [];
  const mapping = {
    stroke_id: "element.id (nanoid)",
    z_order: "element.index (fractional index string) and scene order",
    tombstone: "element.isDeleted (soft delete kept for sync)",
    points: "element.points: [dx, dy] relative to element.x/y, no timestamps",
    pressure: "element.pressures: number[] parallel to points, empty when simulatePressure is true (pointer gave none)",
    simulate_pressure: "element.simulatePressure: boolean",
    time: "none per point; element.updated is a whole-element timestamp",
    tilt: "none",
    brush_definition_version: "none on the element; freedraw geometry is Excalidraw's perfect-freehand 1.2.0 call with options derived from strokeWidth (size = strokeWidth * 4.25, thinning 0.6, smoothing 0.5, streamline 0.5, easing sine, last on pointerup) in the package version; the editor version (element.version is an edit counter, not a brush version) must be recorded by the embedder",
    color: "element.strokeColor (CSS)",
    opacity: "element.opacity 0..100",
    width: "element.strokeWidth (1, 2, 4 in the UI), the rendered size is a multiple",
    eraser: "the eraser removes whole elements (isDeleted: true); no mask operations; no pixel eraser",
    coordinate_space: "scene coordinates with appState.scrollX/scrollY/zoom as the viewport, no snapshot binding",
    native_blob: "none; serializeAsJSON() is open JSON of these fields",
    export: "exportToSvg / exportToBlob (PNG) / serializeAsJSON in the package",
  };
  return {
    freedraw_elements: elements.length,
    element_keys: keys,
    first_element_summary: first ? { id: first.id, index: first.index, points: first.points.length, pressures: first.pressures.length, simulatePressure: first.simulatePressure, strokeColor: first.strokeColor, strokeWidth: first.strokeWidth, opacity: first.opacity, isDeleted: first.isDeleted, version: first.version, updated: first.updated, roughness: first.roughness, seed: first.seed } : null,
    mapping,
  };
}

async function exportSizes() {
  const api = results.api;
  const elements = api.getSceneElements();
  const json = serializeAsJSON(elements, api.getAppState(), api.getFiles(), "local");
  const svg = await exportToSvg({ elements, appState: { ...api.getAppState(), exportBackground: false }, files: api.getFiles() });
  return { json_bytes: json.length, svg_bytes: svg.outerHTML.length, svg_has_paths: svg.querySelectorAll("path").length };
}

function rtl() {
  const root = document.querySelector(".excalidraw");
  // Excalidraw sets the document direction itself when the loaded language is RTL.
  return { lang: params.get("lang") || "en", document_dir: document.documentElement.dir || null, document_lang: document.documentElement.lang || null, root_class: root ? root.className : null, console: window.openlogosConsole.slice(0, 10), toolbar_sample: document.querySelector(".ToolIcon__label, .help-icon, [data-testid='main-menu-trigger']")?.getAttribute("aria-label") || null };
}

results.measure = measure;
results.model = model;
results.exportSizes = exportSizes;
results.rtl = rtl;

window.openlogosConsole = [];
for (const level of ["error", "warn"]) {
  const original = console[level].bind(console);
  console[level] = (...args) => { window.openlogosConsole.push(`${level}: ${args.map(String).join(" ").slice(0, 300)}`); original(...args); };
}
window.addEventListener("error", (e) => window.openlogosConsole.push(`uncaught: ${e.message}`));
window.addEventListener("unhandledrejection", (e) => window.openlogosConsole.push(`rejection: ${e.reason}`));

(async () => {
  for (let i = 0; i < 200 && !results.api; i++) await sleep(50);
  if (!results.api) { window.openlogosError = "Excalidraw API never arrived"; return; }
  await sleep(300);
  results.version = (await import("@excalidraw/excalidraw/package.json").catch(() => ({ default: { version: "0.18.1 (pinned)" } }))).default.version;
  if (params.get("autorun") === "1") {
    const measurement = await measure();
    const touch = await (async () => { results.api.setActiveTool({ type: "freedraw" }); return null; })();
    const debug = {
      canvases: [...document.querySelectorAll("canvas")].map((c) => ({ className: c.className, w: c.width, h: c.height })),
      activeTool: results.api.getAppState().activeTool,
      elementsIncludingDeleted: results.api.getSceneElementsIncludingDeleted().length,
      excalidrawRootClasses: document.querySelector(".excalidraw")?.className,
      htmlDir: document.documentElement.dir, bodyDir: document.body.dir,
      lang: results.api.getAppState().langCode || null,
      console: window.openlogosConsole.slice(0, 20),
    };
    results.results = { measurement, model: model(), export: await exportSizes(), rtl: rtl(), touch, debug };
    results.api.setActiveTool({ type: "selection" });
  }
  window.openlogosReady = true;
})();
