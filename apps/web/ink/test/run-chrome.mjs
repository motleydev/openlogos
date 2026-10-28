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
const pageUrl = `http://127.0.0.1:${port}/ink/`;

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
    : { alwaysMatch: { browserName: "chrome", "goog:chromeOptions": { args: ["--headless=new", `--window-size=${width},${height}`, "--disable-gpu", "--hide-scrollbars", "--force-device-scale-factor=1"] } } };

let failures = 0;
let sessionId = null;
try {
  const session = await wd("POST", "/session", { capabilities });
  sessionId = session.sessionId;
  const S = `/session/${sessionId}`;
  try { await wd("POST", `${S}/window/rect`, { width, height }); } catch {}
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

  if (shotsDir) {
    await mkdir(shotsDir, { recursive: true });
    const shoot = async (name) => {
      const png = await wd("GET", `${S}/screenshot`);
      await writeFile(join(shotsDir, `${name}.png`), Buffer.from(png, "base64"));
      console.log(`screenshot ${join(shotsDir, `${name}.png`)}`);
    };
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
  console.error(String(error));
  failures = failures || 1;
} finally {
  if (sessionId) { try { await wd("DELETE", `/session/${sessionId}`); } catch {} }
  if (driver) driver.kill();
  server.close();
}
process.exit(failures ? 1 : 0);
