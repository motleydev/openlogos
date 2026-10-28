// B039: the web ink prototype (apps/web/ink) in a real browser, headless, through WebDriver,
// after apps/web/reader/test/run-chrome.mjs: a static server for apps/web, a WebDriver session
// (chromedriver by default; any W3C driver on $WEBDRIVER_URL), the page at ?autorun=1, its
// window.openlogosInk.results (the palm cases of crates/ink/src/touch.rs through the JS
// mirror, the live-page palm case, the synthesized pen and touch latency runs, the export
// through inkDocumentJson), and screenshots of the page as the caller asks.
//
//   node apps/web/ink/test/run-chrome.mjs --chromedriver <path> [--shots <dir>] [--theme light|dark]
//
// Exit code 0 when every palm case passed, the live case passed, the export validated and the
// pen run painted frames; the results are printed as JSON lines.
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const webRoot = resolve(fileURLToPath(new URL("../../", import.meta.url)));
const args = process.argv.slice(2);
const option = (name, fallback) => { const i = args.indexOf(name); return i >= 0 ? args[i + 1] : fallback; };
const cases = args.flatMap((a, i) => (a === "--case" ? [args[i + 1]] : []));
const chromedriver = option("--chromedriver", process.env.CHROMEDRIVER);
const shotsDir = option("--shots", null);
const browser = option("--browser", "chrome");
const binary = option("--binary", null);
const theme = option("--theme", null); // light|dark for the case screenshots // the browser executable, when the driver should not use the default
const page = option("--page", "ink"); // ink | excalidraw (the comparison page, needs scripts/ink-excalidraw-build.sh)
const headed = args.includes("--headed"); // a visible Chrome window instead of --headless=new
const actions = args.includes("--actions"); // after autorun: a WebDriver Actions pen drag (trusted input events) on ?record=1 and its report
const width = Number(option("--width", "1440"));
const height = Number(option("--height", "900"));

const types = { ".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8", ".wasm": "application/wasm", ".json": "application/json", ".css": "text/css", ".ts": "text/plain" };

const server = createServer(async (request, response) => {
  const path = decodeURIComponent(new URL(request.url, "http://localhost").pathname);
  const file = join(webRoot, path.endsWith("/") ? `${path}index.html` : path);
  try {
    const body = await readFile(file);
    response.writeHead(200, { "content-type": types[extname(file)] || "application/octet-stream", "cross-origin-opener-policy": "same-origin", "cross-origin-embedder-policy": "require-corp" });
    response.end(body);
  } catch {
    response.writeHead(404);
    response.end("not found");
  }
});
await new Promise((ok) => server.listen(0, "127.0.0.1", ok));
const port = server.address().port;
const pageUrl = page === "excalidraw" ? `http://127.0.0.1:${port}/ink/excalidraw/` : `http://127.0.0.1:${port}/ink/`;

let driver = null;
let driverUrl = process.env.WEBDRIVER_URL;
if (!driverUrl) {
  if (!chromedriver) { console.error("need --chromedriver <path> or WEBDRIVER_URL"); process.exit(2); }
  const driverPort = 9515 + Math.floor(Math.random() * 1000);
  driver = spawn(chromedriver, [`--port=${driverPort}`], { stdio: ["ignore", "pipe", "pipe"] });
  driverUrl = `http://127.0.0.1:${driverPort}`;
  await new Promise((ok, fail) => {
    driver.stdout.on("data", (d) => { if (String(d).includes("was started successfully")) ok(); });
    driver.on("exit", (code) => fail(new Error(`chromedriver exited ${code}`)));
    setTimeout(ok, 3000);
  });
}

async function wd(method, path, body) {
  const response = await fetch(`${driverUrl}${path}`, { method, headers: { "content-type": "application/json" }, body: body ? JSON.stringify(body) : undefined });
  const json = await response.json();
  if (!response.ok || (json.value && json.value.error)) throw new Error(`${method} ${path}: ${JSON.stringify(json.value || json).slice(0, 400)}`);
  return json.value;
}

const capabilities = browser === "firefox"
  ? { alwaysMatch: { browserName: "firefox", "moz:firefoxOptions": { ...(binary ? { binary } : {}), args: ["-headless", `-width=${width}`, `-height=${height}`] } } }
  : browser === "safari"
    ? { alwaysMatch: { browserName: "safari" } }
    : { alwaysMatch: { browserName: "chrome", "goog:chromeOptions": { args: [...(headed ? [] : ["--headless=new", "--disable-gpu"]), `--window-size=${width},${height}`, "--hide-scrollbars", "--force-device-scale-factor=1"] } } };

class DoneSignal extends Error {}
let failures = 0;
let sessionId = null;
try {
  const session = await wd("POST", "/session", { capabilities });
  sessionId = session.sessionId;
  const S = `/session/${sessionId}`;
  try { await wd("POST", `${S}/window/rect`, { width, height }); } catch {}
  if (page === "excalidraw") {
    await wd("POST", `${S}/url`, { url: `${pageUrl}?autorun=1${theme ? `&theme=${theme}` : ""}` });
    let results = null;
    for (let i = 0; i < 300 && !results; i++) {
      const value = await wd("POST", `${S}/execute/sync`, { script: "return window.openlogosError || (window.openlogosReady && window.openlogosExcalidraw && window.openlogosExcalidraw.results ? JSON.stringify({ version: window.openlogosExcalidraw.version, ...window.openlogosExcalidraw.results }) : null)", args: [] });
      if (typeof value === "string" && value.startsWith("{")) results = JSON.parse(value);
      else if (value) throw new Error(`page failed: ${value}`);
      else await new Promise((ok) => setTimeout(ok, 200));
    }
    if (!results) throw new Error("the Excalidraw page never reported results");
    const agent = await wd("POST", `${S}/execute/sync`, { script: "return navigator.userAgent", args: [] });
    console.log(JSON.stringify({ browser: agent, excalidraw: results.version, width, height }));
    for (const key of ["measurement", "model", "export", "rtl", "debug"]) console.log(JSON.stringify({ [key]: results[key] }));
    if (!(results.model.freedraw_elements > 0)) failures += 1;
    const m = results.measurement;
    console.log(`Excalidraw: ${results.model.freedraw_elements} freedraw element(s) from ${m.samples} samples; input→first frame p50 ${m.input_to_first_frame_ms.p50} p95 ${m.input_to_first_frame_ms.p95} ms, input→next frame p50 ${m.input_to_next_frame_ms.p50} p95 ${m.input_to_next_frame_ms.p95} ms; export json ${results.export.json_bytes} bytes, svg ${results.export.svg_bytes} bytes`);
    if (shotsDir) {
      await mkdir(shotsDir, { recursive: true });
      const shoot = async (name) => { const png = await wd("GET", `${S}/screenshot`); await writeFile(join(shotsDir, `${name}.png`), Buffer.from(png, "base64")); console.log(`screenshot ${join(shotsDir, `${name}.png`)}`); };
      await shoot(`web-excalidraw-${browser}-${width}-autorun${theme ? `-${theme}` : ""}`);
      await wd("POST", `${S}/url`, { url: `${pageUrl}?lang=he-IL${theme ? `&theme=${theme}` : ""}` });
      for (let i = 0; i < 100; i++) { const ready = await wd("POST", `${S}/execute/sync`, { script: "return !!window.openlogosReady", args: [] }); if (ready) break; await new Promise((ok) => setTimeout(ok, 200)); }
      await new Promise((ok) => setTimeout(ok, 1500)); // the locale chunk loads after the API arrives
      const rtl = await wd("POST", `${S}/execute/sync`, { script: "return JSON.stringify(window.openlogosExcalidraw.rtl())", args: [] });
      console.log(JSON.stringify({ rtl_he_IL: JSON.parse(rtl) }));
      await shoot(`web-excalidraw-${browser}-${width}-rtl-he${theme ? `-${theme}` : ""}`);
    }
    throw new DoneSignal();
  }
  await wd("POST", `${S}/url`, { url: `${pageUrl}?autorun=1${theme ? `&theme=${theme}` : ""}` });
  let results = null;
  for (let i = 0; i < 300 && !results; i++) {
    const value = await wd("POST", `${S}/execute/sync`, { script: "return window.openlogosError || (window.openlogosReady && window.openlogosInk && window.openlogosInk.results ? JSON.stringify(window.openlogosInk.results) : null)", args: [] });
    if (typeof value === "string" && value.startsWith("{")) results = JSON.parse(value);
    else if (value) throw new Error(`page failed: ${value}`);
    else await new Promise((ok) => setTimeout(ok, 200));
  }
  if (!results) throw new Error("the page never reported results");
  const agent = await wd("POST", `${S}/execute/sync`, { script: "return [navigator.userAgent, document.getElementById('core').textContent, document.getElementById('status').textContent]", args: [] });
  console.log(JSON.stringify({ browser: agent[0], core: agent[1], status: agent[2], width, height }));
  for (const c of results.palm) { console.log(JSON.stringify({ palm_case: c.name, passed: c.passed, got: c.got })); if (!c.passed) failures += 1; }
  console.log(JSON.stringify({ live_case: results.livePalm }));
  if (!results.livePalm.passed) failures += 1;
  console.log(JSON.stringify({ measurement: results.measurement }));
  console.log(JSON.stringify({ measurement: results.measurementTouch }));
  console.log(JSON.stringify({ export_valid: results.exportOk, export_error: results.exportError, strokes: results.strokes }));
  if (!results.exportOk) failures += 1;
  if (!(results.measurement.painted > 0)) failures += 1;
  const m = results.measurement;
  console.log(`Web ink: ${results.palm.filter((c) => c.passed).length}/${results.palm.length} palm cases passed, live palm case ${results.livePalm.passed ? "passed" : "FAILED"}, export ${results.exportOk ? "valid" : "invalid"}; ${m.label}: ${m.samples} samples in ${m.painted} frames, input→draw done p50 ${m.input_to_draw_done_ms.p50} p95 ${m.input_to_draw_done_ms.p95} ms, input→next frame p50 ${m.input_to_next_frame_ms.p50} p95 ${m.input_to_next_frame_ms.p95} ms, inkOutline p50 ${m.outline_call_ms.p50} ms`);

  if (actions) {
    // Trusted input: the WebDriver Actions API drives Chrome's input pipeline (DevTools
    // Input.dispatch*), so the page sees real pointer events with their own timestamps and the
    // compositor in the loop, unlike the in-page synthesized dispatch above.
    await wd("POST", `${S}/url`, { url: `${pageUrl}?record=1&demo=0${theme ? `&theme=${theme}` : ""}` });
    for (let i = 0; i < 100; i++) { const ready = await wd("POST", `${S}/execute/sync`, { script: "return !!window.openlogosReady", args: [] }); if (ready) break; await new Promise((ok) => setTimeout(ok, 200)); }
    const rect = await wd("POST", `${S}/execute/sync`, { script: "const r = document.getElementById('ink').getBoundingClientRect(); return [r.left, r.top, r.width, r.height]", args: [] });
    const [left, top, w, h] = rect;
    const steps = 120;
    const at = (i) => { const a = (i / steps) * Math.PI * 2.4; return [Math.round(left + w * 0.5 + Math.cos(a) * Math.min(w, h) * 0.3 + i * 0.6), Math.round(top + h * 0.45 + Math.sin(a) * Math.min(w, h) * 0.18)]; };
    for (const pointerType of ["pen", "mouse"]) {
      const seq = [{ type: "pointerMove", duration: 0, x: at(0)[0], y: at(0)[1] }, { type: "pointerDown", button: 0, ...(pointerType === "pen" ? { pressure: 0.5 } : {}) }];
      for (let i = 1; i < steps; i++) seq.push({ type: "pointerMove", duration: 4, x: at(i)[0], y: at(i)[1], ...(pointerType === "pen" ? { pressure: 0.3 + 0.5 * Math.sin((i / steps) * Math.PI) } : {}) });
      seq.push({ type: "pointerUp", button: 0 });
      await wd("POST", `${S}/actions`, { actions: [{ type: "pointer", id: `b039-${pointerType}`, parameters: { pointerType }, actions: seq }] });
      await new Promise((ok) => setTimeout(ok, 300));
      const report = await wd("POST", `${S}/execute/sync`, { script: "return JSON.stringify(window.openlogosInk.handReport())", args: [] });
      const r = JSON.parse(report);
      r.label = `webdriver actions ${pointerType} (${headed ? "headed" : "headless"} Chrome)`;
      console.log(JSON.stringify({ measurement: r }));
      console.log(`Web ink actions ${pointerType}: ${r.samples} events in ${r.painted} frames, input→draw done p50 ${r.input_to_draw_done_ms.p50} p95 ${r.input_to_draw_done_ms.p95} ms, input→next frame p50 ${r.input_to_next_frame_ms.p50} p95 ${r.input_to_next_frame_ms.p95} ms`);
      const stroke = await wd("POST", `${S}/execute/sync`, { script: "const s = window.openlogosInk.doc.strokes.at(-1); return s ? JSON.stringify({ points: s.points.length, pressure_first: s.points[0].pressure, pressure_mid: s.points[Math.floor(s.points.length/2)].pressure }) : 'none'", args: [] });
      console.log(JSON.stringify({ actions_stroke: pointerType, stroke: JSON.parse(stroke === "none" ? "null" : stroke) }));
      await wd("POST", `${S}/execute/sync`, { script: "window.openlogosInk.doc.strokes.length = 0; window.openlogosInk.undo(); return 1", args: [] }).catch(() => {});
      await wd("POST", `${S}/url`, { url: `${pageUrl}?record=1&demo=0${theme ? `&theme=${theme}` : ""}` });
      for (let i = 0; i < 100; i++) { const ready = await wd("POST", `${S}/execute/sync`, { script: "return !!window.openlogosReady", args: [] }); if (ready) break; await new Promise((ok) => setTimeout(ok, 200)); }
    }
    if (shotsDir) {
      await mkdir(shotsDir, { recursive: true });
      await wd("POST", `${S}/actions`, { actions: [{ type: "pointer", id: "b039-shot", parameters: { pointerType: "pen" }, actions: [{ type: "pointerMove", duration: 0, x: at(0)[0], y: at(0)[1] }, { type: "pointerDown", button: 0, pressure: 0.5 }, ...Array.from({ length: steps - 1 }, (_, i) => ({ type: "pointerMove", duration: 4, x: at(i + 1)[0], y: at(i + 1)[1], pressure: 0.3 + 0.5 * Math.sin(((i + 1) / steps) * Math.PI) })), { type: "pointerUp", button: 0 }] }] });
      await new Promise((ok) => setTimeout(ok, 300));
      await wd("POST", `${S}/execute/sync`, { script: "window.openlogosInk.handReport(); return 1", args: [] });
      const png = await wd("GET", `${S}/screenshot`);
      await writeFile(join(shotsDir, `web-ink-${browser}-${width}-actions-${headed ? "headed" : "headless"}.png`), Buffer.from(png, "base64"));
      console.log(`screenshot ${join(shotsDir, `web-ink-${browser}-${width}-actions-${headed ? "headed" : "headless"}.png`)}`);
    }
  }

  if (shotsDir) {
    await mkdir(shotsDir, { recursive: true });
    const shoot = async (name) => {
      const png = await wd("GET", `${S}/screenshot`);
      await writeFile(join(shotsDir, `${name}.png`), Buffer.from(png, "base64"));
      console.log(`screenshot ${join(shotsDir, `${name}.png`)}`);
    };
    await wd("POST", `${S}/url`, { url: `${pageUrl}?autorun=1${theme ? `&theme=${theme}` : ""}` });
    for (let i = 0; i < 300; i++) { const ready = await wd("POST", `${S}/execute/sync`, { script: "return !!(window.openlogosReady && window.openlogosInk && window.openlogosInk.results)", args: [] }); if (ready) break; await new Promise((ok) => setTimeout(ok, 200)); }
    await shoot(`web-ink-${browser}-${width}-autorun${theme ? `-${theme}` : ""}`);
    for (const view of ["highlighter", "eraser"]) {
      await wd("POST", `${S}/url`, { url: `${pageUrl}?tool=${view}${theme ? `&theme=${theme}` : ""}` });
      for (let i = 0; i < 50; i++) {
        const ready = await wd("POST", `${S}/execute/sync`, { script: "return !!window.openlogosReady", args: [] });
        if (ready) break;
        await new Promise((ok) => setTimeout(ok, 200));
      }
      await shoot(`web-ink-${browser}-${width}-${view}${theme ? `-${theme}` : ""}`);
    }
  }
} catch (error) {
  if (!(error instanceof DoneSignal)) {
    console.error(String(error));
    failures = failures || 1;
  }
} finally {
  if (sessionId) { try { await wd("DELETE", `/session/${sessionId}`); } catch {} }
  if (driver) driver.kill();
  server.close();
}
process.exit(failures ? 1 : 0);
