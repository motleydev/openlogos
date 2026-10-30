// B039 web ink prototype: PointerEvents → the Rust perfect-freehand outline (inkOutline in the
// core's wasm module) → one non-zero fill per stroke on a canvas; the strokes are kept as the
// portable InkDocument record and validated by inkDocumentJson on export.
//
// Rules mirrored from the core (crates/ink):
//   - brush.rs and touch.rs: the brushes, the compositing rule and the palm arbiter, in
//     layer.js since B072 so the workspace shell's margin layer draws with the same code.
//   - document.rs (schema 1.1): null pressure when the device reports none, altitude and
//     azimuth from a pen's altitudeAngle/azimuthAngle (radians) and null otherwise (a mouse; a touch or pen whose
//     pressure is the 0.5 placeholder with no real sensor: pointer events cannot tell a real
//     0.5 from the placeholder, so a pen's pressure is kept only when it varies).
//
// Measurement: `?autorun=1` (test/run-chrome.mjs) and the Measure button dispatch a synthesized
// pen stroke with pressure through the same listeners a real pen reaches; every sample records
// dispatch time → handler start → outline done → draw done → the rAF timestamp of the frame
// after the drawing one (the first that can show it). A hand run records real events with
// event.timeStamp as the start. Results on window.openlogosInk for the runner.
import init, * as core from "../core-host/pkg/openlogos_ffi.js";
import { ENGINE, BRUSHES, HIGHLIGHTER_WIDTH_FACTOR, PALM_RADIUS_PX, TouchArbiter, compositeAt, strokeOutline, distanceToSegment } from "./layer.js";

export { PALM_RADIUS_PX, TouchArbiter };

const params = new URLSearchParams(location.search);
const $ = (id) => document.getElementById(id);
const canvas = $("ink");
const ctx = canvas.getContext("2d", { desynchronized: true });
const readingPane = $("reading-pane");

// MARK: Document and history

const doc = {
  schema_version: "1.1", fixture_only: true, id: "web-ink-1", snapshot_id: "fixture:snapshot:1", coordinate_space_id: "fixture:coordinates:1",
  strokes: [], deleted_stroke_ids: [], viewport_transform: { scale: 1, translate_x: 0, translate_y: 0 },
};
const undoStack = [];
const redoStack = [];
let strokeCounter = 0;
const state = { tool: "pen", mode: "stylus", color: "#1d4ed8", width: 4 };
const arbiter = new TouchArbiter(state.mode);
const decisions = []; // the arbiter's log for the readout and the test

// MARK: Rendering

let dpr = 1;
let committed = document.createElement("canvas");
const committedCtx = committed.getContext("2d");
let live = null; // { stroke, points: Float64Array buffer, count, pointerId, started, options, brush }
let frame = 0;

function resize() {
  dpr = window.devicePixelRatio || 1;
  const rect = canvas.getBoundingClientRect();
  canvas.width = Math.max(1, Math.round(rect.width * dpr));
  canvas.height = Math.max(1, Math.round(rect.height * dpr));
  committed.width = canvas.width;
  committed.height = canvas.height;
  repaintCommitted();
  paint();
}

/** One fill of the stroke at the page's device pixel ratio (layer.js composite). */
function composite(target, stroke, flatOutline) { compositeAt(target, stroke, flatOutline, dpr); }

function outlineOf(stroke) { return strokeOutline(core, stroke); }

function repaintCommitted() {
  committedCtx.clearRect(0, 0, committed.width, committed.height);
  const deleted = new Set(doc.deleted_stroke_ids);
  for (const stroke of doc.strokes) {
    if (deleted.has(stroke.id)) continue;
    composite(committedCtx, stroke, outlineOf(stroke));
  }
  $("meta").textContent = `${doc.strokes.length - deleted.size} strokes · ${deleted.size} erased · ${ENGINE}`;
}

function paint() {
  frame += 1;
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  ctx.drawImage(committed, 0, 0);
  if (live && live.count >= 1) {
    const t0 = performance.now();
    const flat = core.inkOutline(live.buffer.subarray(0, live.count * 3), { ...live.options, last: false });
    const t1 = performance.now();
    composite(ctx, live.stroke, flat);
    const t2 = performance.now();
    live.lastOutlineMs = t1 - t0;
    live.lastDrawMs = t2 - t1;
    live.lastOutlinePoints = flat.length / 2;
  }
}

let scheduled = false;
function schedulePaint(sample) {
  if (scheduled) return;
  scheduled = true;
  requestAnimationFrame((drawFrameTime) => {
    scheduled = false;
    const handlerToDrawStart = performance.now();
    paint();
    const drawDone = performance.now();
    if (sample) {
      sample.drawFrameTime = drawFrameTime;
      sample.drawStart = handlerToDrawStart;
      sample.drawDone = drawDone;
      sample.outlineMs = live ? live.lastOutlineMs : 0;
      sample.drawMs = live ? live.lastDrawMs : 0;
      sample.outlinePoints = live ? live.lastOutlinePoints : 0;
      // The frame after the drawing one is the first that can show the drawn pixels.
      requestAnimationFrame((nextFrameTime) => { sample.visibleFrameTime = nextFrameTime; });
    }
  });
}

// MARK: Strokes

function canvasPoint(event) {
  const rect = canvas.getBoundingClientRect();
  return { x: event.clientX - rect.left, y: event.clientY - rect.top };
}

function contactOf(event) {
  return event.pointerType === "pen" ? "stylus" : event.pointerType === "touch" ? "finger" : "pointer";
}

function paneOf(target) {
  if (canvas.contains(target)) return 0;
  if (readingPane.contains(target)) return 1;
  return 2;
}

/** A pen reports pressure in 0..1; a mouse reports 0 while down and 0.5 is the spec placeholder
 *  for devices without a sensor. Pressure is recorded only from a pen. */
function pressureOf(event) {
  return event.pointerType === "pen" ? Math.min(1, Math.max(0, event.pressure)) : null;
}

/** Tilt (schema 1.1): PointerEvent.altitudeAngle (0..π/2) and azimuthAngle (0..2π) exist in
 *  Chromium 86+ and Safari; a pen reports them, anything else gets null. A pen whose angles
 *  are the spec defaults (π/2, 0) on every sample is treated like placeholder pressure below. */
function tiltOf(event) {
  if (event.pointerType !== "pen" || typeof event.altitudeAngle !== "number") return [null, null];
  return [Math.min(Math.PI / 2, Math.max(0, event.altitudeAngle)), ((event.azimuthAngle % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI)];
}

function beginStroke(event, sample) {
  if (state.tool === "eraser") { eraseAt(canvasPoint(event)); return; }
  const brush = BRUSHES[state.tool];
  const width = state.tool === "highlighter" ? state.width * HIGHLIGHTER_WIDTH_FACTOR : state.width;
  const stroke = {
    id: `stroke-${++strokeCounter}`, tool: brush.tool, brush_version: brush.version, width,
    color: state.tool === "highlighter" ? state.color : state.color, opacity: brush.opacity, points: [],
  };
  live = { stroke, buffer: new Float64Array(3 * 4096), count: 0, pointerId: event.pointerId, started: event.timeStamp, brush, hasPressure: event.pointerType === "pen", options: null, pressureVaries: false, firstPressure: null };
  live.options = { ...brush.options, size: width, simulatePressure: brush.options.simulatePressure && !live.hasPressure };
  addSample(event, sample);
}

function addSample(event, sample) {
  if (!live) return;
  const events = typeof event.getCoalescedEvents === "function" && event.getCoalescedEvents().length ? event.getCoalescedEvents() : [event];
  for (const e of events) {
    const { x, y } = canvasPoint(e);
    const pressure = pressureOf(e);
    const [altitude, azimuth] = tiltOf(e);
    if (pressure != null) { if (live.firstPressure == null) live.firstPressure = pressure; else if (pressure !== live.firstPressure) live.pressureVaries = true; }
    live.stroke.points.push({ x, y, pressure, elapsed_ms: Math.max(0, e.timeStamp - live.started), altitude, azimuth });
    if (live.count < 4096) {
      const i = live.count * 3;
      live.buffer[i] = x; live.buffer[i + 1] = y; live.buffer[i + 2] = live.hasPressure && live.brush.usesPressure && pressure != null ? pressure : -1;
      live.count += 1;
    }
  }
  schedulePaint(sample);
}

function endStroke(event, cancelled) {
  if (!live) return;
  if (!cancelled && event) addSample(event, null);
  const stroke = live.stroke;
  live = null;
  if (cancelled || stroke.points.length < 2) { paint(); return; }
  // A pen whose pressure never varied is a device without a sensor reporting the 0.5
  // placeholder: record null rather than a fabricated constant (specs/13).
  if (stroke.points.every((p) => p.pressure != null) && stroke.points.length > 3) {
    const first = stroke.points[0].pressure;
    if (stroke.points.every((p) => p.pressure === first)) for (const p of stroke.points) p.pressure = null;
  }
  // Likewise tilt that never left the spec defaults (altitude π/2, azimuth 0) is no sensor.
  if (stroke.points.every((p) => p.altitude != null && Math.abs(p.altitude - Math.PI / 2) < 1e-9 && p.azimuth === 0)) {
    for (const p of stroke.points) { p.altitude = null; p.azimuth = null; }
  }
  // Monotonic time (coalesced events can carry equal or earlier stamps).
  let t = 0;
  for (const p of stroke.points) { if (p.elapsed_ms < t) p.elapsed_ms = t; t = p.elapsed_ms; }
  doc.strokes.push(stroke);
  undoStack.push({ type: "add", id: stroke.id });
  redoStack.length = 0;
  repaintCommitted();
  paint();
  updateHistoryButtons();
  readout(`stroke ${stroke.id}: ${stroke.tool} ${stroke.points.length} points, width ${stroke.width}, pressure ${stroke.points[0].pressure == null ? "null" : "recorded"}, ${stroke.brush_version}`);
}

/** Stroke eraser: the first live stroke whose centre line passes within its half width (+4) of the point, from the top. */
function eraseAt(p) {
  const deleted = new Set(doc.deleted_stroke_ids);
  for (let i = doc.strokes.length - 1; i >= 0; i--) {
    const s = doc.strokes[i];
    if (deleted.has(s.id)) continue;
    const r = s.width / 2 + 4;
    for (let j = 1; j < s.points.length; j++) {
      if (distanceToSegment(p, s.points[j - 1], s.points[j]) <= r) {
        doc.deleted_stroke_ids.push(s.id);
        undoStack.push({ type: "erase", id: s.id });
        redoStack.length = 0;
        repaintCommitted(); paint(); updateHistoryButtons();
        readout(`erased ${s.id} (tombstone kept in deleted_stroke_ids)`);
        return;
      }
    }
  }
}

function undo() {
  const op = undoStack.pop();
  if (!op) return;
  if (op.type === "add") { const i = doc.strokes.findIndex((s) => s.id === op.id); if (i >= 0) redoStack.push({ type: "add", stroke: doc.strokes.splice(i, 1)[0] }); }
  else { const i = doc.deleted_stroke_ids.lastIndexOf(op.id); if (i >= 0) doc.deleted_stroke_ids.splice(i, 1); redoStack.push({ type: "erase", id: op.id }); }
  repaintCommitted(); paint(); updateHistoryButtons();
}

function redo() {
  const op = redoStack.pop();
  if (!op) return;
  if (op.type === "add") { doc.strokes.push(op.stroke); undoStack.push({ type: "add", id: op.stroke.id }); }
  else { doc.deleted_stroke_ids.push(op.id); undoStack.push({ type: "erase", id: op.id }); }
  repaintCommitted(); paint(); updateHistoryButtons();
}

function updateHistoryButtons() {
  $("undo").disabled = undoStack.length === 0;
  $("redo").disabled = redoStack.length === 0;
}

function readout(text) { $("readout").textContent = text; }

// MARK: Pointer events through the arbiter

const samples = []; // measurement samples of the current run
// `?record=1`: real pointer events are recorded from the start (the hand run in a headed
// browser); `openlogosInk.handReport()` analyses what was drawn so far.
let measuring = params.get("record") === "1" ? { label: "hand run (real pointer events)" } : null;

function touchEvent(event, phase) {
  return { id: event.pointerId, contact: contactOf(event), phase, pane: paneOf(event.target), radius: event.pointerType === "touch" ? Math.max(event.width, event.height) / 2 : null };
}

function logDecision(event, phase, decision) {
  decisions.push({ phase, pointerType: event.pointerType, pane: paneOf(event.target), decision, t: event.timeStamp });
  if (decisions.length > 400) decisions.shift();
}

function onPointerDown(event) {
  const decision = arbiter.decide(touchEvent(event, "began"));
  logDecision(event, "began", decision);
  if (decision === "cancel-stroke-then-draw") { endStroke(null, true); }
  if (decision === "draw" || decision === "cancel-stroke-then-draw") {
    if (paneOf(event.target) !== 0) return;
    event.preventDefault();
    try { canvas.setPointerCapture(event.pointerId); } catch {}
    const sample = measuring ? { dispatched: event.timeStamp, handlerStart: performance.now(), kind: "down" } : null;
    if (sample) samples.push(sample);
    beginStroke(event, sample);
  } else if (decision.startsWith("ignore")) {
    // A palm: nothing happens, and the reading pane must not scroll under it (touchmove is
    // cancelled below while a touch is ignored).
    event.preventDefault();
    if (decision !== "ignore:finger-in-stylus-mode") readout(`${event.pointerType} in pane ${paneOf(event.target)} ignored: ${decision.slice(7)}`);
  }
}

function onPointerMove(event) {
  if (!(event.buttons & 1) && event.pointerType === "mouse") return;
  const decision = arbiter.decide(touchEvent(event, "moved"));
  if (decision === "draw" && live && live.pointerId === event.pointerId) {
    const sample = measuring ? { dispatched: event.timeStamp, handlerStart: performance.now(), kind: "move" } : null;
    if (sample) samples.push(sample);
    addSample(event, sample);
  }
}

function onPointerUp(event) {
  const decision = arbiter.decide(touchEvent(event, event.type === "pointercancel" ? "cancelled" : "ended"));
  logDecision(event, event.type === "pointercancel" ? "cancelled" : "ended", decision);
  if (decision === "draw" && live && live.pointerId === event.pointerId) {
    endStroke(event, event.type === "pointercancel");
  }
}

// The whole page listens: a finger landing on the reading pane mid-stroke reaches the arbiter
// (and is ignored) instead of scrolling the pane; a pointer on the pane with no stroke scrolls
// as usual because the arbiter says `scroll:1` and nothing is prevented.
document.addEventListener("pointerdown", onPointerDown);
document.addEventListener("pointermove", onPointerMove);
document.addEventListener("pointerup", onPointerUp);
document.addEventListener("pointercancel", onPointerUp);
const ignoredTouchIds = () => [...arbiter.live.entries()].filter(([, d]) => String(d).startsWith("ignore")).map(([id]) => id);
document.addEventListener("touchstart", (e) => { if (arbiter.strokeInProgress) e.preventDefault(); }, { passive: false });
document.addEventListener("touchmove", (e) => { if (arbiter.strokeInProgress || ignoredTouchIds().length) e.preventDefault(); }, { passive: false });

// MARK: Controls

for (const button of document.querySelectorAll("[data-tool]")) {
  button.addEventListener("click", () => { state.tool = button.dataset.tool; for (const b of document.querySelectorAll("[data-tool]")) b.setAttribute("aria-pressed", String(b === button)); canvas.style.cursor = state.tool === "eraser" ? "cell" : "crosshair"; });
}
for (const button of document.querySelectorAll("[data-mode]")) {
  button.addEventListener("click", () => { state.mode = button.dataset.mode; arbiter.mode = state.mode; for (const b of document.querySelectorAll("[data-mode]")) b.setAttribute("aria-pressed", String(b === button)); });
}
$("undo").addEventListener("click", undo);
$("redo").addEventListener("click", redo);
$("color").addEventListener("input", (e) => { state.color = e.target.value; });
$("width").addEventListener("input", (e) => { state.width = Number(e.target.value); });
$("export").addEventListener("click", () => {
  try {
    const json = core.inkDocumentJson(doc);
    readout(json.length > 1800 ? `${json.slice(0, 1800)}\n… (${json.length} bytes, valid)` : json);
    window.openlogosInkJson = json;
  } catch (error) { readout(`inkDocumentJson rejected the document: ${error.kind}: ${error.message}`); }
});
$("measure").addEventListener("click", () => { measure().then((r) => readout(summarize(r))); });
document.addEventListener("keydown", (e) => { if ((e.metaKey || e.ctrlKey) && e.key === "z") { e.preventDefault(); if (e.shiftKey) redo(); else undo(); } });

// MARK: Measurement

function dispatch(type, pointerType, x, y, pressure, pointerId = 7) {
  const rect = canvas.getBoundingClientRect();
  const event = new PointerEvent(type, { pointerType, pointerId, isPrimary: true, clientX: rect.left + x, clientY: rect.top + y, pressure, buttons: type === "pointerup" ? 0 : 1, bubbles: true, cancelable: true, composed: true });
  canvas.dispatchEvent(event);
}

const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));

/** A synthesized pen stroke: 120 samples on a loop with pressure, about 4 ms apart (a 240 Hz
 *  pen coalesces to the frame rate; the cadence here is the dispatcher's, not a device's). */
export async function measure({ points = 120, pointerType = "pen", label = "synthesized pen" } = {}) {
  const rect = canvas.getBoundingClientRect();
  const cx = rect.width * 0.5, cy = rect.height * 0.45, rx = Math.min(rect.width, rect.height) * 0.3, ry = rx * 0.6;
  samples.length = 0;
  measuring = { label };
  const previousTool = state.tool;
  const previousMode = arbiter.mode;
  state.tool = "pen";
  if (pointerType === "touch") arbiter.mode = "finger"; // a finger draws only in finger mode
  const at = (i) => { const a = (i / points) * Math.PI * 2.4; return [cx + Math.cos(a) * rx + i * 0.6, cy + Math.sin(a) * ry, 0.3 + 0.5 * Math.sin((i / points) * Math.PI)]; };
  dispatch("pointerdown", pointerType, ...at(0));
  for (let i = 1; i < points; i++) { await sleep(4); dispatch("pointermove", pointerType, ...at(i)); }
  dispatch("pointerup", pointerType, ...at(points - 1));
  await new Promise((ok) => requestAnimationFrame(() => requestAnimationFrame(ok)));
  await sleep(50);
  measuring = null;
  state.tool = previousTool;
  arbiter.mode = previousMode;
  const result = analyse(samples, label);
  window.openlogosInk = { ...(window.openlogosInk || {}), measurement: result, decisions: [...decisions] };
  return result;
}

function percentile(values, p) {
  if (!values.length) return null;
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.floor(p * sorted.length))];
}

function stats(values) {
  const v = values.filter((x) => Number.isFinite(x));
  return { n: v.length, p50: percentile(v, 0.5), p95: percentile(v, 0.95), max: v.length ? Math.max(...v) : null };
}

function analyse(list, label) {
  const painted = list.filter((s) => s.drawDone != null);
  const r = (x) => (x == null ? null : Math.round(x * 100) / 100);
  const round = (o) => Object.fromEntries(Object.entries(o).map(([k, v]) => [k, typeof v === "number" ? r(v) : v]));
  return {
    label, samples: list.length, painted: painted.length, coalesced_into_frames: painted.length,
    input_to_handler_ms: round(stats(list.map((s) => s.handlerStart - s.dispatched))),
    handler_to_draw_done_ms: round(stats(painted.map((s) => s.drawDone - s.handlerStart))),
    outline_call_ms: round(stats(painted.map((s) => s.outlineMs))),
    outline_points: round(stats(painted.map((s) => s.outlinePoints))),
    input_to_draw_frame_ms: round(stats(painted.map((s) => s.drawFrameTime - s.dispatched))),
    input_to_draw_done_ms: round(stats(painted.map((s) => s.drawDone - s.dispatched))),
    input_to_next_frame_ms: round(stats(painted.filter((s) => s.visibleFrameTime != null).map((s) => s.visibleFrameTime - s.dispatched))),
    note: "input_to_next_frame is the rAF timestamp of the frame after the drawing one minus the event time: the earliest the stroke can be on screen, excluding compositor and display scan-out, which no page can observe.",
    user_agent: navigator.userAgent, dpr, canvas_css_px: [Math.round(canvas.getBoundingClientRect().width), Math.round(canvas.getBoundingClientRect().height)],
  };
}

function summarize(r) {
  const f = (s) => (s && s.n ? `p50 ${s.p50} · p95 ${s.p95} · max ${s.max} ms (n=${s.n})` : "–");
  return [`${r.label}: ${r.samples} samples, ${r.painted} painted frames`,
    `input → handler: ${f(r.input_to_handler_ms)}`, `handler → draw done: ${f(r.handler_to_draw_done_ms)}`,
    `inkOutline call: ${f(r.outline_call_ms)} for ${r.outline_points.p50}–${r.outline_points.max} outline points`,
    `input → draw done: ${f(r.input_to_draw_done_ms)}`, `input → next frame (earliest visible): ${f(r.input_to_next_frame_ms)}`].join("\n");
}

/** The palm cases of crates/ink/src/touch.rs run through the JS mirror, as synthesized events on the live page. */
export function palmCases() {
  const results = [];
  const run = (name, mode, stream, expect) => {
    const a = new TouchArbiter(mode);
    const got = stream.map((e) => a.decide(e));
    const passed = expect.every(([i, d]) => got[i] === d);
    results.push({ name, passed, got, expect });
  };
  const ev = (id, contact, phase, pane, radius = contact === "finger" ? 7 : null) => ({ id, contact, phase, pane, radius });
  run("stylus-mode-finger-in-another-pane-during-a-stroke-does-not-scroll-it", "stylus", [
    ev(1, "stylus", "began", 0), ev(1, "stylus", "moved", 0), ev(2, "finger", "began", 1), ev(2, "finger", "moved", 1), ev(1, "stylus", "moved", 0), ev(2, "finger", "moved", 1), ev(1, "stylus", "ended", 0), ev(2, "finger", "moved", 1), ev(2, "finger", "ended", 1), ev(3, "finger", "began", 1), ev(3, "finger", "moved", 1),
  ], [[0, "draw"], [2, "ignore:palm-during-stroke"], [3, "ignore:palm-during-stroke"], [5, "ignore:palm-during-stroke"], [7, "ignore:palm-during-stroke"], [9, "scroll:1"], [10, "scroll:1"]]);
  run("stylus-mode-finger-in-the-canvas-is-ignored-and-scrolls-elsewhere", "stylus", [ev(1, "finger", "began", 0), ev(1, "finger", "ended", 0), ev(2, "finger", "began", 1)], [[0, "ignore:finger-in-stylus-mode"], [2, "scroll:1"]]);
  run("finger-mode-draws-with-one-finger-and-ignores-the-second", "finger", [
    ev(1, "finger", "began", 0), ev(2, "finger", "began", 0), ev(2, "finger", "moved", 0), ev(1, "finger", "moved", 0), ev(3, "finger", "began", 1), ev(1, "finger", "ended", 0), ev(2, "finger", "ended", 0), ev(4, "finger", "began", 1),
  ], [[0, "draw"], [1, "ignore:second-finger"], [2, "ignore:second-finger"], [3, "draw"], [4, "ignore:palm-during-stroke"], [7, "scroll:1"]]);
  run("a-stylus-cancels-a-finger-stroke-and-takes-over", "finger", [ev(1, "finger", "began", 0), ev(2, "stylus", "began", 0), ev(2, "stylus", "moved", 0), ev(1, "finger", "moved", 0)], [[0, "draw"], [1, "cancel-stroke-then-draw"], [2, "draw"], [3, "ignore:unknown"]]);
  run("a-wide-contact-is-a-palm-in-either-mode-even-with-no-stroke", "finger", [ev(9, "finger", "began", 0, PALM_RADIUS_PX), ev(9, "finger", "moved", 0, 6)], [[0, "ignore:large-contact"], [1, "ignore:large-contact"]]);
  run("a-pointer-draws-in-the-canvas-and-scrolls-elsewhere", "stylus", [ev(1, "pointer", "began", 0), ev(1, "pointer", "ended", 0), ev(2, "pointer", "began", 3)], [[0, "draw"], [1, "draw"], [2, "scroll:3"]]);
  return results;
}

/** Synthesized on the live page: a pen stroke in progress, a touch lands on the reading pane
 *  and moves; the pane's scrollTop must not change and the arbiter must say palm. */
export async function livePalmCase() {
  readingPane.scrollTop = 40;
  const before = readingPane.scrollTop;
  const rect = canvas.getBoundingClientRect();
  const paneRect = readingPane.getBoundingClientRect();
  const touch = (type, y, id = 11) => readingPane.dispatchEvent(new PointerEvent(type, { pointerType: "touch", pointerId: id, isPrimary: false, clientX: paneRect.left + 60, clientY: paneRect.top + y, width: 12, height: 12, pressure: 0.5, buttons: 1, bubbles: true, cancelable: true }));
  dispatch("pointerdown", "pen", rect.width * 0.3, rect.height * 0.3, 0.5, 21);
  dispatch("pointermove", "pen", rect.width * 0.35, rect.height * 0.32, 0.6, 21);
  const start = decisions.length;
  touch("pointerdown", 200);
  touch("pointermove", 150);
  touch("pointermove", 100);
  touch("pointerup", 100);
  dispatch("pointermove", "pen", rect.width * 0.4, rect.height * 0.34, 0.6, 21);
  dispatch("pointerup", "pen", rect.width * 0.42, rect.height * 0.35, 0.6, 21);
  await sleep(30);
  const palm = decisions.slice(start).find((d) => d.pointerType === "touch" && d.phase === "began");
  const after = readingPane.scrollTop;
  // The last stroke was synthetic; drop it so the picture stays the user's.
  undo();
  return { name: "live-page-finger-on-reading-pane-during-pen-stroke", passed: !!palm && palm.decision === "ignore:palm-during-stroke" && after === before, decision: palm ? palm.decision : null, scrollTop_before: before, scrollTop_after: after };
}

// MARK: Demo strokes for pictures

function demoStrokes() {
  const rect = canvas.getBoundingClientRect();
  const w = rect.width, h = rect.height;
  const mk = (id, tool, color, width, pts, pressure) => ({ id, tool, brush_version: BRUSHES[tool].version, width, color, opacity: BRUSHES[tool].opacity, points: pts.map(([x, y], i) => ({ x: x * w, y: y * h, pressure: pressure ? pressure(i / (pts.length - 1)) : null, elapsed_ms: i * 8, altitude: null, azimuth: null })) });
  const curve = (n, f) => Array.from({ length: n }, (_, i) => f(i / (n - 1)));
  doc.strokes.push(mk("demo-hl", "highlighter", "#facc15", 14, curve(24, (t) => [0.08 + t * 0.55, 0.14 + Math.sin(t * 3) * 0.004]), null));
  doc.strokes.push(mk("demo-word", "pen", "#1d4ed8", 3.5, curve(90, (t) => [0.1 + t * 0.5, 0.3 + Math.sin(t * Math.PI * 6) * 0.03 * (1 - t * 0.3) + t * 0.02]), (t) => 0.3 + 0.6 * Math.sin(t * Math.PI)));
  doc.strokes.push(mk("demo-loop", "pen", "#b91c1c", 3, curve(80, (t) => { const a = t * Math.PI * 2.5; return [0.72 + Math.cos(a) * 0.12, 0.42 + Math.sin(a) * 0.1 + t * 0.03]; }), (t) => 0.25 + 0.5 * t));
  doc.strokes.push(mk("demo-mouse", "pen", "#15803d", 4, curve(40, (t) => [0.12 + t * 0.4, 0.62 + Math.abs(Math.sin(t * Math.PI * 2)) * 0.08]), null));
  doc.strokes.push(mk("demo-hl2", "highlighter", "#38bdf8", 14, curve(20, (t) => [0.3 + t * 0.4, 0.15 + t * 0.001]), null));
  strokeCounter = 100;
  for (const s of doc.strokes) undoStack.push({ type: "add", id: s.id });
  updateHistoryButtons();
}

// MARK: Boot

try {
  await init();
  $("core").textContent = `core ${core.apiVersion()} · wasm`;
  // The reading pane shows the core's BSB John 3:16–17 conformance fixture (S52, CC0): the
  // only text bytes on screen come from a registered source, as in the DOM reader.
  const fixtures = JSON.parse(core.conformanceFixturesJson());
  const bsb = fixtures.find((f) => f.id === "cross-verse-selection-john-3" && f.kind === "text");
  if (!bsb) throw new Error("the BSB fixture is missing from the core");
  const verse = (s) => { const el = document.createElement("span"); el.className = "verse"; const sup = document.createElement("sup"); sup.className = "marker"; sup.textContent = s.segment_id.split(".").pop(); el.append(sup, s.text); return el; };
  const fill = (target) => { target.replaceChildren(); bsb.segments.forEach((s, i) => { if (i) target.append(" "); target.append(verse(s)); }); };
  fill($("reading-text"));
  const license = (bsb.license || "Berean Standard Bible: public domain (CC0)").split(" ").filter((w) => !w.startsWith("http")).join(" ").replace(/[,;: ]+$/, "");
  $("reading-attribution").textContent = `Fixture: ${license}. The ink to the right anchors nothing yet.`;
  // The same verses again, labelled, so the pane scrolls at every window size and the palm
  // case can check that a finger during a stroke does not move it.
  $("reading-text").insertAdjacentHTML("afterend", `<p class="attribution">The same fixture verses repeated so the pane scrolls.</p><p class="text" id="reading-text-repeat"></p>`);
  fill($("reading-text-repeat"));
  resize();
  new ResizeObserver(resize).observe(canvas.parentElement);
  if (params.get("demo") !== "0") { demoStrokes(); repaintCommitted(); paint(); }
  if (params.get("tool")) document.getElementById(`tool-${params.get("tool")}`)?.click();
  $("status").textContent = `Pointer events: ${"PointerEvent" in window ? "yes" : "no"} · coalesced events: ${"getCoalescedEvents" in PointerEvent.prototype ? "yes" : "no"} · dpr ${dpr} · brushes ${ENGINE}`;
  window.openlogosInk = { doc, measure, palmCases, livePalmCase, undo, redo, arbiter, decisions, core, brushes: BRUSHES,
    handReport: () => { const r = analyse(samples, "hand run (real pointer events)"); readout(summarize(r)); return r; } };
  if (params.get("autorun") === "1") {
    const palm = palmCases();
    const livePalm = await livePalmCase();
    const measurement = await measure();
    const measurementTouch = await measure({ pointerType: "touch", label: "synthesized touch (finger mode)", points: 60 });
    let exportOk = false, exportError = null;
    try { core.inkDocumentJson(doc); exportOk = true; } catch (e) { exportError = `${e.kind}: ${e.message}`; }
    window.openlogosInk.results = { palm, livePalm, measurement, measurementTouch, exportOk, exportError, strokes: doc.strokes.length };
    readout(summarize(measurement));
  }
  window.openlogosReady = true;
} catch (error) {
  window.openlogosError = String(error && error.stack || error);
  $("status").textContent = `Failed: ${error}`;
}
