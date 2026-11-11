// B072: the web workspace shell (apps/web/shell) in a real browser through WebDriver, after
// apps/web/reader/test/run-chrome.mjs: a static server for apps/web, a WebDriver session
// (chromedriver by default; any W3C driver on $WEBDRIVER_URL), then the shell cases below driven
// with WebDriver Actions (trusted key and pointer events, so the registry's bindings are what
// is tested), the speed runs of specs/22 §7, and screenshots on request.
//
//   node apps/web/shell/test/run-chrome.mjs --chromedriver <path> [--case name…] [--headed]
//        [--bench [--samples 200] [--bench-json <file>]] [--shots <dir> [--extra-shots <dir>]]
//
// Exit code 0 when every case passed (and, with --bench, every run produced its samples); the
// results are printed as JSON lines and a Summary line.
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const webRoot = resolve(fileURLToPath(new URL("../../", import.meta.url)));
const repoRoot = resolve(webRoot, "../..");
const args = process.argv.slice(2);
const option = (name, fallback) => { const i = args.indexOf(name); return i >= 0 ? args[i + 1] : fallback; };
const only = args.flatMap((a, i) => (a === "--case" ? [args[i + 1]] : []));
const chromedriver = option("--chromedriver", process.env.CHROMEDRIVER);
const shotsDir = option("--shots", null);
const extraDir = option("--extra-shots", null); // shots beyond the ten kept in the evidence
const wellShotsDir = option("--well-shots", null); // B076: the contextual well's states
const menuShotsDir = option("--menu-shots", null); // B077: the context menu's states
const readShotsDir = option("--read-shots", null); // B078: continuous reading
const storeShotsDir = option("--store-shots", null); // B074: the user store
const headed = args.includes("--headed");
const storeBenchOn = args.includes("--store-bench"); // B074: restore launches and save/workspace rows alone
const launches = Number(option("--launches", "30"));
const benchOn = args.includes("--bench");
const samples = Number(option("--samples", "200"));
const benchJson = option("--bench-json", null);
const width = Number(option("--width", "1440"));
const height = Number(option("--height", "900"));

const types = { ".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8", ".wasm": "application/wasm", ".json": "application/json", ".css": "text/css; charset=utf-8", ".ts": "text/plain" };
const server = createServer(async (request, response) => {
  const path = decodeURIComponent(new URL(request.url, "http://localhost").pathname);
  const file = join(webRoot, path.endsWith("/") ? `${path}index.html` : path);
  try {
    const body = await readFile(file);
    response.writeHead(200, { "content-type": types[extname(file)] || "application/octet-stream" });
    response.end(body);
  } catch {
    response.writeHead(404);
    response.end("not found");
  }
});
await new Promise((ok) => server.listen(0, "127.0.0.1", ok));
const base = `http://127.0.0.1:${server.address().port}/shell/`;

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

const chromeArgs = [`--window-size=${width},${height}`, "--force-device-scale-factor=1", "--hide-scrollbars"];
if (!headed) chromeArgs.unshift("--headless=new", "--disable-gpu");
const capabilities = { alwaysMatch: { browserName: "chrome", "goog:chromeOptions": { args: chromeArgs } } };

// MARK: helpers

const K = { Meta: "", Alt: "", Shift: "", Control: "", Enter: "", Escape: "", Tab: "", ArrowDown: "", ArrowUp: "", ArrowLeft: "", ArrowRight: "", F10: "" };
let S = "";
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
const js = (script, ...a) => wd("POST", `${S}/execute/sync`, { script, args: a });
const jsAsync = (script, ...a) => wd("POST", `${S}/execute/async`, { script, args: a });
const snap = () => js("return window.openlogosShell.snapshot()");

/** Presses a chord: modifiers down, the key, everything up (trusted events through the driver). */
async function chord(...keys) {
  const down = keys.map((k) => ({ type: "keyDown", value: K[k] || k }));
  const up = [...keys].reverse().map((k) => ({ type: "keyUp", value: K[k] || k }));
  await wd("POST", `${S}/actions`, { actions: [{ type: "key", id: "kb", actions: [...down, ...up] }] });
  await wd("DELETE", `${S}/actions`);
  await sleep(30);
}

async function type(text) {
  const actions = [...text].flatMap((c) => [{ type: "keyDown", value: c }, { type: "keyUp", value: c }]);
  await wd("POST", `${S}/actions`, { actions: [{ type: "key", id: "kb", actions }] });
  await sleep(30);
}

/** A mouse drag between two viewport points. */
async function drag(from, to, steps = 8) {
  const moves = [];
  for (let i = 1; i <= steps; i++) moves.push({ type: "pointerMove", duration: 16, x: Math.round(from[0] + ((to[0] - from[0]) * i) / steps), y: Math.round(from[1] + ((to[1] - from[1]) * i) / steps) });
  await wd("POST", `${S}/actions`, { actions: [{ type: "pointer", id: "mouse", parameters: { pointerType: "mouse" }, actions: [
    { type: "pointerMove", duration: 0, x: Math.round(from[0]), y: Math.round(from[1]) }, { type: "pointerDown", button: 0 }, ...moves, { type: "pointerUp", button: 0 },
  ] }] });
  await wd("DELETE", `${S}/actions`);
  await sleep(60);
}

async function click(x, y, count = 1) {
  const clicks = [];
  for (let i = 0; i < count; i++) clicks.push({ type: "pointerDown", button: 0 }, { type: "pointerUp", button: 0 });
  await wd("POST", `${S}/actions`, { actions: [{ type: "pointer", id: "mouse", parameters: { pointerType: "mouse" }, actions: [{ type: "pointerMove", duration: 0, x: Math.round(x), y: Math.round(y) }, ...clicks] }] });
  await wd("DELETE", `${S}/actions`);
  await sleep(40);
}

/** B077: a right-click (button 2) at a viewport point, with Shift held for the escape hatch. */
async function rightClick(x, y, { shift = false } = {}) {
  const actions = [{ type: "pointer", id: "mouse", parameters: { pointerType: "mouse" }, actions: [
    { type: "pointerMove", duration: 0, x: Math.round(x), y: Math.round(y) }, ...(shift ? [{ type: "pause", duration: 0 }] : []), { type: "pointerDown", button: 2 }, { type: "pointerUp", button: 2 },
  ] }];
  if (shift) actions.push({ type: "key", id: "kb", actions: [{ type: "keyDown", value: K.Shift }, { type: "pause", duration: 0 }, { type: "pause", duration: 0 }, { type: "keyUp", value: K.Shift }] });
  await wd("POST", `${S}/actions`, { actions });
  await wd("DELETE", `${S}/actions`);
  await sleep(40);
}

/** The middle of a word of column N's text. */
async function wordPoint(column, w) {
  const pts = await phrasePoints(column, w);
  if (!pts) throw new Error(`no "${w}" in column ${column}`);
  return [(pts[0][0] + pts[1][0]) / 2, (pts[0][1] + pts[1][1]) / 2];
}

/** The top-level rows of the open context menu (or level `n`), as drawn. */
async function menuRows(level = 0) {
  const s = await snap();
  return s.contextMenu && s.contextMenu.menus[level] ? s.contextMenu.menus[level].rows : null;
}

/** Clicks the row of the open context menu (at `level`) whose label is `label`. */
async function chooseRow(label, level = 0) {
  const at = await js("const m = document.querySelectorAll('.ctx-menu')[arguments[1]]; const r = m && [...m.querySelectorAll('.ctx-row')].find((x) => x.querySelector('.ctx-label > span').textContent === arguments[0]); if (!r) return null; const b = r.getBoundingClientRect(); return [b.left + b.width / 2, b.top + b.height / 2];", label, level);
  if (!at) throw new Error(`no menu row "${label}"`);
  await click(...at);
}

/** Centre of the first element matching a selector, in viewport pixels. */
async function centre(selector) {
  const r = await js("const e = document.querySelector(arguments[0]); if (!e) return null; const r = e.getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2];", selector);
  if (!r) throw new Error(`no element ${selector}`);
  return r;
}

/** Viewport points just inside the first and last character of `phrase` in column N's text. */
async function phrasePoints(column, phrase) {
  return js(`
    const text = document.querySelector('#columns > .column[data-number="' + arguments[0] + '"] .measure .text');
    const walker = document.createTreeWalker(text, NodeFilter.SHOW_TEXT);
    let node; while ((node = walker.nextNode())) { const i = node.data.indexOf(arguments[1]); if (i >= 0) {
      const r = document.createRange(); r.setStart(node, i); r.setEnd(node, i + 1); const a = r.getBoundingClientRect();
      r.setStart(node, i + arguments[1].length - 1); r.setEnd(node, i + arguments[1].length); const b = r.getBoundingClientRect();
      const rtl = getComputedStyle(text).direction === 'rtl';
      return rtl ? [[a.right - 1, a.top + a.height / 2], [b.left + 1, b.top + b.height / 2]] : [[a.left + 1, a.top + a.height / 2], [b.right - 1, b.top + b.height / 2]]; } }
    return null;`, column, phrase);
}

async function load(query = "") {
  await wd("POST", `${S}/url`, { url: `${base}${query}` });
  for (let i = 0; i < 100; i++) {
    const v = await js("return window.openlogosError || (window.openlogosReady ? 'ready' : null)");
    if (v === "ready") return;
    if (v) throw new Error(`page failed: ${v}`);
    await sleep(100);
  }
  throw new Error("the shell never became ready");
}

const rgb = (hex) => `rgb(${parseInt(hex.slice(1, 3), 16)}, ${parseInt(hex.slice(3, 5), 16)}, ${parseInt(hex.slice(5, 7), 16)})`;
function expect(problems, ok, message) { if (!ok) problems.push(message); }
const eq = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const sorted = (o) => (o ? Object.fromEntries(Object.entries(o).sort(([a], [b]) => a.localeCompare(b))) : o);
const groupsOf = (well) => well.groups.map((g) => [g.kind, g.items]);

// MARK: B078 helpers

/** Scrolls column N's stream `dy` px a frame, `steps` times (or until `until` is the chapter at
 *  the reading line), as a reader's wheel would: after each step the verse that was on the
 *  reading line must have moved by exactly the scroll (prepends and releases above it shifted
 *  scrollTop by their height), so `maxJump` is how far the text moved under the eye. */
async function scrollStream(column, dy, steps, until = null) {
  return jsAsync(`const [n, dy, steps, until] = arguments; const done = arguments[arguments.length - 1];
    const v = [...window.openlogosShell.host.views.values()].find((x) => x.section.isConnected && x.section.dataset.number === String(n));
    const st = v.stream, sc = v.scroller;
    const out = { maxJump: 0, maxBlocks: 0, maxNodes: 0, chapters: [], headers: [], steps: 0 };
    const next = (i) => {
      if (i >= steps) return finish();
      const top = st.measureTop(); const el = top && st.verseEl(top.segment_id); const before = el && el.getBoundingClientRect().top;
      const t0 = sc.scrollTop; sc.scrollTop = t0 + dy; const moved = sc.scrollTop - t0;
      requestAnimationFrame(() => requestAnimationFrame(() => {
        if (el && el.isConnected) out.maxJump = Math.max(out.maxJump, Math.abs(before - el.getBoundingClientRect().top - moved));
        out.maxBlocks = Math.max(out.maxBlocks, st.blocks.length);
        out.maxNodes = Math.max(out.maxNodes, v.page.getElementsByTagName('*').length);
        const b = st.blockAtTop(); const key = b && b.key;
        if (key && out.chapters[out.chapters.length - 1] !== key) { out.chapters.push(key); out.headers.push(st.header.textContent); }
        out.steps = i + 1;
        if (until && key === until) return finish();
        next(i + 1);
      }));
    };
    const finish = () => setTimeout(() => done(out), 400); // past the settle report
    next(0);`, column, dy, steps, until);
}

/** The y of a column's reading line and of a verse's top, viewport px. */
async function readingLine(column, segment) {
  return js(`const v = [...window.openlogosShell.host.views.values()].find((x) => x.section.isConnected && x.section.dataset.number === String(arguments[0]));
    const el = v.stream.verseEl(arguments[1]); return [v.stream.edge(), el ? el.getBoundingClientRect().top : null];`, column, segment);
}

async function navigate(reference) {
  await js("window.openlogosShell.run({ command_id: 'nav.navigate', args: { reference: arguments[0] } })", reference);
  await sleep(60);
}

async function palette(text) {
  await chord("Meta", "k");
  await type(text);
  await chord("Enter");
  await sleep(60);
}

// MARK: B074 helpers

let storeSerial = 0;
/** A database of its own for a case, so cases never see each other's saved store. */
const uniqueStore = (label) => `${label}-${process.pid}-${++storeSerial}`;

/** Puts a value straight into a store database (as a damaged file would be). */
async function idbPut(db, key, value) {
  return jsAsync(`const [name, key, value] = arguments; const done = arguments[arguments.length - 1];
    const r = indexedDB.open(name, 1); r.onupgradeneeded = () => r.result.createObjectStore('user-store');
    r.onsuccess = () => { const tx = r.result.transaction('user-store', 'readwrite'); tx.objectStore('user-store').put(value, key); tx.oncomplete = () => { r.result.close(); done(true); }; };`, db, key, value);
}

async function idbKeys(db) {
  return jsAsync(`const done = arguments[arguments.length - 1]; const r = indexedDB.open(arguments[0], 1); r.onupgradeneeded = () => r.result.createObjectStore('user-store');
    r.onsuccess = () => { const g = r.result.transaction('user-store').objectStore('user-store').getAllKeys(); g.onsuccess = () => { r.result.close(); done(g.result.map(String)); }; };`, db);
}

/** Ink pixels on the margin canvas around a verse of column N. */
async function inkNear(column, segment) {
  return js(`const v = [...window.openlogosShell.host.views.values()].find((x) => x.section.isConnected && x.section.dataset.number === String(arguments[0]));
    const el = v.stream.verseEl(arguments[1]); if (!el) return -1; const c = v.canvas; if (!c.width) return 0; const cr = c.getBoundingClientRect(); const r = el.getBoundingClientRect();
    const y0 = Math.max(0, Math.round(r.top - cr.top - 20)), h = 60; const d = c.getContext('2d').getImageData(0, y0, c.width, h).data; let n = 0; for (let i = 3; i < d.length; i += 4) if (d[i]) n++; return n;`, column, segment);
}

// MARK: cases

const tokens = JSON.parse(await readFile(join(repoRoot, "design/tokens/shell.json"), "utf8"));

const CASES = {
  async "seed-session"(p) {
    const s = await snap();
    expect(p, eq(s.workspaces, ["Study", "Hebrew"]), `workspaces ${JSON.stringify(s.workspaces)}`);
    expect(p, eq(s.dom.map((c) => c.tabs), [["BSB Jn 3", "WLC Gen 1"], ["SBLGNT Jn 3"], ["Note"]]), `tabs ${JSON.stringify(s.dom.map((c) => c.tabs))}`);
    expect(p, eq(s.dom.map((c) => c.badge), ["1", "2", "3"]), "badges 1 2 3");
    expect(p, s.dom[0].active && !s.dom[1].active, "column 1 active");
    expect(p, s.dom.every((c) => c.text), "every shown tab has text");
    expect(p, eq(s.columns.map((c) => c.link), ["Group 1", "Group 1", "Pinned"]), `links ${JSON.stringify(s.columns.map((c) => c.link))}`);
    const switcher = await js("return document.getElementById('workspace-switcher').textContent.replace(/\\s+/g, ' ').trim()");
    expect(p, switcher.startsWith("Study · 1/2"), `switcher "${switcher}"`);
    const placeholder = await js("return document.getElementById('palette-field').placeholder");
    expect(p, placeholder === "Passage, word, command, tab…  ⌘K", `placeholder "${placeholder}"`);
    const m = await js(`const q = (s) => document.querySelector(s).getBoundingClientRect();
      return { top: q('.topbar').height, strip: q('.tabstrip').height, identity: q('.identity').height, badge: q('.badge').width, well: q('#well').width, wellLeft: q('#well').left, tool: q('.tool').width, rule: q('.rule').width };`);
    const t = tokens.metrics;
    const want = { top: t.palette_field_height, strip: t.tab_strip_height, identity: t.identity_row_height, badge: t.column_badge, well: t.tool_well_width, wellLeft: t.tool_well_inset, tool: t.tool_button, rule: t.column_rule };
    for (const [k, v] of Object.entries(want)) expect(p, m[k] === v, `${k} is ${m[k]}, token ${v}`);
  },

  async "tokens-match-design"(p) {
    for (const theme of Object.keys(tokens.themes)) {
      await js("window.openlogosShell.run({ command_id: 'view.theme', args: { text: arguments[0] } })", theme);
      const got = await js(`const s = getComputedStyle(document.documentElement); return ['page','text','chrome','rule','accent'].map((k) => s.getPropertyValue('--' + (k === 'rule' ? 'rule' : k)).trim().toUpperCase());`);
      const want = ["page", "text", "chrome", "rule", "accent"].map((k) => tokens.themes[theme][k].toUpperCase());
      expect(p, eq(got, want), `${theme}: ${JSON.stringify(got)} ≠ ${JSON.stringify(want)}`);
      const bg = await js("return getComputedStyle(document.body).backgroundColor");
      expect(p, bg.length > 0, "body background");
    }
    const page = await js(`const s = getComputedStyle(document.documentElement); const T = window.openlogosShell.tokens;
      const sheet = [...document.styleSheets].flatMap((x) => [...x.cssRules]).map((r) => r.cssText).join('\\n');
      return { sizes: T.sizes, spacings: T.spacings, layout: window.openlogosShell.core.layoutConstants(), swatches: T.swatches,
        hl: ['yellow','green','blue','pink'].map((k) => [s.getPropertyValue('--hl-' + k).trim().toUpperCase(), Number(s.getPropertyValue('--hl-' + k + '-opacity'))]),
        radius: [s.getPropertyValue('--radius-control').trim(), s.getPropertyValue('--radius-tool-well').trim()],
        spacing: [1,2,3,4,5].map((i) => parseInt(s.getPropertyValue('--space-' + i))),
        fonts: [s.getPropertyValue('--font-reading').trim(), parseInt(s.getPropertyValue('--chrome-size')), parseInt(s.getPropertyValue('--tab-label-size')), parseInt(s.getPropertyValue('--badge-size'))],
        textSize: getComputedStyle(document.querySelector('.measure .text')).fontSize,
        leading: getComputedStyle(document.querySelector('.measure .text')).lineHeight,
        measure_px: document.querySelector('.measure').getBoundingClientRect().width,
        highlightRules: Object.fromEntries(['yellow','green','blue','pink'].map((k) => [k, (sheet.split('\\n').find((r) => r.startsWith('::highlight(ol-' + k + ')')) || '')])) };`);
    const r = tokens.reading;
    expect(p, eq(page.sizes, r.sizes) && page.spacings[1] === r.default_line_spacing && eq(page.spacings, r.line_spacings), "reading sizes and spacings");
    expect(p, page.layout.measure_cap_standard_pt === r.measure_cap.standard && page.layout.measure_cap_accessibility_pt === r.measure_cap.accessibility, `the tokens' measure caps are the core's layoutConstants() ${JSON.stringify(page.layout)}`);
    const hl = Object.entries(tokens.highlights).map(([, v]) => [v.color.toUpperCase(), v.opacity]);
    expect(p, eq(page.hl, hl), `highlight vars ${JSON.stringify(page.hl)}`);
    for (const [k, v] of Object.entries(tokens.highlights)) {
      const [c, o] = page.swatches[k] || [];
      expect(p, c && c.toUpperCase() === v.color.toUpperCase() && o === v.opacity, `script swatch ${k}: ${c} ${o}`);
      const rule = page.highlightRules[k] || "";
      expect(p, rule.includes(rgb(v.color)) && rule.includes(`${Math.round(v.opacity * 100)}%`), `::highlight(ol-${k}) paints ${v.color} at ${v.opacity}: ${rule}`);
    }
    expect(p, eq(page.radius, [`${tokens.radius.control}px`, `${tokens.radius.tool_well}px`]), `radii ${page.radius}`);
    expect(p, eq(page.spacing, tokens.spacing), `spacing ${page.spacing}`);
    expect(p, page.fonts[0] === tokens.fonts.reading.web && page.fonts[1] === tokens.fonts.chrome.size && page.fonts[2] === tokens.fonts.tab_label.size && page.fonts[3] === tokens.fonts.badge.size, `fonts ${JSON.stringify(page.fonts)}`);
    expect(p, page.textSize === `${r.default_size}px`, `reading size ${page.textSize}`);
    expect(p, page.leading === `${r.default_size * r.default_line_spacing}px`, `leading ${page.leading}`);
    expect(p, page.measure_px <= r.measure_cap.standard, `measure ${page.measure_px}`);
  },

  async "registry-bindings"(p) {
    const r = await js(`const h = window.openlogosShell.host; const n = h.registry.reduce((a, s) => a + s.shortcuts.web.length, 0);
      return { bindings: h.bindings.length, shortcuts: n, commands: h.registry.length, wellKeys: [...document.querySelectorAll('.tool')].map((b) => b.getAttribute('aria-keyshortcuts')) };`);
    expect(p, r.bindings === r.shortcuts, `${r.bindings} bindings for ${r.shortcuts} registry shortcuts`);
    expect(p, r.commands === 85, `${r.commands} commands`);
    expect(p, eq(r.wellKeys, ["V", "H", "P", "M", "E", "C"]), `tool keys from the registry ${r.wellKeys}`);
  },

  async "palette-passage-follows-group"(p) {
    await chord("Meta", "k");
    let s = await snap();
    expect(p, s.focus === "palette-field" && s.paletteOpen, `⌘K focuses the palette (focus ${s.focus})`);
    await type("jn 3:16");
    const first = await js("const r = document.querySelector('.row[aria-selected=true]'); return [document.querySelector('.results h3').textContent, r && r.querySelector('.title').textContent]");
    expect(p, eq(first, ["Passage", "John 3:16"]), `first row ${JSON.stringify(first)}`);
    await chord("Enter");
    s = await snap();
    expect(p, s.columns[0].reference === "John.3.16" && s.columns[1].reference === "John.3.16", `group 1 follows: ${s.columns.map((c) => c.reference)}`);
    expect(p, s.columns[2].reference === null, "the pinned note stays");
    expect(p, !s.paletteOpen && s.focus !== "palette-field", "palette closed, focus back in the text");
    expect(p, eq(s.dom[0].tabs, ["BSB Jn 3", "WLC Gen 1"]), `tab label stays the chapter ${s.dom[0].tabs}`);
    const identity = await js("return document.querySelector('#columns > .column .identity .passage').textContent");
    expect(p, identity === "John 3:16", `identity passage ${identity}`);
  },

  async "palette-sections-and-expand"(p) {
    await chord("Meta", "k");
    await type("tab");
    const rows = await js("return [...document.querySelectorAll('.results .row')].map((r) => [r.querySelector('.title').textContent, (r.querySelector('.key') || {}).textContent || '', (r.querySelector('.sub') || {}).textContent || '', r.classList.contains('disabled')])");
    const newTab = rows.find((r) => r[0] === "New tab");
    expect(p, newTab && newTab[1] === "⌥T", `New tab shows ⌥T: ${JSON.stringify(newTab)}`);
    const more = rows.find((r) => /^Show all \d+$/.test(r[0]));
    const widthCheck = await js("return [document.getElementById('palette-results').getBoundingClientRect().width, document.querySelector('.field-wrap').getBoundingClientRect().width]");
    expect(p, widthCheck[0] === Math.min(tokens.metrics.palette_dropdown_max_width, widthCheck[1]), `dropdown width ${widthCheck}`);
    expect(p, !!more, "a capped section offers Show all N");
    await type(" "); // "tab " still matches
    await chord("Meta", "a");
    await type(">");
    let count = await js("return document.querySelectorAll('.results .row:not(.more)').length");
    expect(p, count === 5, `commands capped at 5 (${count})`);
    await chord("Tab");
    count = await js("return document.querySelectorAll('.results .row:not(.more)').length");
    expect(p, count > 5, `Tab expands the section (${count})`);
    const keysShown = await js("return [...document.querySelectorAll('.results .row .key')].map((k) => k.textContent).join(' ')");
    expect(p, !keysShown.includes("{n}") && !keysShown.includes("…"), `no family placeholder in the keys: ${keysShown}`);
    const disabled = await js("return [...document.querySelectorAll('.results .row.disabled')].map((r) => r.querySelector('.title').textContent + ' — ' + ((r.querySelector('.sub') || {}).textContent || ''))");
    expect(p, disabled.some((d) => d.startsWith("Reopen closed tab — No closed tab to reopen")), `disabled with reason: ${JSON.stringify(disabled.slice(0, 4))}`);
    await chord("Meta", "a");
    await type("@");
    const sections = await js("return [...document.querySelectorAll('.results h3')].map((h) => h.textContent)");
    expect(p, eq(sections, ["Tabs and workspaces"]), `@ → ${sections}`);
    const tabRow = await js("const r = [...document.querySelectorAll('.results .row')].find((x) => x.querySelector('.title').textContent === 'BSB · Jn 3:16'); return r && r.querySelector('.sub').textContent");
    expect(p, tabRow === "Study · column 1", `tab row (the core's label) subtitle ${tabRow}`);
    await chord("Meta", "a");
    await type("move tab to column");
    const expansion = await js("return [...document.querySelectorAll('.results .row')].map((r) => [r.querySelector('.title').textContent, (r.querySelector('.key') || {}).textContent || ''])");
    expect(p, eq(expansion.slice(0, 3), [["Move tab to column 2", "⌥⇧2"], ["Move tab to column 3", "⌥⇧3"], ["Move tab to new column", ""]]), `argument expansion rows with concrete keys ${JSON.stringify(expansion)}`);
    await chord("Meta", "a");
    await type("#loved");
    const text = await js("return [...document.querySelectorAll('.results h3')].map((h) => h.textContent)");
    expect(p, eq(text, ["Text"]), `# → ${text}`);
    await chord("Escape");
    const s = await snap();
    expect(p, !s.paletteOpen && s.focus === "scroller", `Esc returns focus to the active column's text (${s.focus})`);
  },

  async "reason-text-from-core"(p) {
    await chord("Meta", "z");
    const s = await snap();
    expect(p, s.toast === "Nothing to undo", `⌘Z with nothing to undo: toast "${s.toast}" (reason_text)`);
  },

  async "slash-focuses-palette"(p) {
    await click(...(await centre('#columns > .column[data-number="2"] .measure')));
    await type("/");
    const s = await snap();
    expect(p, s.focus === "palette-field", `/ focuses the palette (${s.focus})`);
    expect(p, s.active === 2, "clicking column 2 focused it");
  },

  async "column-tab-workspace-keys"(p) {
    await chord("Alt", "2");
    let s = await snap();
    expect(p, s.active === 2 && s.dom[1].active, `⌥2 → column ${s.active}`);
    await chord("Alt", "1");
    await chord("Alt", "]");
    s = await snap();
    expect(p, s.dom[0].shown === "WLC Gen 1", `⌥] shows the next tab (${s.dom[0].shown})`);
    await chord("Alt", "[");
    s = await snap();
    expect(p, s.dom[0].shown === "BSB Jn 3", `⌥[ back (${s.dom[0].shown})`);
    await chord("Alt", "Shift", "]");
    s = await snap();
    expect(p, s.workspace === "Hebrew" && s.dom.length === 1, `⌥⇧] → ${s.workspace}`);
    await chord("Alt", "Shift", "[");
    s = await snap();
    expect(p, s.workspace === "Study" && s.dom.length === 3, `⌥⇧[ → ${s.workspace}`);
  },

  async "tab-commands-and-undo"(p) {
    await chord("Alt", "t");
    let s = await snap();
    expect(p, eq(s.dom[0].tabs, ["BSB Jn 3", "BSB Jn 3", "WLC Gen 1"]) && s.dom[0].shown === "BSB Jn 3", `⌥T new tab: ${s.dom[0].tabs}`);
    await chord("Alt", "Shift", "3");
    s = await snap();
    expect(p, eq(s.dom[2].tabs, ["Note", "BSB Jn 3"]) && s.active === 3, `⌥⇧3 moves the tab to column 3: ${JSON.stringify(s.dom.map((c) => c.tabs))}`);
    await chord("Meta", "z");
    s = await snap();
    expect(p, eq(s.dom[0].tabs, ["BSB Jn 3", "BSB Jn 3", "WLC Gen 1"]), `⌘Z takes the move back: ${JSON.stringify(s.dom.map((c) => c.tabs))}`);
    await chord("Meta", "z");
    s = await snap();
    expect(p, eq(s.dom[0].tabs, ["BSB Jn 3", "WLC Gen 1"]), `⌘Z takes the new tab back: ${s.dom[0].tabs}`);
    await chord("Meta", "Shift", "z");
    s = await snap();
    expect(p, eq(s.dom[0].tabs, ["BSB Jn 3", "BSB Jn 3", "WLC Gen 1"]), `⌘⇧Z redoes: ${s.dom[0].tabs}`);
    await chord("Alt", "w");
    s = await snap();
    expect(p, eq(s.dom[0].tabs, ["BSB Jn 3", "WLC Gen 1"]), `⌥W closes: ${s.dom[0].tabs}`);
    // The close × of a tab that is not shown: focus it, close it (two registered commands).
    await click(...(await centre('#columns > .column[data-number="1"] .tab:nth-child(2) .close')));
    s = await snap();
    expect(p, eq(s.dom[0].tabs, ["BSB Jn 3"]), `× closes WLC: ${s.dom[0].tabs}`);
    await click(...(await centre('#columns > .column[data-number="2"] .tabstrip .add')));
    s = await snap();
    expect(p, s.active === 2 && s.dom[1].tabs.length === 2, `+ in column 2: ${s.dom[1].tabs}`);
  },

  async "tool-keys-and-well"(p) {
    for (const [key, tool] of [["h", "highlight"], ["p", "pen"], ["m", "highlighter_pen"], ["e", "eraser"], ["c", "collect"], ["v", "select"]]) {
      await type(key);
      const s = await snap();
      const pressed = await js("return [...document.querySelectorAll('.tool[aria-pressed=true]')].map((b) => b.dataset.tool)");
      expect(p, s.tool === tool && eq(pressed, [tool]), `${key} → ${s.tool}, pressed ${pressed}`);
    }
    await type("x");
    let s = await snap();
    expect(p, eq(s.swatches, ["green", "yellow"]), `X swaps swatches ${s.swatches}`);
    await click(...(await centre(".tool[data-tool='pen']")));
    s = await snap();
    expect(p, s.tool === "pen", "clicking Pen chooses it");
    const label = await js("return document.getElementById('well').getAttribute('aria-label')");
    expect(p, label === "Tools, current: Pen", `well label ${label}`);
    const grip = await centre(".grip");
    await click(...grip, 2);
    s = await snap();
    expect(p, s.wellCollapsed, "double-click on the grip collapses the well");
    await click(...grip, 2);
    s = await snap();
    expect(p, !s.wellCollapsed, "and expands it");
    const before = await js("return document.getElementById('well').getBoundingClientRect().top");
    await drag(grip, [grip[0], grip[1] + 120], 6);
    const after = await js("return document.getElementById('well').getBoundingClientRect().top");
    expect(p, Math.abs(after - before - 120) < 4, `the grip drags the well (${before} → ${after})`);
    // Typing in the palette never changes the tool.
    await chord("Meta", "k");
    await type("hp");
    s = await snap();
    expect(p, s.tool === "pen", "bare keys are ignored in the field");
    await chord("Escape");
  },

  async "highlight-drag"(p) {
    await type("h");
    const pts = await phrasePoints(1, "eternal life");
    await drag(pts[0], pts[1]);
    await sleep(80);
    const s = await snap();
    const h = s.highlights[0];
    expect(p, h && h.swatch === "yellow" && h.anchors[0].segment_id === "John.3.16" && h.anchors[0].exact === "eternal life", `highlight ${JSON.stringify(s.highlights)}`);
    const painted = await js("const h = CSS.highlights.get('ol-yellow'); return h ? [...h].map((r) => r.toString()) : []");
    expect(p, eq(painted, ["eternal life"]), `painted ${JSON.stringify(painted)}`);
    expect(p, s.log.some((l) => l.command_id === "selection.highlight" && l.handled === false), "dispatched selection.highlight");
    // The same highlight is painted in another tab of the same edition (anchors, not DOM).
    await chord("Alt", "t");
    const again = await js("const h = CSS.highlights.get('ol-yellow'); return h ? h.size : 0");
    expect(p, again === 1, `repainted after re-render (${again})`);
  },

  async "collect-drag"(p) {
    await type("c");
    const pts = await phrasePoints(1, "For God so loved");
    await drag(pts[0], pts[1]);
    await sleep(80);
    const s = await snap();
    expect(p, s.toast === "Collected: Jn 3:16", `toast ${s.toast}`);
  },

  async "hebrew-highlight"(p) {
    await chord("Alt", "Shift", "]");
    await type("h");
    const pts = await phrasePoints(1, "אֱלֹהִ֑ים");
    await drag(pts[0], pts[1]);
    await sleep(80);
    const s = await snap();
    const a = s.highlights[0] && s.highlights[0].anchors[0];
    expect(p, a && a.segment_id === "Gen.1.1" && a.exact.includes("אֱלֹהִ֑ים"), `hebrew highlight ${JSON.stringify(s.highlights)}`);
  },

  async "pen-and-eraser"(p) {
    await type("p");
    const box = await js("const m = document.querySelector('#columns > .column[data-number=\"1\"] .measure').getBoundingClientRect(); const r = document.querySelector('#columns > .column[data-number=\"1\"] .scroller').getBoundingClientRect(); return [m.left, r.top, m.width];");
    const y = box[1] + 140;
    await drag([box[0] + 40, y], [box[0] + 240, y + 30], 12);
    let s = await snap();
    expect(p, s.ink.length === 1 && s.ink[0].strokes === 1, `pen stroke ${JSON.stringify(s.ink)}`);
    const inked = await js("const c = document.querySelector('#columns > .column[data-number=\"1\"] canvas.margin'); const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data; let n = 0; for (let i = 3; i < d.length; i += 4) if (d[i]) n++; return n;");
    expect(p, inked > 200, `the stroke is painted (${inked} px)`);
    await type("m");
    await drag([box[0] + 40, y + 60], [box[0] + 240, y + 60], 10);
    s = await snap();
    expect(p, s.ink[0].strokes === 2, "highlighter pen stroke");
    await type("e");
    await drag([box[0] + 140, y - 30], [box[0] + 140, y + 80], 10);
    s = await snap();
    expect(p, s.ink[0].erased >= 1, `eraser ${JSON.stringify(s.ink)}`);
  },

  async "placeholder-and-navigation"(p) {
    // B078: Romans 8 is a placeholder chapter of the BSB and SBLGNT streams, marked as such.
    await chord("Meta", "k");
    await type("rom 8");
    await chord("Enter");
    const s = await snap();
    expect(p, s.reading[0].blocks.includes("Rom.8") && s.reading[1].blocks.includes("Rom.8"), `Rom 8 opens in the group: ${JSON.stringify(s.reading.map((r) => r && r.blocks))}`);
    expect(p, s.reading[0].header === "BSB · Rom 8" && s.reading[0].notice === "Placeholder text — this edition is not installed", `header and notice ${JSON.stringify(s.reading[0])}`);
    await chord("Meta", "[");
    const back = await snap();
    expect(p, back.columns[0].reference === "John.3.16", `⌘[ goes back (${back.columns[0].reference})`);
  },

  async "menus"(p) {
    await click(...(await centre("#workspace-switcher")));
    const items = await js("return [...document.querySelectorAll('.menu [role^=menuitem]')].map((b) => b.textContent)");
    expect(p, items[0].includes("Study") && items[1].includes("Hebrew"), `switcher lists ${JSON.stringify(items.slice(0, 2))}`);
    await js("[...document.querySelectorAll('.menu [role^=menuitem]')].find((b) => b.textContent.includes('Hebrew')).click()");
    let s = await snap();
    expect(p, s.workspace === "Hebrew", "switch through the menu");
    await click(...(await centre("#text-menu")));
    await click(...(await centre(".menu [data-c='view.text_size_up']")));
    await click(...(await centre(".menu [data-theme='sepia']")));
    await click(...(await centre(".menu [data-spacing='1.7']")));
    s = await snap();
    expect(p, s.size === 21 && s.theme === "sepia" && s.spacing === 1.7, `text menu → ${s.size} ${s.theme} ${s.spacing}`);
    expect(p, ["view.text_size_up", "view.theme", "view.line_spacing"].every((id) => s.log.some((l) => l.command_id === id && l.handled === true)), "size, theme and spacing are core commands");
    const label = await js("return document.getElementById('text-menu').textContent.replace(/\\s+/g, ' ').trim()");
    expect(p, label === "Aa 21", `text menu label "${label}"`);
    const fs = await js("return getComputedStyle(document.querySelector('.measure .text')).fontSize");
    expect(p, fs === "21px", `text size applied (${fs})`);
    await chord("Escape");
    await chord("Meta", "k");
    await type("theme dark");
    await chord("Enter");
    s = await snap();
    expect(p, s.theme === "dark", "theme through the palette");
    await chord("Alt", "Shift", "[");
    await click(...(await centre('#columns > .column[data-number="2"] .pane-menu')));
    const pane = await js("return [...document.querySelectorAll('.menu [role^=menuitem]')].map((b) => b.textContent)");
    expect(p, pane.some((t) => t.startsWith("Pin tab")) && pane.some((t) => t.startsWith("Move tab to column 1")), `pane menu ${JSON.stringify(pane)}`);
    await js("[...document.querySelectorAll('.menu [role^=menuitem]')].find((b) => b.textContent.startsWith('Pin tab')).click()");
    s = await snap();
    expect(p, s.columns[1].link === "Pinned" && s.active === 2, `pin through the pane menu: ${s.columns[1].link}`);
  },

  async "column-resize"(p) {
    const before = await js("return document.querySelector('#columns > .column').getBoundingClientRect().width");
    const rule = await centre(".rule");
    await drag(rule, [rule[0] + 100, rule[1]], 6);
    const after = await js("return document.querySelector('#columns > .column').getBoundingClientRect().width");
    expect(p, Math.abs(after - before - 100) < 6, `column 1 ${before} → ${after}`);
    let s = await snap();
    expect(p, s.log.some((l) => l.command_id === "column.resize" && l.handled), "the drag ends in column.resize");
    expect(p, Math.abs(s.widths.reduce((a, b) => a + b, 0) - 1) < 1e-6 && s.widths[0] > 0.45, `session widths ${s.widths}`);
    await chord("Meta", "z");
    s = await snap();
    expect(p, eq(s.widths, [0.4, 0.35, 0.25]), `⌘Z restores the widths ${s.widths}`);
    await click(...(await centre(".rule")), 2);
    s = await snap();
    expect(p, s.widths.every((w) => Math.abs(w - 1 / 3) < 1e-3), `double-click equalizes ${s.widths}`);
  },

  async "scroll-anchor-recorded"(p) {
    // Large text; scroll column 1 on from the typed verse, let it settle, check the core has it.
    await setWindow(1024, 420);
    for (let i = 0; i < 4; i++) await js("window.openlogosShell.run({ command_id: 'view.text_size_up' })");
    const room = await js("const s = document.querySelector('#columns > .column .scroller'); const t = s.scrollTop; s.scrollTop = t + 200; return [t, s.scrollTop];");
    expect(p, room[1] - room[0] > 100, `column 1 scrolls (${room})`);
    await sleep(400);
    let s = await snap();
    const a = s.anchors[0];
    expect(p, a && /^John\.3\.(1[6-9]|2\d)$/.test(a.segment_id) && a.scalar >= 0, `scroll anchor ${JSON.stringify(a)}`);
    expect(p, s.log.some((l) => l.command_id === "tab.record_scroll" && l.handled), "tab.record_scroll dispatched");
    // Away and back: the column is rebuilt from the session and lands on the same anchor.
    const top = await js("return document.querySelector('#columns > .column .scroller').scrollTop");
    const verse = (await snap()).reading[0].top;
    await chord("Alt", "]");
    await chord("Alt", "[");
    await sleep(300);
    const again = await snap();
    expect(p, again.reading[0].top && again.reading[0].top.segment_id === verse.segment_id, `scroll restored from the anchor (${JSON.stringify(verse)} → ${JSON.stringify(again.reading[0].top)})`);
    await setWindow(width, height);
  },

  async "swatches-and-width"(p) {
    await click(...(await centre(".swatches .current")));
    await js("[...document.querySelectorAll('.menu [role^=menuitem]')].find((b) => b.textContent.includes('Blue')).click()");
    let s = await snap();
    expect(p, s.swatches[0] === "blue", `tool.swatch → ${s.swatches}`);
    await click(...(await centre(".swatches .alternate")));
    s = await snap();
    expect(p, eq(s.swatches, ["green", "blue"]), `swap via the alternate well ${s.swatches}`);
    const widths = [];
    for (let i = 0; i < 5; i++) { await click(...(await centre(".width-dot"))); widths.push((await snap()).width); }
    s = await snap();
    expect(p, eq(widths, [6, 8, 12, 16, 1]), `width dot steps up the core's widths and wraps: ${widths}`);
    expect(p, ["tool.swatch", "tool.swap_swatches", "tool.width_up", "tool.width"].every((id) => s.log.some((l) => l.command_id === id && l.handled)), "core commands");
  },

  async "well-clears-column-one"(p) {
    // At 1440 and 1024 the expanded well never covers column 1's text; collapsing gives the inset back.
    for (const w of [1440, 1024]) {
      await setWindow(w, w === 1440 ? 900 : 768);
      const r = await js(`const well = document.getElementById('well').getBoundingClientRect(); const t = document.querySelector('#columns > .column .measure .text').getBoundingClientRect(); return [well.right, t.left];`);
      expect(p, r[0] <= r[1] - 4, `${w}: well right ${r[0]} vs text left ${r[1]}`);
    }
    await click(...(await centre(".grip")), 2);
    const c = await js(`const well = document.getElementById('well').getBoundingClientRect(); const t = document.querySelector('#columns > .column .measure .text').getBoundingClientRect(); return [well.width, well.height, well.right, t.left];`);
    const m = tokens.metrics;
    expect(p, c[0] === m.tool_well_collapsed && c[1] === m.tool_well_collapsed, `collapsed grip ${c[0]}×${c[1]}`);
    expect(p, c[3] === m.tool_well_inset + m.tool_well_collapsed + 8 && c[2] <= c[3] - 8, `collapsed: text starts at ${c[3]}, clear of the grip (right ${c[2]})`);
    await setWindow(width, height);
  },

  // MARK: B076 — the contextual well

  async "well-contexts"(p) {
    const tops = () => js("const q = (c) => { const b = document.querySelector('#well button[data-command=\"' + c + '\"]'); return b ? Math.round(b.getBoundingClientRect().top) : null; }; return { well: document.getElementById('well').getBoundingClientRect().top, select: q('tool.select'), word: q('tool.word_study'), highlight: q('tool.highlight') };");
    let s = await snap();
    const translation = [["tools", ["tool.select", "tool.highlight", "tool.pen", "tool.highlighter_pen", "tool.eraser", "tool.collect"]], ["ink", ["tool.swap_swatches", "tool.swatch", "tool.width"]]];
    expect(p, s.well.context === "translation" && eq(groupsOf(s.well), translation), `translation well ${JSON.stringify(s.well)}`);
    const t0 = await tops();
    await chord("Alt", "2");
    s = await snap();
    expect(p, s.well.context === "original" && eq(s.well.groups[0].items.slice(0, 2), ["tool.select", "tool.word_study"]) && s.well.groups.length === 2, `original well ${JSON.stringify(s.well.groups)}`);
    const t1 = await tops();
    expect(p, t1.well === t0.well && t1.select === t0.select, `the well's top and Select stay put (${JSON.stringify(t0)} → ${JSON.stringify(t1)})`);
    await chord("Alt", "3");
    s = await snap();
    expect(p, s.well.context === "notes" && eq(groupsOf(s.well), [["tools", ["tool.type", "note.bold", "note.italic", "note.heading", "note.list", "note.link_passage", "tool.collect"]]]) && eq(s.well.pressed, ["tool.type"]), `notes well ${JSON.stringify(s.well)}`);
    const tips = await js("return [...document.querySelectorAll('#well .group button')].map((b) => b.title)");
    expect(p, tips.includes("Bold (⌘B)") && tips.includes("Heading") && tips.includes("Type (T)"), `tooltips ${JSON.stringify(tips)}`);
    const label = await js("return document.getElementById('well').getAttribute('aria-label')");
    expect(p, label === "Tools, current: Type", `well label ${label}`);
    // The dock: focus in its editor is the notes context whatever the column shows.
    await chord("Alt", "1");
    await js("window.openlogosShell.run({ command_id: 'view.notes_dock' })");
    await click(...(await centre(".dock .note-editor")));
    s = await snap();
    expect(p, s.well.focus === "notes_dock" && s.well.context === "notes" && s.active === 1, `dock focus ${JSON.stringify(s.well)}`);
    await click(...(await centre('#columns > .column[data-number="1"] .measure .text')));
    s = await snap();
    expect(p, s.well.focus === "tab" && s.well.context === "translation", `back in the column ${JSON.stringify(s.well)}`);
    const t2 = await tops();
    expect(p, t2.well === t0.well, `well top fixed through the contexts (${t0.well} → ${t2.well})`);
  },

  async "effective-tool-restore"(p) {
    await type("p");
    let s = await snap();
    expect(p, s.tool === "pen" && eq(s.well.pressed, ["tool.pen"]), `pen ${JSON.stringify(s.well.pressed)}`);
    await chord("Alt", "3");
    s = await snap();
    expect(p, s.tool === "pen" && s.well.effective === "type" && eq(s.well.pressed, ["tool.type"]), `notes: Pen stays stored, Type in effect (${s.tool}, ${s.well.effective})`);
    const tool = await js("return document.documentElement.dataset.tool");
    expect(p, tool === "type", `the text follows the effective tool (${tool})`);
    await type("e");
    s = await snap();
    expect(p, s.toast === "Eraser works on Bible text" && s.tool === "pen", `E in notes: toast "${s.toast}", stored ${s.tool}`);
    await chord("Alt", "2");
    s = await snap();
    expect(p, s.well.effective === "pen" && eq(s.well.pressed, ["tool.pen"]), `back on Bible text Pen is restored (${s.well.effective})`);
    await type("w");
    await chord("Alt", "1");
    s = await snap();
    expect(p, s.tool === "word_study" && s.well.effective === "select" && eq(s.well.pressed, ["tool.select"]), `Word study falls back to Select on a translation (${s.well.effective})`);
    await chord("Alt", "2");
    s = await snap();
    expect(p, s.well.effective === "word_study", `and comes back on Greek (${s.well.effective})`);
  },

  async "selection-group"(p) {
    const before = await js("const b = document.querySelector('#well button[data-command=\"tool.width\"]').getBoundingClientRect(); return [document.getElementById('well').getBoundingClientRect().top, b.top];");
    const pts = await phrasePoints(1, "eternal life");
    await drag(pts[0], pts[1]);
    await sleep(60);
    let s = await snap();
    const sel = s.well.groups.find((g) => g.kind === "selection");
    expect(p, sel && eq(sel.items, ["selection.highlight", "selection.collect", "selection.note", "selection.search", "selection.copy_reference", "selection.study"]), `selection group ${JSON.stringify(s.well.groups)}`);
    const after = await js("const b = document.querySelector('#well button[data-command=\"tool.width\"]').getBoundingClientRect(); return [document.getElementById('well').getBoundingClientRect().top, b.top];");
    expect(p, eq(before, after), `the group grows downward (${before} → ${after})`);
    await click(...(await centre('#well button[data-command="selection.copy_reference"]')));
    s = await snap();
    expect(p, s.toast === "Copied John 3:16" && s.copied === "John 3:16", `copy reference: toast "${s.toast}"`);
    await js("document.getSelection().removeAllRanges()");
    await sleep(60);
    s = await snap();
    expect(p, !s.well.groups.some((g) => g.kind === "selection"), "the group goes with the selection");
  },

  async "selection-note-and-search"(p) {
    const pts = await phrasePoints(1, "eternal life");
    await drag(pts[0], pts[1]);
    await sleep(60);
    await click(...(await centre('#well button[data-command="selection.note"]')));
    let s = await snap();
    const note = s.notes["fixture:note:1"] || "";
    expect(p, note.includes("<blockquote>“eternal life” — <a") && note.includes('data-reference="John.3.16"') && note.includes(">John 3:16</a></blockquote>"), `quote in the note tab ${note.slice(-200)}`);
    const shown = await js("return document.querySelector('#columns > .column[data-number=\"3\"] .note-editor blockquote')?.textContent");
    expect(p, shown === "“eternal life” — John 3:16", `on screen "${shown}"`);
    // With the note tab hidden the dock opens and takes the next quote.
    await js("window.openlogosShell.run({ command_id: 'tab.focus', args: { workspace_id: 'w1', tab_id: 't4' } }); window.openlogosShell.run({ command_id: 'tab.close' })");
    const more = await phrasePoints(1, "eternal life");
    await drag(more[0], more[1]);
    await sleep(60);
    await click(...(await centre('#well button[data-command="selection.note"]')));
    const dock = await js("const d = document.querySelector('.dock .note-editor'); return d ? d.querySelectorAll('blockquote').length : -1");
    expect(p, dock === 2, `the dock opened with the note (${dock} quotes)`);
    const again = await phrasePoints(1, "For God so loved");
    await drag(again[0], again[1]);
    await sleep(60);
    await chord("Meta", "e");
    s = await snap();
    const value = await js("return document.getElementById('palette-field').value");
    expect(p, s.paletteOpen && s.focus === "palette-field" && value === "For God so loved", `⌘E searches "${value}"`);
  },

  async "notes-bold-heading-link"(p) {
    await click(...(await centre('#columns > .column[data-number="3"] .note-editor')));
    let s = await snap();
    expect(p, s.active === 3 && s.well.context === "notes", `note tab focused (${s.well.context})`);
    await js("const e = document.querySelector('#columns > .column[data-number=\"3\"] .note-editor'); const r = document.createRange(); r.selectNodeContents(e); r.collapse(false); const s = document.getSelection(); s.removeAllRanges(); s.addRange(r);");
    await type(" ");
    await chord("Meta", "b");
    await type("grace");
    await chord("Meta", "b");
    await chord("Meta", "i");
    await type(" truth");
    await chord("Meta", "i");
    s = await snap();
    let note = s.notes["fixture:note:1"];
    expect(p, /<b>grace<\/b>/.test(note) && /<i>(&nbsp;| )truth<\/i>/.test(note), `⌘B ⌘I in the note: ${note.slice(-120)}`);
    expect(p, s.log.some((l) => l.command_id === "note.bold" && l.handled === false), "note.bold dispatched");
    await chord("Enter");
    await type("Heading");
    await click(...(await centre('#well button[data-command="note.heading"]')));
    await chord("Enter");
    await click(...(await centre('#well button[data-command="note.list"]')));
    await type("item");
    s = await snap();
    note = s.notes["fixture:note:1"];
    expect(p, note.includes("<h2>Heading</h2>") && /<ul><li>item/.test(note), `heading and list: ${note.slice(-160)}`);
    await click(...(await centre('#well button[data-command="note.link_passage"]')));
    s = await snap();
    const value = await js("return document.getElementById('palette-field').value");
    expect(p, s.paletteOpen && value === "Link passage ", `Link passage asks for the passage ("${value}")`);
    await type("jn 3:17");
    const row = await js("const r = document.querySelector('.row[aria-selected=true]'); return r && r.querySelector('.title').textContent");
    expect(p, row === "Link passage", `first row ${row}`);
    await chord("Enter");
    s = await snap();
    note = s.notes["fixture:note:1"];
    expect(p, note.includes('data-reference="John.3.17"') && note.includes(">John 3:17</a>"), `link inserted: ${note.slice(-200)}`);
    await click(...(await centre('#columns > .column[data-number="3"] a.passage-link')));
    s = await snap();
    expect(p, s.columns[0].reference === "John.3.17" && s.active === 1, `the link navigates column 1 (${s.columns[0].reference}, active ${s.active})`);
  },

  async "well-fits-short-window"(p) {
    // The tallest well (Greek: seven tools, ink, the selection group) in a short window.
    await setWindow(1024, 600);
    await chord("Alt", "2");
    const pts = await phrasePoints(2, "ὁ θεὸς");
    await drag(pts[0], pts[1]);
    await sleep(60);
    const r = await js("const w = document.getElementById('well').getBoundingClientRect(); return [w.top, w.bottom, innerHeight, document.querySelectorAll('#well .group').length];");
    expect(p, r[3] === 3 && r[1] <= r[2] && r[0] >= 44, `well ${r[0]}–${r[1]} in ${r[2]} (${r[3]} groups)`);
    await setWindow(width, height);
  },

  async "word-study-lexicon"(p) {
    const word = async (column, w) => { const pts = await phrasePoints(column, w); return [(pts[0][0] + pts[1][0]) / 2, (pts[0][1] + pts[1][1]) / 2]; };
    const page = () => js("const l = document.querySelector('#columns .lexicon'); return l && [...l.children].map((e) => e.textContent)");
    await chord("Alt", "2");
    await type("w");
    let s = await snap();
    expect(p, s.tool === "word_study" && eq(s.well.pressed, ["tool.word_study"]), `W on Greek (${s.well.pressed})`);
    await click(...(await word(2, "ἠγάπησεν")));
    s = await snap();
    expect(p, s.active === 2 && eq(s.dom[2].tabs, ["Note", "Lexicon Jn 3"]) && s.dom[2].shown === "Lexicon Jn 3", `a Lexicon tab in column 3: ${JSON.stringify(s.dom.map((c) => c.tabs))}`);
    let shown = await page();
    expect(p, eq(shown, ["ἠγάπησεν", "Greek · John 3:16", "Lexicon entries arrive with the lexicon import."]), `lexicon page ${JSON.stringify(shown)}`);
    expect(p, s.well.context === "original" && s.well.effective === "word_study", `focus stays on the Greek column, Word study still in effect (${s.well.context}, ${s.well.effective})`);
    await click(...(await word(2, "κόσμον")));
    s = await snap();
    shown = await page();
    expect(p, eq(s.dom[2].tabs, ["Note", "Lexicon Jn 3"]) && shown && shown[0] === "κόσμον", `reused: ${JSON.stringify(s.dom[2].tabs)} ${JSON.stringify(shown)}`);
    expect(p, s.log.some((l) => l.command_id === "tab.open_lexicon" && l.handled), "tab.open_lexicon dispatched");
    // Hebrew, one column: the lexicon opens in a new column 2.
    await chord("Alt", "Shift", "]");
    await click(...(await word(1, "אֱלֹהִ֑ים")));
    s = await snap();
    shown = await page();
    expect(p, s.dom.length === 2 && s.active === 1 && shown && shown[0] === "אֱלֹהִ֑ים" && shown[1] === "Hebrew · Gen 1:1", `Hebrew: ${s.dom.length} columns, ${JSON.stringify(shown)}`);
    // Word study does nothing on a translation (Select is in effect there).
    await chord("Alt", "Shift", "[");
    await click(...(await word(1, "loved")));
    s = await snap();
    expect(p, s.active === 1 && s.dom[2].tabs.length === 2, `no lexicon from BSB (active ${s.active})`);
  },

  // MARK: B077 — the context menu

  async "menu-word-translation"(p) {
    const before = (await snap()).log.length;
    await rightClick(...(await wordPoint(1, "loved")));
    let s = await snap();
    const m = s.contextMenu && s.contextMenu.menus[0];
    expect(p, m && m.title === "loved" && m.sections === 4, `word menu header ${JSON.stringify(m && [m.title, m.sections])}`);
    const labels = m ? m.rows.map((r) => r.label) : [];
    expect(p, eq(labels, ["Search “loved” in this edition", "Search “loved” in all editions", "Search lemma", "Word study", "Find in this chapter", "Compare in SBLGNT", "Open John 3:16 in", "Cross references", "Highlight", "Collect", "Note on it", "Copy"]), `rows ${JSON.stringify(labels)}`);
    const row = (l) => m.rows.find((r) => r.label === l) || {};
    expect(p, row("Word study").disabled === "Word study works on Greek and Hebrew text", `Word study disabled with words: ${JSON.stringify(row("Word study"))}`);
    expect(p, row("Search lemma").disabled === "Needs the lexicon import" && row("Cross references").disabled === "Needs cross-reference data", "lemma and cross references say what they need");
    expect(p, row("Highlight").submenu && row("Copy").submenu && row("Open John 3:16 in").submenu && row("Collect").end === "⌘⇧C", `submenus and keys ${JSON.stringify([row("Highlight"), row("Collect")])}`);
    expect(p, s.selection === "loved", `the word is selected as the menu opens ("${s.selection}")`);
    expect(p, eq(sorted(s.target), sorted({ kind: "word", text: "loved", reference: "John.3.16", segment_id: "John.3.16", scalar_start: 11, scalar_end: 16, resource_id: "bsb", tab_id: "t1" })), `target ${JSON.stringify(s.target)}`);
    expect(p, s.log.length === before, "opening the menu dispatches nothing");
    const inside = s.contextMenu.menus[0].rect;
    expect(p, inside[0] >= 8 && inside[1] >= 8 && inside[2] <= width - 8 && inside[3] <= height - 8, `inside the viewport ${inside}`);
    await chord("Escape");
    s = await snap();
    expect(p, !s.contextMenu && s.selection === "loved", `Esc closes, the word stays selected (${s.selection})`);
  },

  async "menu-word-greek-focus-on-choose"(p) {
    await rightClick(...(await wordPoint(2, "ἠγάπησεν")));
    let s = await snap();
    const rows = s.contextMenu.menus[0].rows;
    const study = rows.find((r) => r.label === "Word study");
    expect(p, study && !study.disabled, `Word study enabled on Greek ${JSON.stringify(study)}`);
    expect(p, rows.some((r) => r.label === "Compare in BSB"), "Compare in BSB");
    expect(p, s.active === 1, `opening on column 2 does not focus it (active ${s.active})`);
    await chooseRow("Word study");
    s = await snap();
    expect(p, s.active === 2 && s.dom[2].shown === "Lexicon Jn 3", `focus first, then Word study: active ${s.active}, column 3 ${s.dom[2].shown}`);
    const ids = s.log.slice(-2).map((l) => l.command_id);
    expect(p, eq(ids, ["tab.focus", "tab.open_lexicon"]), `dispatched ${ids}`);
  },

  async "menu-compare-and-undo"(p) {
    // Column 2 shows the note once SBLGNT is moved out, so Compare opens the edition beside column 1.
    await chord("Alt", "2");
    await js("window.openlogosShell.run({ command_id: 'column.close' })");
    await chord("Alt", "1");
    let s = await snap();
    const columns = s.dom.length;
    await rightClick(...(await wordPoint(1, "world")));
    await chooseRow("Compare in SBLGNT");
    s = await snap();
    const where = s.columns.findIndex((c) => c.shown.startsWith("SBLGNT"));
    expect(p, where === 1 && s.columns[1].reference === "John.3.16", `SBLGNT opens in column 2: ${JSON.stringify(s.columns.map((c) => [c.shown, c.reference]))}`);
    expect(p, s.log.some((l) => l.command_id === "tab.open_edition" && l.handled), "tab.open_edition is the core's");
    await chord("Meta", "z");
    s = await snap();
    expect(p, !s.columns.some((c) => c.shown.startsWith("SBLGNT")) && s.dom.length === columns, `⌘Z takes it back: ${JSON.stringify(s.columns.map((c) => c.shown))}`);
  },

  async "menu-highlight-submenu-keyboard"(p) {
    await rightClick(...(await wordPoint(1, "eternal")));
    for (let i = 0; i < 9; i++) await chord("ArrowDown");
    let s = await snap();
    expect(p, s.contextMenu.focused === "Highlight", `↓ ×9 reaches Highlight (${s.contextMenu.focused})`);
    await chord("ArrowRight");
    s = await snap();
    const sub = s.contextMenu.menus[1];
    expect(p, sub && eq(sub.rows.map((r) => [r.label, r.checked, r.swatch]), [["Yellow", "true", true], ["Green", "false", true], ["Blue", "false", true], ["Pink", "false", true]]) && s.contextMenu.focused === "Yellow", `→ opens the swatches ${JSON.stringify(sub && sub.rows)}`);
    expect(p, sub && sub.rect[0] >= s.contextMenu.menus[0].rect[2] - 4, "the submenu opens to the right");
    await chord("ArrowLeft");
    s = await snap();
    expect(p, s.contextMenu.menus.length === 1 && s.contextMenu.focused === "Highlight", "← closes it");
    await chord("ArrowRight");
    await chord("ArrowDown");
    await chord("Enter");
    s = await snap();
    expect(p, !s.contextMenu && s.highlights.length === 1 && s.highlights[0].swatch === "green" && s.highlights[0].anchors[0].exact === "eternal", `green highlight on the word ${JSON.stringify(s.highlights)}`);
    // Type-ahead: "c" goes to the next row starting with C, "co" to Copy.
    await rightClick(...(await wordPoint(1, "world")));
    await type("c");
    s = await snap();
    expect(p, s.contextMenu.focused === "Compare in SBLGNT", `type-ahead c → ${s.contextMenu.focused}`);
    await type("op");
    s = await snap();
    expect(p, s.contextMenu.focused === "Copy" && s.tool === "select", `type-ahead cop → ${s.contextMenu.focused}, no tool key (tool ${s.tool})`);
    await chord("Escape");
  },

  async "menu-selection-copy-passage"(p) {
    const pts = await phrasePoints(1, "eternal life");
    await drag(pts[0], pts[1]);
    await sleep(60);
    await rightClick((pts[0][0] + pts[1][0]) / 2, pts[0][1]);
    let s = await snap();
    expect(p, s.target && s.target.kind === "selection" && s.target.text === "eternal life" && s.target.reference === "John.3.16" && s.target.resource_id === "bsb", `selection target ${JSON.stringify(s.target)}`);
    const labels = s.contextMenu.menus[0].rows.map((r) => r.label);
    expect(p, eq(labels.slice(-2), ["Copy", "Copy with reference"]) && labels.includes("Study"), `selection rows ${JSON.stringify(labels)}`);
    await chooseRow("Copy with reference");
    s = await snap();
    expect(p, s.copied === "“eternal life” — John 3:16 (BSB)" && s.toast === "Copied John 3:16 with its text", `copied "${s.copied}", toast "${s.toast}"`);
    const board = await jsAsync("const done = arguments[arguments.length - 1]; navigator.clipboard.readText().then(done, (e) => done('unreadable: ' + e.name))");
    expect(p, board === s.copied || board.startsWith("unreadable"), `system clipboard "${board}"`);
    // Copy ▸ Copy verse with reference on a word reads the verse from the text source.
    await rightClick(...(await wordPoint(1, "condemn")));
    await chooseRow("Copy");
    await chooseRow("Copy verse with reference", 1);
    s = await snap();
    expect(p, s.copied.startsWith("“For God did not send His Son into the world to condemn") && s.copied.endsWith("” — John 3:17 (BSB)"), `verse copied "${s.copied}"`);
    await js("document.getSelection().removeAllRanges()"); // else the right-click is on the selection
    await rightClick(...(await wordPoint(1, "condemn")));
    await chooseRow("Copy");
    await chooseRow("Copy reference", 1);
    s = await snap();
    expect(p, s.copied === "John 3:17" && s.toast === "Copied John 3:17", `reference "${s.copied}" toast "${s.toast}"`);
    p.clipboard = board;
  },

  async "menu-search-scope"(p) {
    await rightClick(...(await wordPoint(1, "world")));
    await chooseRow("Search “world” in this edition");
    let s = await snap();
    const titles = () => js("return [...document.querySelectorAll('.results .row .title')].map((t) => t.textContent)");
    let rows = await titles();
    expect(p, s.paletteOpen && s.paletteValue === "#world" && s.scope && s.scope.chip === "This edition · BSB", `palette "${s.paletteValue}" scope ${JSON.stringify(s.scope)}`);
    expect(p, rows.length === 2 && rows.every((t) => t.startsWith("BSB · ")), `edition rows ${JSON.stringify(rows)}`);
    await chord("Escape");
    await rightClick(...(await wordPoint(2, "κόσμον")));
    await chooseRow("Search “κόσμον” in all editions");
    s = await snap();
    rows = await titles();
    expect(p, s.scope && s.scope.chip === "All editions" && rows.some((t) => t.startsWith("SBLGNT · ")), `all editions: ${JSON.stringify(s.scope)} ${JSON.stringify(rows)}`);
    await click(...(await centre("#scope-chip button")));
    s = await snap();
    expect(p, !s.scope && s.paletteOpen, "× clears the scope");
    await chord("Escape");
    s = await snap();
    expect(p, !s.scope, "closing the palette clears the scope");
  },

  async "menu-find-in-chapter"(p) {
    await rightClick(...(await wordPoint(1, "world")));
    await chooseRow("Find in this chapter");
    let s = await snap();
    expect(p, s.find && s.find.count === 4 && s.find.index === 0 && s.find.current === "world" && s.toast === "4 matches in John 3", `find ${JSON.stringify(s.find)} toast "${s.toast}"`);
    const painted = await js("const h = CSS.highlights.get('ol-find'); return h ? h.size : 0");
    expect(p, painted === 4, `4 ranges painted (${painted})`);
    await chord("Meta", "g");
    s = await snap();
    expect(p, s.find.index === 1 && s.toast === "2 of 4 in John 3", `⌘G → ${s.find.index} "${s.toast}"`);
    await chord("Meta", "Shift", "g");
    await chord("Meta", "Shift", "g");
    s = await snap();
    expect(p, s.find.index === 3, `⇧⌘G wraps back (${s.find.index})`);
    await chord("Escape");
    s = await snap();
    const left = await js("return CSS.highlights.has('ol-find')");
    expect(p, !s.find && !left, "Esc clears the matches");
  },

  async "menu-tab-close-others-undo"(p) {
    await rightClick(...(await centre('#columns > .column[data-number="1"] .tab:nth-child(1)')));
    let s = await snap();
    const m = s.contextMenu.menus[0];
    expect(p, m.title === "BSB · Jn 3:16" && eq(m.rows.map((r) => r.label), ["Close tab", "Close other tabs", "Duplicate to", "Move to", "Pin", "Follow active passage", "Link group"]), `tab menu ${JSON.stringify([m.title, m.rows.map((r) => r.label)])}`);
    await chooseRow("Close other tabs");
    s = await snap();
    expect(p, eq(s.dom[0].tabs, ["BSB Jn 3"]), `close others: ${s.dom[0].tabs}`);
    await chord("Meta", "z");
    s = await snap();
    expect(p, eq(s.dom[0].tabs, ["BSB Jn 3", "WLC Gen 1"]), `⌘Z: ${s.dom[0].tabs}`);
    // Move to ▸ on a tab in another column: the column's own row is disabled with words.
    await rightClick(...(await centre('#columns > .column[data-number="2"] .tab')));
    await chooseRow("Move to");
    const sub = await menuRows(1);
    const own = sub && sub.find((r) => r.label.startsWith("Column 2"));
    expect(p, own && own.disabled === "The tab is already in column 2" && own.end === "⌥⇧2", `own column disabled: ${JSON.stringify(own)}`);
    await chooseRow("Column 3 · Note", 1);
    s = await snap();
    expect(p, eq(s.dom.map((c) => c.tabs), [["BSB Jn 3", "WLC Gen 1"], ["Note", "SBLGNT Jn 3"]]) || eq(s.dom[2] && s.dom[2].tabs, ["Note", "SBLGNT Jn 3"]), `moved: ${JSON.stringify(s.dom.map((c) => c.tabs))}`);
  },

  async "menu-note-link-remove"(p) {
    const pts = await phrasePoints(1, "eternal life");
    await drag(pts[0], pts[1]);
    await sleep(60);
    await click(...(await centre('#well button[data-command="selection.note"]')));
    await rightClick(...(await centre('#columns > .column[data-number="3"] a.passage-link')));
    let s = await snap();
    const m = s.contextMenu.menus[0];
    expect(p, s.target.kind === "note_link" && s.target.reference === "John.3.16" && s.target.tab_id === "t4", `link target ${JSON.stringify(s.target)}`);
    expect(p, m.title === "John 3:16" && eq(m.rows.map((r) => r.label), ["Open John 3:16 in", "Compare in", "Copy reference", "Remove link"]), `link menu ${JSON.stringify([m.title, m.rows.map((r) => r.label)])}`);
    await chooseRow("Remove link");
    s = await snap();
    const note = s.notes["fixture:note:1"];
    expect(p, !note.includes("passage-link") && note.includes("“eternal life” — John 3:16</blockquote>") && s.toast === "Removed link to John 3:16", `link removed: ${note.slice(-120)} toast "${s.toast}"`);
    // Note text: Edit, Format, Search.
    await rightClick(...(await centre('#columns > .column[data-number="3"] .note-editor p')));
    s = await snap();
    expect(p, s.target.kind === "note_text" && eq(s.contextMenu.menus[0].rows.map((r) => r.label).slice(0, 5), ["Cut", "Copy", "Paste", "Bold", "Italic"]), `note text menu ${JSON.stringify(s.contextMenu.menus[0].rows.map((r) => [r.label, r.disabled]))}`);
    await chord("Escape");
  },

  async "menu-column-background-theme"(p) {
    await rightClick(...(await centre('#columns > .column[data-number="2"] .badge')));
    let s = await snap();
    expect(p, s.contextMenu.menus[0].title === "Column 2" && s.contextMenu.menus[0].rows[0].label === "Focus column 2" && s.active === 1, `column menu ${JSON.stringify(s.contextMenu.menus[0].rows.map((r) => r.label))}`);
    await chooseRow("Move left");
    s = await snap();
    expect(p, s.dom[0].shown === "SBLGNT Jn 3" && s.active === 1, `column 2 moved left: ${JSON.stringify(s.dom.map((c) => c.shown))}`);
    await chord("Meta", "z");
    // B078: the stream fills the column; its sticky header is background.
    const box = await js("const r = document.querySelector('#columns > .column[data-number=\"1\"] .reading-header').getBoundingClientRect(); return [r.right - 30, r.top + r.height / 2];");
    await rightClick(...box);
    s = await snap();
    expect(p, s.target.kind === "background" && s.target.column === 1 && eq(s.contextMenu.menus[0].rows.map((r) => r.label), ["New tab", "Paste passage", "Text size", "Theme"]), `background ${JSON.stringify(s.target)} ${JSON.stringify(s.contextMenu.menus[0].rows.map((r) => r.label))}`);
    await chooseRow("Theme");
    const themes = await menuRows(1);
    expect(p, themes && eq(themes.map((r) => [r.label, r.checked]), [["System", "false"], ["Light", "true"], ["Dark", "false"], ["Sepia", "false"]]), `theme submenu ${JSON.stringify(themes)}`);
    await chooseRow("Dark", 1);
    s = await snap();
    expect(p, s.theme === "dark", `theme ${s.theme}`);
  },

  async "menu-paste-passage"(p) {
    await rightClick(...(await wordPoint(1, "condemn")));
    await chooseRow("Copy");
    await chooseRow("Copy reference", 1);
    await chord("Alt", "Shift", "]");
    const box = await js("const r = document.querySelector('#columns > .column .reading-header').getBoundingClientRect(); return [r.right - 30, r.top + r.height / 2];");
    await rightClick(...box);
    await chooseRow("Paste passage");
    await sleep(100);
    const s = await snap();
    expect(p, s.columns[0].reference === "John.3.17" && s.toast === "Opened John 3:17", `paste passage navigates WLC to ${s.columns[0].reference}, toast "${s.toast}"`);
  },

  async "menu-escape-hatch"(p) {
    await js("window.__prevented = null; window.addEventListener('contextmenu', (e) => { window.__prevented = e.defaultPrevented; })");
    await rightClick(...(await wordPoint(1, "loved")), { shift: true });
    const s = await snap();
    const prevented = await js("return window.__prevented");
    expect(p, !s.contextMenu && prevented === false, `⇧-right-click leaves the platform menu (menu ${!!s.contextMenu}, defaultPrevented ${prevented})`);
    await rightClick(...(await wordPoint(1, "loved")));
    const again = await js("return window.__prevented");
    expect(p, again === true && (await snap()).contextMenu, "a plain right-click is the OpenLogos menu");
    // In the palette field the platform menu stays.
    await rightClick(...(await centre("#palette-field")));
    expect(p, (await js("return window.__prevented")) === false && !(await snap()).contextMenu, "the palette field keeps the platform menu");
  },

  async "menu-keyboard-open"(p) {
    await click(...(await centre('#columns > .column[data-number="1"] .tab:nth-child(1)')));
    await chord("Shift", "F10");
    let s = await snap();
    expect(p, s.target && s.target.kind === "tab" && s.target.tab_id === "t1" && s.contextMenu.focused === "Close tab", `⇧F10 on the focused tab: ${JSON.stringify(s.target)} focused ${s.contextMenu && s.contextMenu.focused}`);
    await chord("ArrowDown");
    s = await snap();
    expect(p, s.contextMenu.focused === "Close other tabs", `↓ ${s.contextMenu.focused}`);
    await chord("Escape");
    s = await snap();
    expect(p, !s.contextMenu && s.focus === "tab", `Esc returns focus to the tab (${s.focus})`);
    const pts = await phrasePoints(1, "eternal life");
    await drag(pts[0], pts[1]);
    await sleep(60);
    await chord("Shift", "F10");
    s = await snap();
    expect(p, s.target && s.target.kind === "selection" && s.target.text === "eternal life", `⇧F10 with a selection: ${JSON.stringify(s.target)}`);
    await chord("Escape");
    // The context-menu key (DOM key "ContextMenu") on a collapsed caret in the text: the word there.
    await js("const t = document.querySelector('#columns > .column[data-number=\"1\"] [data-segment=\"John.3.16\"] .t').firstChild; const r = document.createRange(); r.setStart(t, 13); r.collapse(true); const s = document.getSelection(); s.removeAllRanges(); s.addRange(r); document.activeElement.dispatchEvent(new KeyboardEvent('keydown', { key: 'ContextMenu', bubbles: true }));");
    s = await snap();
    expect(p, s.target && s.target.kind === "word" && s.target.text === "loved" && s.selection === "loved", `menu key at the caret: ${JSON.stringify(s.target)}`);
    await chord("Escape");
  },

  async "menu-closes"(p) {
    await rightClick(...(await wordPoint(1, "loved")));
    await click(...(await centre('#columns > .column[data-number="2"] .identity .name')));
    expect(p, !(await snap()).contextMenu, "a press outside closes it");
    await rightClick(...(await wordPoint(1, "loved")));
    await js("document.querySelector('#columns > .column .scroller').dispatchEvent(new Event('scroll'))");
    expect(p, !(await snap()).contextMenu, "a scroll closes it");
    // Near the bottom-right corner the menu flips to stay inside the viewport.
    await setWindow(1024, 600);
    const corner = await js("const r = document.querySelector('#columns > .column[data-number=\"3\"] .scroller').getBoundingClientRect(); return [r.right - 6, r.bottom - 6];");
    await rightClick(...corner);
    const m = (await snap()).contextMenu;
    const r = m && m.menus[0].rect;
    expect(p, r && r[0] >= 8 && r[1] >= 8 && r[2] <= 1024 - 8 && r[3] <= 600 - 8, `flipped inside 1024×600: ${r}`);
    await chord("Escape");
    await setWindow(width, height);
  },

  // MARK: B078 continuous reading (specs/23)

  async "reading-jump-flash-mark"(p) {
    await palette("John 3:16");
    const s = await snap();
    const [line, top] = await readingLine(1, "John.3.16");
    expect(p, Math.abs(top - line) <= 2, `John 3:16 on the reading line (${top} vs ${line})`);
    expect(p, s.reading[0].top.segment_id === "John.3.16" && s.reading[1].top.segment_id === "John.3.16", `tops ${JSON.stringify(s.reading.map((r) => r && r.top))}`);
    expect(p, eq(s.reading[0].flashing, ["John.3.16"]) && eq(s.reading[0].marked, ["John.3.16"]), `flash ${s.reading[0].flashing} mark ${s.reading[0].marked}`);
    expect(p, eq(s.reading[1].marked, ["John.3.16"]), `the follower is marked too: ${s.reading[1].marked}`);
    expect(p, s.reading[0].header === "BSB · Jn 3" && s.reading[1].header === "SBLGNT · Jn 3", `headers ${s.reading.map((r) => r && r.header)}`);
    expect(p, eq(s.reading[0].blocks.slice(0, 5), ["John.1", "John.2", "John.3", "John.4", "John.5"]), `window ${s.reading[0].blocks}`);
    await sleep(1400);
    const later = await snap();
    expect(p, later.reading[0].flashing.length === 0 && eq(later.reading[0].marked, ["John.3.16"]), `flash once, mark stays: ${later.reading[0].flashing} ${later.reading[0].marked}`);
  },

  async "reading-range-and-back"(p) {
    await palette("John 3:16-18");
    let s = await snap();
    expect(p, eq(s.reading[0].marked, ["John.3.16", "John.3.17", "John.3.18"]), `range marked ${s.reading[0].marked}`);
    expect(p, s.dom[0].identity.includes("John 3:16–18"), `identity ${s.dom[0].identity}`);
    await palette("Ps 23");
    s = await snap();
    expect(p, s.reading[0].top.segment_id === "Ps.23.1" && s.reading[0].marked.length === 0 && s.reading[0].header === "BSB · Ps 23", `Ps 23 ${JSON.stringify(s.reading[0])}`);
    await palette("Jude 3");
    s = await snap();
    expect(p, s.reading[0].top.segment_id === "Jude.1.3" && eq(s.reading[0].marked, ["Jude.1.3"]), `Jude 3 ${JSON.stringify(s.reading[0].top)}`);
    await chord("Meta", "[");
    s = await snap();
    expect(p, s.columns[0].reference === "Ps.23" && s.reading[0].marked.length === 0, `back to Ps 23: ${s.columns[0].reference} ${s.reading[0].marked}`);
    await chord("Meta", "[");
    s = await snap();
    expect(p, s.columns[0].reference === "John.3.16-John.3.18" && s.reading[0].top.segment_id === "John.3.16" && s.reading[0].marked.length === 0, `back to Jn 3:16–18 (mark cleared): ${s.columns[0].reference} ${JSON.stringify(s.reading[0].top)} ${s.reading[0].marked}`);
    await chord("Meta", "]");
    s = await snap();
    expect(p, s.columns[0].reference === "Ps.23", `forward ${s.columns[0].reference}`);
    // A whole book opens at its first verse: the palette's Passage row for "Genesis".
    await palette("Genesis");
    s = await snap();
    expect(p, s.reading[0].top.segment_id === "Gen.1.1" && s.reading[0].header === "BSB · Gen 1", `Genesis ${JSON.stringify(s.reading[0].top)}`);
    expect(p, s.reading[1].error && s.reading[1].error.includes("Genesis is not in SBLGNT"), `SBLGNT has no Genesis: ${JSON.stringify(s.reading[1])}`);
  },

  async "reading-palette-books"(p) {
    await chord("Meta", "k");
    await type("Jo");
    const rows = await js("return [...document.querySelectorAll('.results h3, .results .row .title')].map((e) => e.textContent)");
    const passage = rows.slice(rows.indexOf("Passage") + 1, rows.indexOf("Passage") + 6);
    expect(p, rows[0] === "Passage" && passage.includes("John"), `"Jo" offers John: ${JSON.stringify(rows)}`);
    await chord("Meta", "a");
    await type("Genesis");
    const first = await js("const r = document.querySelector('.row[aria-selected=true]'); return [document.querySelector('.results h3').textContent, r && r.querySelector('.title').textContent]");
    expect(p, eq(first, ["Passage", "Genesis"]), `first row ${JSON.stringify(first)}`);
    await chord("Enter");
    await sleep(60);
    const s = await snap();
    const [line, top] = await readingLine(1, "Gen.1.1");
    expect(p, s.columns[0].reference === "Gen.1" && s.reading[0].top.segment_id === "Gen.1.1" && Math.abs(top - line) <= 2 && s.reading[0].header === "BSB · Gen 1", `Genesis → Gen 1:1: ${s.columns[0].reference} ${JSON.stringify(s.reading[0].top)}`);
    expect(p, s.reading[1].error && s.reading[1].error.includes("Genesis is not in SBLGNT"), `SBLGNT: ${JSON.stringify(s.reading[1])}`);
  },

  async "reading-invalid-reference"(p) {
    await palette("John 22");
    const s = await snap();
    expect(p, s.reading[0].error && s.reading[0].error.startsWith("Berean Standard Bible · John 22") && s.reading[0].error.includes("John has no chapter 22."), `not in edition: ${JSON.stringify(s.reading[0])}`);
    expect(p, s.reading[1].error && s.reading[1].error.includes("John has no chapter 22"), `follower too: ${JSON.stringify(s.reading[1])}`);
    expect(p, !s.dom[0].text, "no text of another chapter");
    await chord("Meta", "[");
    const back = await snap();
    expect(p, back.reading[0].blocks && back.reading[0].top.segment_id === "John.3.16", `back to the text: ${JSON.stringify(back.reading[0].top)}`);
  },

  async "reading-scroll-down-across-books"(p) {
    await navigate("John.21.20");
    const r = await scrollStream(1, 60, 400, "Acts.2");
    expect(p, r.chapters.includes("Acts.1") && r.chapters.includes("Acts.2"), `reached Acts: ${r.chapters}`);
    expect(p, r.maxJump <= 2, `the text under the eye moved ${r.maxJump} px`);
    expect(p, r.headers.includes("BSB · Acts 1"), `headers ${r.headers}`);
    const s = await snap();
    expect(p, s.dom[0].shown === "BSB Acts 2" && s.dom[0].tabs[0] === "BSB Acts 2", `tab label ${s.dom[0].shown}`);
    expect(p, /^Acts 2:\d+$/.test(s.dom[0].identity.match(/Acts [\d:]+/)?.[0] || ""), `identity ${s.dom[0].identity}`);
    const acts = await js("return [...document.querySelectorAll('#columns > .column[data-number=\"1\"] .chapter[data-chapter=\"Acts.1\"] .book-title')].map((h) => h.textContent)");
    expect(p, eq(acts, ["Acts"]), `book heading on Acts 1: ${acts}`);
    expect(p, s.reading[1].top && s.reading[1].top.segment_id.startsWith("Acts.2.") && s.reading[1].top.segment_id === s.reading[0].top.segment_id, `SBLGNT follows: ${JSON.stringify(s.reading.map((x) => x && x.top))}`);
    expect(p, r.maxBlocks <= 9, `blocks ≤ 9 (${r.maxBlocks})`);
  },

  async "reading-scroll-up-across-books"(p) {
    await navigate("John.1.3");
    const r = await scrollStream(1, -60, 400, "Luke.23");
    expect(p, r.chapters.includes("Luke.24") && r.chapters.includes("Luke.23"), `reached Luke: ${r.chapters}`);
    expect(p, r.maxJump <= 2, `the text under the eye moved ${r.maxJump} px`);
    expect(p, r.headers.includes("BSB · Lk 24"), `headers ${r.headers}`);
    const s = await snap();
    expect(p, s.dom[0].shown === "BSB Lk 23", `tab label ${s.dom[0].shown}`);
    await navigate("Gen.2.3");
    const g = await scrollStream(1, -60, 200, "Gen.1");
    expect(p, g.chapters.includes("Gen.1") && g.maxJump <= 2, `Genesis 2 → 1 without a jump: ${g.chapters} ${g.maxJump}`);
  },

  async "reading-linked-follow"(p) {
    await navigate("John.3.16");
    const r = await scrollStream(1, 50, 120);
    const s = await snap();
    const [a, b] = [s.reading[0].top, s.reading[1].top];
    expect(p, a && b && a.segment_id === b.segment_id && a.segment_id !== "John.3.16", `SBLGNT on the BSB's verse: ${JSON.stringify([a, b])}`);
    const [line, top] = await readingLine(2, b.segment_id);
    expect(p, Math.abs(top - line) <= 2, `the follower's verse on its reading line (${top} vs ${line})`);
    const records = s.log.filter((l) => l.command_id === "tab.record_scroll");
    expect(p, records.length > 0 && records.every((l) => l.tab_id === "t1"), `only the driver reports: ${JSON.stringify(records.slice(-4))}`);
    expect(p, s.columns[1].reference === a.segment_id, `the follower's passage ${s.columns[1].reference}`);
    expect(p, r.chapters.length >= 2, `scrolled through ${r.chapters}`);
    // Scrolling the follower drives the group the other way.
    await scrollStream(2, 300, 3);
    const t = await snap();
    expect(p, t.reading[0].top.segment_id === t.reading[1].top.segment_id && t.log.some((l) => l.command_id === "tab.record_scroll" && l.tab_id === "t3"), `column 2 drives: ${JSON.stringify([t.reading[0].top, t.reading[1].top])}`);
  },

  async "reading-wlc-follows-its-scheme"(p) {
    // WLC (Gen 1:1, pinned behind the BSB in column 1) joins group 1 in column 3.
    await js("const r = window.openlogosShell.run; r({ command_id: 'tab.focus', args: { workspace_id: 'w1', tab_id: 't2' } }); r({ command_id: 'tab.link', args: { group: '1' } }); r({ command_id: 'tab.move_to_column', args: { column: 3 } }); r({ command_id: 'column.focus', args: { column: 1 } });");
    await palette("Mal 4");
    let s = await snap();
    expect(p, s.reading[2] && s.reading[2].top && s.reading[2].top.segment_id === "Mal.3.19" && s.reading[2].header === "WLC · Mal 3", `WLC at Malachi 3:19: ${JSON.stringify(s.reading[2])}`);
    expect(p, s.reading[1].error && s.reading[1].error.includes("not in SBLGNT"), `SBLGNT has no Malachi: ${JSON.stringify(s.reading[1])}`);
    // Scrolling the BSB from Malachi 4 into Matthew 1; the WLC stays at its last verse (its edition ends).
    const r = await scrollStream(1, 60, 200, "Matt.1");
    s = await snap();
    expect(p, r.chapters.includes("Matt.1") && s.reading[0].header === "BSB · Mt 1", `BSB into Matthew: ${r.chapters} ${s.reading[0].header}`);
    expect(p, r.maxJump <= 2, `no jump ${r.maxJump}`);
  },

  async "reading-psalm-poetry"(p) {
    await navigate("Ps.3");
    const s = await snap();
    expect(p, s.reading[0].top.segment_id === "Ps.3.0" && s.reading[0].header === "BSB · Ps 3", `a titled Psalm opens at its title: ${JSON.stringify(s.reading[0].top)}`);
    const block = await js(`const b = document.querySelector('#columns > .column[data-number="1"] .chapter[data-chapter="Ps.3"]');
      return { heading: b.querySelector('.chapter-heading').textContent, book: !!b.querySelector('.book-title'), title: b.querySelector('p.superscription [data-segment]').dataset.segment,
        titleLabel: b.querySelector('p.superscription .marker') ? 1 : 0, lines: b.querySelectorAll('p.line').length, verses: b.querySelectorAll('[data-segment]').length,
        book1: [...document.querySelectorAll('#columns > .column[data-number="1"] .chapter[data-chapter="Ps.1"] .book-title')].map((h) => h.textContent) };`);
    expect(p, block.heading === "Psalm 3" && !block.book && block.title === "Ps.3.0" && block.titleLabel === 0, `Psalm 3 heading and title ${JSON.stringify(block)}`);
    expect(p, block.lines === 8 && block.verses === 9, `one line per verse ${JSON.stringify(block)}`);
    expect(p, eq(block.book1, ["Psalms"]), `book title on Psalm 1 only: ${block.book1}`);
  },

  async "reading-placeholder-actions"(p) {
    const pts = await phrasePoints(1, "Placeholder verse — John 3:18");
    await drag(pts[0], pts[1]);
    await sleep(80);
    let s = await snap();
    const disabled = await js("return [...document.querySelectorAll('#well button[aria-disabled]')].map((b) => b.dataset.command + ': ' + b.title)");
    for (const id of ["selection.highlight", "selection.collect", "selection.note", "selection.study"]) {
      expect(p, disabled.some((d) => d.startsWith(id) && d.endsWith("Placeholder text can’t be quoted or collected")), `${id} disabled with words: ${JSON.stringify(disabled)}`);
    }
    await click(...(await centre('#well button[data-command="selection.collect"]')));
    s = await snap();
    expect(p, s.toast === "Placeholder text can’t be quoted or collected", `pressing it says why: ${s.toast}`);
    await rightClick((pts[0][0] + pts[1][0]) / 2, pts[0][1]);
    const rows = await menuRows();
    const row = (label) => rows.find((r) => r.label === label);
    expect(p, row("Collect") && row("Collect").disabled === "Placeholder text can’t be quoted or collected" && row("Note on it").disabled && row("Highlight").disabled && !row("Highlight").submenu, `menu rows ${JSON.stringify(rows.map((r) => [r.label, r.disabled]))}`);
    await chord("Escape");
    await js("document.getSelection().removeAllRanges()");
    await type("h");
    await drag(pts[0], pts[1]);
    await sleep(80);
    s = await snap();
    expect(p, s.highlights.length === 0 && s.toast === "Placeholder text can’t be quoted or collected", `the highlight tool refuses: ${JSON.stringify(s.highlights)} ${s.toast}`);
    // A real verse in the same chapter still highlights.
    const real = await phrasePoints(1, "eternal life");
    await drag(real[0], real[1]);
    await sleep(80);
    s = await snap();
    expect(p, s.highlights.length === 1, `fixture verse highlights: ${JSON.stringify(s.highlights)}`);
  },

  async "reading-highlight-and-ink-survive-unload"(p) {
    await type("h");
    const pts = await phrasePoints(1, "eternal life");
    await drag(pts[0], pts[1]);
    await sleep(80);
    await type("p");
    const at = await js("const el = document.querySelector('#columns > .column[data-number=\"1\"] [data-segment=\"John.3.17\"]'); const r = el.getBoundingClientRect(); return [r.left + 20, r.top + 6];");
    await drag(at, [at[0] + 160, at[1] + 10], 10);
    await type("v");
    const inked = () => js(`const v = [...window.openlogosShell.host.views.values()].find((x) => x.section.isConnected && x.section.dataset.number === '1');
      const el = v.stream.verseEl('John.3.17'); if (!el) return -1; const c = v.canvas; const cr = c.getBoundingClientRect(); const r = el.getBoundingClientRect();
      const y0 = Math.max(0, Math.round(r.top - cr.top - 20)), h = 60; const d = c.getContext('2d').getImageData(0, y0, c.width, h).data; let n = 0; for (let i = 3; i < d.length; i += 4) if (d[i]) n++; return n;`);
    const before = await inked();
    expect(p, before > 200, `stroke painted at John 3:17 (${before} px)`);
    // Far enough that John 3 is released, then back.
    await scrollStream(1, 400, 40);
    let s = await snap();
    expect(p, !s.reading[0].blocks.includes("John.3"), `John 3 released: ${s.reading[0].blocks}`);
    await scrollStream(1, -400, 80, "John.3");
    await navigate("John.3.16");
    s = await snap();
    expect(p, s.reading[0].blocks.includes("John.3"), "John 3 loaded again");
    const painted = await js("const h = CSS.highlights.get('ol-yellow'); return h ? [...h].map((r) => r.toString()) : []");
    expect(p, eq(painted, ["eternal life"]), `highlight repainted on the reloaded block: ${JSON.stringify(painted)}`);
    const after = await inked();
    expect(p, after > 200 && Math.abs(after - before) < before * 0.25, `stroke follows John 3:17 (${before} → ${after} px)`);
  },

  async "reading-dom-window-cap"(p) {
    await navigate("Ps.100");
    const r = await scrollStream(1, 120, 600);
    expect(p, r.chapters.length >= 30, `scrolled through ${r.chapters.length} chapters`);
    expect(p, r.maxBlocks <= 9 && r.maxNodes < 6000, `window ${r.maxBlocks} blocks, ${r.maxNodes} nodes`);
    expect(p, r.maxJump <= 2, `no jump (${r.maxJump})`);
    p.info = { chapters: r.chapters.length, maxBlocks: r.maxBlocks, maxNodes: r.maxNodes };
  },

  // MARK: B074: the user store, restore, drag and resize undo

  async "store-restores-everything"(p) {
    const query = `?theme=light&autotest=1&store=${uniqueStore("restore")}`;
    await load(query);
    // A highlight on BSB John 3:16 and a pen stroke on John 3:17 in workspace 1.
    await type("h");
    const pts = await phrasePoints(1, "eternal life");
    await drag(pts[0], pts[1]);
    await sleep(80);
    await type("p");
    const at = await js("const el = document.querySelector('#columns > .column[data-number=\"1\"] [data-segment=\"John.3.17\"]'); const r = el.getBoundingClientRect(); return [r.left + 20, r.top + 6];");
    await drag(at, [at[0] + 160, at[1] + 10], 10);
    const inkBefore = await inkNear(1, "John.3.17");
    // The note: bold and a passage link.
    await type("v");
    await click(...(await centre('#columns > .column[data-number="3"] .note-editor')));
    await js("const e = document.querySelector('#columns > .column[data-number=\"3\"] .note-editor'); const r = document.createRange(); r.selectNodeContents(e); r.collapse(false); const s = document.getSelection(); s.removeAllRanges(); s.addRange(r);");
    await type(" ");
    await chord("Meta", "b");
    await type("grace");
    await chord("Meta", "b");
    await click(...(await centre('#well button[data-command="note.link_passage"]')));
    await type("jn 3:17");
    await chord("Enter");
    await sleep(60);
    // A lexicon tab from a Greek word (its language is kept on the tab).
    await click(...(await wordPoint(2, "κόσμον")));
    await type("w");
    await click(...(await wordPoint(2, "ἠγάπησεν")));
    // Workspace 3: a new one at Acts 2, scrolled so a verse sits on the reading line.
    await js("window.openlogosShell.run({ command_id: 'workspace.new' })");
    await sleep(60);
    await navigate("Acts.2.4");
    await scrollStream(1, 37, 3);
    await sleep(300);
    // Back to workspace 1, tool Highlight.
    await js("window.openlogosShell.run({ command_id: 'workspace.switch', args: { workspace_id: 'w1' } })");
    await type("h");
    await sleep(60);
    const before = await snap();
    const third = before.workspaces.length === 3;
    expect(p, third, `three workspaces ${JSON.stringify(before.workspaces)}`);
    const ws3 = await js("const s = window.openlogosShell.host.session; return s.workspaces.find((w) => w.title === 'Workspace 3')");
    const anchor = ws3 && ws3.columns[0].tabs.find((t) => t.id === ws3.columns[0].shown_tab_id).scroll_anchor;
    expect(p, anchor && /^Acts\.2\./.test(anchor.segment_id), `workspace 3 anchored in Acts 2 (${JSON.stringify(anchor)})`);
    // Quit: reload the page (the save runs on hide, nothing waited for).
    await load(query);
    const after = await snap();
    const keep = (s) => ({ workspaces: s.workspaces, workspace: s.workspace, tool: s.tool, widths: s.widths, columns: s.columns, anchors: s.anchors, highlights: s.highlights, ink: s.ink.map((k) => [k.tab, k.strokes, k.segments]) });
    expect(p, eq(keep(after), keep(before)), `same state after relaunch\n before ${JSON.stringify(keep(before))}\n after  ${JSON.stringify(keep(after))}`);
    expect(p, after.tool === "highlight" && after.well.effective === "highlight", `tool ${after.tool}`);
    expect(p, after.stored.restore && after.stored.restore.found && after.stored.restore.report.issues.length === 0 && !after.toast, `restored quietly ${JSON.stringify(after.stored.restore)} toast ${after.toast}`);
    const painted = await js("const h = CSS.highlights.get('ol-yellow'); return h ? [...h].map((r) => r.toString()) : []");
    expect(p, eq(painted, ["eternal life"]), `highlight painted after relaunch ${JSON.stringify(painted)}`);
    const inkAfter = await inkNear(1, "John.3.17");
    expect(p, inkBefore > 200 && Math.abs(inkAfter - inkBefore) < inkBefore * 0.25, `ink on John 3:17 (${inkBefore} → ${inkAfter} px)`);
    const lexicon = await js("const l = document.querySelector('#columns .lexicon .word'); return l && [l.textContent, l.lang, l.nextElementSibling.textContent]");
    expect(p, lexicon && lexicon[0] === "ἠγάπησεν" && lexicon[1] === "grc" && lexicon[2].startsWith("Greek"), `lexicon tab restored in Greek ${JSON.stringify(lexicon)}`);
    const stored = JSON.stringify(after.stored.notes.find((n) => n.id === "fixture:note:1") || null);
    expect(p, stored.includes('{"bold":true,"text":"grace"}') && stored.includes('"link":"John.3.17"'), `note in the core after relaunch ${stored}`);
    await js("window.openlogosShell.run({ command_id: 'tab.focus', args: { workspace_id: 'w1', tab_id: 't4' } })");
    const note = (await snap()).notes["fixture:note:1"] || "";
    await js("window.openlogosShell.run({ command_id: 'column.focus', args: { column: 1 } })");
    expect(p, /<b>grace<\/b>/.test(note) && note.includes('data-reference="John.3.17"') && note.includes(">John 3:17</a>"), `note after relaunch ${note.slice(-200)}`);
    // Nothing to undo after a restore.
    const undo = await js("return window.openlogosShell.host.handle.view().undo_title");
    expect(p, undo == null, `nothing to undo (${undo})`);
    // The palette finds workspace 3's tab from workspace 1 and switches to it.
    await palette("@acts");
    const s3 = await snap();
    expect(p, s3.workspace === "Workspace 3", `palette switched to ${s3.workspace}`);
    await sleep(200);
    const s4 = await snap();
    const r = s4.reading[0];
    const line = r && await readingLine(1, anchor.segment_id);
    expect(p, r && line && line[1] != null && Math.abs(line[1] - line[0]) < 40 + 0 && s4.anchors[0] && s4.anchors[0].segment_id === anchor.segment_id, `Acts verse back on the reading line ${JSON.stringify({ line, top: r && r.top, anchor: s4.anchors[0] })}`);
  },

  async "store-damaged-file"(p) {
    const name = uniqueStore("damaged");
    const query = `?theme=light&autotest=1&store=${name}`;
    await load(query);
    await idbPut(`openlogos-${name}`, "current", '{"format":"openlogos-user-store","schema_version":"1.0","sess');
    await load(query);
    let s = await snap();
    expect(p, s.toast === "Your saved workspace could not be read; a copy was kept." && s.workspace === "Study", `notice "${s.toast}" on ${s.workspace}`);
    await sleep(150);
    let keys = await idbKeys(`openlogos-${name}`);
    expect(p, keys.filter((k) => k.startsWith("backup-")).length === 1 && !keys.includes("current"), `backup kept, current moved aside ${JSON.stringify(keys)}`);
    // The app works: a change saves a fresh store.
    await js("window.openlogosShell.run({ command_id: 'tool.pen' })");
    await jsAsync("const done = arguments[arguments.length - 1]; window.openlogosShell.saveNow().then(done)");
    keys = await idbKeys(`openlogos-${name}`);
    expect(p, keys.includes("current"), `a fresh store saved ${JSON.stringify(keys)}`);
    await load(query);
    s = await snap();
    expect(p, s.tool === "pen" && s.toast == null, `the fresh store restores quietly (${s.tool}, ${s.toast})`);
    // A newer major: said as such.
    await idbPut(`openlogos-${name}`, "current", '{"format":"openlogos-user-store","schema_version":"2.0"}');
    await load(query);
    s = await snap();
    expect(p, s.toast === "Your saved workspace was written by a newer version of OpenLogos; a copy was kept.", `newer version notice "${s.toast}"`);
    // A readable store with a bad note and a dangling session: what was set aside is named.
    await js("window.openlogosShell.host.handle.putNote({ id: 'n9', blocks: [{ kind: 'paragraph', spans: [{ text: 'ok' }] }] })");
    const bad = await js("const d = JSON.parse(window.openlogosShell.host.handle.exportStore()); d.session.active_workspace_id = 'nowhere'; d.notes.push({ id: 'n8', blocks: [{ kind: 'paragraph', spans: [{ text: 'x', link: 'not a passage' }] }] }); return JSON.stringify(d);");
    await js("window.openlogosShell.host.store = null"); // this page saves nothing as it goes
    await idbPut(`openlogos-${name}`, "current", bad);
    await load(query);
    s = await snap();
    expect(p, s.toast === "Your workspaces were reset; 1 note could not be restored; a copy was kept." && s.workspace === "Study" && s.stored.notes.some((n) => n.id === "n9"), `issues notice "${s.toast}", notes ${JSON.stringify(s.stored.notes.map((n) => n.id))}`);
    await sleep(150);
    keys = await idbKeys(`openlogos-${name}`);
    expect(p, keys.filter((k) => k.startsWith("backup-")).length === 3, `three backups ${JSON.stringify(keys)}`);
    expect(p, s.stored.changes !== s.stored.lastSaved, `the repaired store is due to save (${s.stored.changes} vs ${s.stored.lastSaved})`);
  },

  async "store-backups-keep-five"(p) {
    const name = uniqueStore("five");
    const query = `?theme=light&autotest=1&store=${name}`;
    for (let i = 0; i < 7; i++) {
      await load(query);
      await idbPut(`openlogos-${name}`, "current", `not json ${i}`);
      await sleep(5);
    }
    await load(query);
    await sleep(200);
    const keys = (await idbKeys(`openlogos-${name}`)).filter((k) => k.startsWith("backup-"));
    expect(p, keys.length === 5, `five backups kept ${JSON.stringify(keys)}`);
    const newest = await jsAsync(`const done = arguments[arguments.length - 1]; const r = indexedDB.open(arguments[0]); r.onsuccess = () => { const g = r.result.transaction('user-store').objectStore('user-store').get(arguments[1]); g.onsuccess = () => { done(g.result); r.result.close(); }; };`, `openlogos-${name}`, keys[keys.length - 1]);
    expect(p, newest === "not json 6", `the newest is the last damaged file (${newest})`);
  },

  async "store-note-round-trip"(p) {
    const query = `?theme=light&autotest=1&store=${uniqueStore("note")}`;
    await load(query);
    const fixture = JSON.parse(await readFile(join(repoRoot, "examples/user-store.json"), "utf8")).notes.find((n) => n.id === "fixture:note:1");
    await js("window.openlogosShell.host.handle.putNote(arguments[0]); return window.openlogosShell.saveNow()", fixture);
    await sleep(100);
    await load(query);
    // Loaded into the editor and saved back unchanged: the same blocks.
    const html = await js("const e = document.querySelector('#columns > .column[data-number=\"3\"] .note-editor'); e.dispatchEvent(new Event('input')); e.dispatchEvent(new Event('blur')); return e.innerHTML;");
    const back = await js("return window.openlogosShell.host.handle.note('fixture:note:1')");
    expect(p, eq(back, fixture), `round trip\n ${JSON.stringify(fixture)}\n ${JSON.stringify(back)}\n ${html}`);
    expect(p, html.startsWith("<h2>New birth <a") && html.includes("<i>works</i>") && html.includes("<b>from above</b>") && html.includes("<ul><li>Compare") && html.includes("<blockquote>") && html.endsWith("<p><br></p>"), `editor DOM ${html}`);
    // Typing goes to the core within the settle time; the dock and the tab stay in step.
    await click(...(await centre('#columns > .column[data-number="3"] .note-editor blockquote')));
    await type("!");
    await sleep(400);
    const typed = await js("return JSON.stringify(window.openlogosShell.host.handle.note('fixture:note:1').blocks[3])");
    expect(p, typed.includes("!"), `typed into the core ${typed}`);
  },

  async "tab-drag-and-resize-undo"(p) {
    const strip = (n) => js("return [...document.querySelectorAll('#columns > .column[data-number=\"' + arguments[0] + '\"] .tab')].map((t) => { const r = t.getBoundingClientRect(); return [t.firstChild.textContent, r.left, r.right, r.top + r.height / 2]; })", n);
    const c1 = await strip(1), c2 = await strip(2);
    // WLC Gen 1 (column 1's second tab) onto column 2's strip after its only tab.
    await drag([(c1[1][1] + c1[1][2]) / 2, c1[1][3]], [c2[0][2] + 20, c2[0][3]], 10);
    let s = await snap();
    expect(p, eq(s.dom.map((c) => c.tabs), [["BSB Jn 3"], ["SBLGNT Jn 3", "WLC Gen 1"], ["Note"]]), `dropped in column 2 ${JSON.stringify(s.dom.map((c) => c.tabs))}`);
    expect(p, s.log.some((l) => l.command_id === "tab.move" && l.handled), "tab.move dispatched");
    const title = await js("return window.openlogosShell.host.handle.view().undo_title");
    expect(p, title === "Move tab to position", `undo title "${title}"`);
    await chord("Meta", "z");
    s = await snap();
    expect(p, eq(s.dom.map((c) => c.tabs), [["BSB Jn 3", "WLC Gen 1"], ["SBLGNT Jn 3"], ["Note"]]), `⌘Z put it back ${JSON.stringify(s.dom.map((c) => c.tabs))}`);
    // Into a new column at the trailing edge.
    const again = await strip(1);
    const box = await js("const r = document.querySelector('#columns > .column[data-number=\"3\"]').getBoundingClientRect(); return [r.right - 20, r.top + 300];");
    // Held over the trailing zone: the band is drawn with its label.
    const from = [(again[1][1] + again[1][2]) / 2, again[1][3]];
    const steps = [];
    for (let i = 1; i <= 12; i++) steps.push({ type: "pointerMove", duration: 16, x: Math.round(from[0] + ((box[0] - from[0]) * i) / 12), y: Math.round(from[1] + ((box[1] - from[1]) * i) / 12) });
    await wd("POST", `${S}/actions`, { actions: [{ type: "pointer", id: "mouse", parameters: { pointerType: "mouse" }, actions: [{ type: "pointerMove", duration: 0, x: Math.round(from[0]), y: Math.round(from[1]) }, { type: "pointerDown", button: 0 }, ...steps] }] });
    await sleep(60);
    const band = await js("const b = document.getElementById('drop-slot'); if (!b || b.hidden) return null; const r = b.getBoundingClientRect(); return [b.className, b.textContent, Math.round(r.width), getComputedStyle(b).display];");
    expect(p, band && band[0] === "zone" && band[1] === "New column" && band[2] === 56 && band[3] !== "none", `the new-column band shows its label during the drag ${JSON.stringify(band)}`);
    await wd("POST", `${S}/actions`, { actions: [{ type: "pointer", id: "mouse", parameters: { pointerType: "mouse" }, actions: [{ type: "pointerUp", button: 0 }] }] });
    await wd("DELETE", `${S}/actions`);
    await sleep(60);
    const gone = await js("const b = document.getElementById('drop-slot'); return !b || b.hidden;");
    expect(p, gone, "the band goes when the drop ends");
    s = await snap();
    expect(p, eq(s.dom.map((c) => c.tabs), [["BSB Jn 3"], ["SBLGNT Jn 3"], ["Note"], ["WLC Gen 1"]]), `new column ${JSON.stringify(s.dom.map((c) => c.tabs))}`);
    await chord("Meta", "z");
    s = await snap();
    expect(p, eq(s.dom.map((c) => c.tabs), [["BSB Jn 3", "WLC Gen 1"], ["SBLGNT Jn 3"], ["Note"]]), `⌘Z removed the new column ${JSON.stringify(s.dom.map((c) => c.tabs))}`);
    // A drop back into its own slot sends nothing.
    const moves = s.log.filter((l) => l.command_id === "tab.move").length;
    const same = await strip(1);
    await drag([(same[1][1] + same[1][2]) / 2, same[1][3]], [same[1][2] - 3, same[1][3]], 6);
    s = await snap();
    expect(p, s.log.filter((l) => l.command_id === "tab.move").length === moves && !s.toast, `no-op drop sent nothing (${s.toast})`);
    // A plain click still shows the tab.
    await click((same[1][1] + same[1][2]) / 2, same[1][3]);
    s = await snap();
    expect(p, s.dom[0].shown === "WLC Gen 1", `click shows the tab (${s.dom[0].shown})`);
    // Resize: one column.resize at the end of the drag; ⌘Z restores the widths.
    const widths = s.widths;
    const rule = await centre(".rule");
    await drag(rule, [rule[0] + 150, rule[1]], 8);
    s = await snap();
    expect(p, s.widths[0] > widths[0] + 0.05 && s.log.filter((l) => l.command_id === "column.resize").length === 1, `resized ${JSON.stringify(widths)} → ${JSON.stringify(s.widths)}`);
    await chord("Meta", "z");
    s = await snap();
    expect(p, eq(s.widths, widths), `⌘Z restored the widths ${JSON.stringify(s.widths)}`);
    const grow = await js("return [...document.querySelectorAll('#columns > .column')].map((c) => Number(c.style.flexGrow).toFixed(4))");
    expect(p, eq(grow, widths.map((w) => w.toFixed(4))), `and on screen ${grow}`);
  },

  async "store-saves-after-debounce-and-on-hide"(p) {
    const name = uniqueStore("save");
    const query = `?theme=light&autotest=1&store=${name}`;
    await load(query);
    // A change is written 1 s after it settles (IndexedDB), with nothing kept aside.
    await js("window.openlogosShell.run({ command_id: 'tool.pen' })");
    await sleep(1500);
    const pendingKey = `openlogos-${name}:current:pending`;
    const quiet = await js("return [localStorage.getItem(arguments[0]), window.openlogosShell.host.lastSaved === window.openlogosShell.host.handle.changeCount()]", pendingKey);
    expect(p, quiet[0] == null && quiet[1], `saved after the debounce, nothing pending ${JSON.stringify(quiet)}`);
    await load(query);
    let s = await snap();
    expect(p, s.tool === "pen" && s.stored.restore.found && !s.stored.restore.pending, `restored from IndexedDB (${s.tool}, ${JSON.stringify(s.stored.restore)})`);
    // A change just before the page goes: the copy kept on hide restores, then is written.
    await js("window.openlogosShell.run({ command_id: 'tool.highlight' })");
    await load(query);
    s = await snap();
    expect(p, s.tool === "highlight" && s.stored.restore.pending, `restored from the copy kept on hide (${s.tool}, ${JSON.stringify(s.stored.restore)})`);
    await sleep(200);
    const left = await js("return localStorage.getItem(arguments[0])", pendingKey);
    const current = JSON.parse(await jsAsync(`const done = arguments[arguments.length - 1]; const r = indexedDB.open(arguments[0]); r.onsuccess = () => { const g = r.result.transaction('user-store').objectStore('user-store').get('current'); g.onsuccess = () => { done(g.result); r.result.close(); }; };`, `openlogos-${name}`));
    expect(p, left == null && current.session.tool === "highlight", `the copy went into IndexedDB (${left == null}, ${current.session.tool})`);
  },

  async "only-registered-commands"(p) {
    // Drive a bit of everything, then check every dispatch was a registry id.
    await chord("Alt", "2"); await type("h"); await chord("Alt", "n"); await chord("Meta", "z");
    const r = await js("const h = window.openlogosShell.host; const ids = new Set(h.registry.map((s) => s.id)); return h.log.filter((l) => !ids.has(l.command_id)).map((l) => l.command_id);");
    expect(p, r.length === 0, `unregistered dispatches ${r}`);
  },
};

// MARK: B074 speed: restore over launches, then saves and workspace commands at the acceptance size

async function storeBenchRun() {
  const query = `?theme=light&autotest=1&session=bench&store=${uniqueStore("accept")}`;
  await load(query);
  const bytes = await js("return window.openlogosShell.buildAcceptance()");
  await jsAsync("const done = arguments[arguments.length - 1]; window.openlogosShell.saveNow().then(done)");
  const rows = [];
  const samples_ = { read: [], imported: [], rendered: [], painted: [] };
  for (let i = 0; i < launches; i++) {
    await load(query);
    await jsAsync("const done = arguments[arguments.length - 1]; requestAnimationFrame(() => requestAnimationFrame(() => done()))");
    const r = await js("return window.openlogosShell.host.restore");
    if (!r || !r.found || r.refused || (r.report && r.report.issues.length)) { failures += 1; console.log(JSON.stringify({ bench: "store", error: "restore", restore: r })); continue; }
    samples_.read.push(r.read_ms);
    samples_.imported.push(r.read_ms + r.import_ms);
    samples_.rendered.push(r.rendered_ms);
    samples_.painted.push(r.first_paint_ms);
  }
  const st = (v) => { const a = [...v].sort((x, y) => x - y); const q = (p) => a[Math.min(a.length - 1, Math.floor(p * a.length))]; return { n: a.length, p50: +q(0.5).toFixed(3), p95: +q(0.95).toFixed(3), max: +a[a.length - 1].toFixed(3) }; };
  const report = await js("return window.openlogosShell.host.restore.report");
  rows.push({ name: `B074 restore: IndexedDB read (${bytes} bytes)`, work_ms: st(samples_.read) });
  rows.push({ name: "B074 restore: read + importStore", work_ms: st(samples_.imported) });
  rows.push({ name: "B074 restore: read → active workspace rendered (laid out)", work_ms: st(samples_.rendered) });
  rows.push({ name: "B074 restore: read → first painted text (next frame)", work_ms: st(samples_.painted), report });
  if (samples_.painted.length < Math.min(20, launches)) failures += 1;
  const more = await jsAsync("const done = arguments[arguments.length - 1]; window.openlogosShell.storeBench({ samples: arguments[0] }).then(done, (e) => done({ error: String(e) }))", samples);
  if (more.error) { failures += 1; console.log(JSON.stringify({ bench: "store", error: more.error })); return { rows }; }
  rows.push(...more.rows);
  for (const row of rows) console.log(JSON.stringify({ bench: "store", ...row }));
  summary.push(`B074 store bench: ${launches} launches, ${samples} samples per row`);
  return { bytes, workspaces: more.workspaces, rows };
}

// MARK: screenshots

// The ten the evidence keeps (≤ 20 PNGs per task across both hosts); the rest go to
// --extra-shots when given, and are skipped otherwise.
const EVIDENCE = new Set(["web-light-1440", "web-sepia-1440", "web-dark-1024", "web-palette-passage", "web-palette-commands-expanded-dark", "web-palette-tabs", "web-palette-text", "web-tool-palette", "web-ink", "web-placeholder"]);

async function shoot(name) {
  const dir = EVIDENCE.has(name) ? shotsDir : extraDir;
  if (!dir) return;
  await mkdir(dir, { recursive: true });
  const png = await wd("GET", `${S}/screenshot`);
  const file = join(dir, `${name}.png`);
  await writeFile(file, Buffer.from(png, "base64"));
  console.log(`screenshot ${file}`);
}

async function setWindow(w, h) {
  await wd("POST", `${S}/window/rect`, { width: w, height: h });
  // Headless Chrome sizes the window, not the viewport: correct by the difference once.
  const inner = await js("return [innerWidth, innerHeight]");
  if (inner[0] !== w || inner[1] !== h) await wd("POST", `${S}/window/rect`, { width: w + (w - inner[0]), height: h + (h - inner[1]) });
}

async function screenshots() {
  await mkdir(shotsDir, { recursive: true });
  // The theme and width shots carry one highlight and no ink; ink has its own shot.
  const prepare = async () => {
    await type("h");
    const pts = await phrasePoints(1, "eternal life");
    if (pts) await drag(pts[0], pts[1]);
    await type("v");
    await chord("Escape");
  };
  for (const theme of ["light", "sepia", "dark"]) {
    await setWindow(1440, 900);
    await load(`?theme=${theme}`);
    await prepare();
    await shoot(`web-${theme}-1440`);
  }
  await setWindow(1024, 768);
  for (const theme of ["light", "dark"]) {
    await load(`?theme=${theme}`);
    await prepare();
    await shoot(`web-${theme}-1024`);
  }
  await setWindow(1440, 900);
  for (const [name, text] of [["passage", "jn 3:16"], ["commands", "tab"], ["tabs", "@"], ["text", "#loved"]]) {
    await load("?theme=light");
    await chord("Meta", "k");
    await type(text);
    await shoot(`web-palette-${name}`);
  }
  await load("?theme=dark");
  await chord("Meta", "k");
  await type(">");
  await chord("Tab");
  await shoot("web-palette-commands-expanded-dark");
  await load("?theme=light");
  await type("p");
  const box = await js("const r = document.querySelector('#columns > .column[data-number=\"1\"] .measure .text').getBoundingClientRect(); return [r.left, r.top, r.width, r.height];");
  await drag([box[0] + 10, box[1] + box[3] + 14], [box[0] + box[2] * 0.6, box[1] + box[3] + 8], 14);
  await type("m");
  const words = await phrasePoints(1, "For God did not send");
  if (words) await drag(words[0], words[1], 10);
  await shoot("web-ink");
  await load("?theme=light");
  await type("m");
  await shoot("web-tool-palette");
  await click(...(await centre("#workspace-switcher")));
  await shoot("web-workspace-menu");
  await chord("Escape");
  await click(...(await centre("#text-menu")));
  await shoot("web-text-menu");
  await chord("Escape");
  await chord("Meta", "k");
  await type("rom 8");
  await chord("Enter");
  await shoot("web-placeholder");
}

/** B076: the states the owner reviews, the same on the Mac (docs/evidence/B076). */
async function wellScreenshots() {
  await mkdir(wellShotsDir, { recursive: true });
  const shot = async (name) => {
    const file = join(wellShotsDir, `${name}.png`);
    await writeFile(file, Buffer.from(await wd("GET", `${S}/screenshot`), "base64"));
    console.log(`screenshot ${file}`);
  };
  const word = async (column, w) => { const pts = await phrasePoints(column, w); return [(pts[0][0] + pts[1][0]) / 2, (pts[0][1] + pts[1][1]) / 2]; };
  await setWindow(1440, 900);
  await load("?theme=light&autotest=1");
  await click(...(await centre('#columns > .column[data-number="1"] .identity')));
  await shot("web-well-translation");
  await chord("Alt", "2");
  await type("w");
  await click(...(await word(2, "ἠγάπησεν")));
  await shot("web-well-original-lexicon"); // focus stays on the Greek column (tab.open_lexicon)
  await load("?theme=light&autotest=1");
  await click(...(await centre('#columns > .column[data-number="3"] .note-editor')));
  await js("const e = document.querySelector('#columns > .column[data-number=\"3\"] .note-editor'); const r = document.createRange(); r.selectNodeContents(e); r.collapse(false); const s = document.getSelection(); s.removeAllRanges(); s.addRange(r);");
  await chord("Enter");
  await type("Love that gives: ");
  await chord("Meta", "b");
  await type("grace");
  await chord("Meta", "b");
  await type(", see ");
  await click(...(await centre('#well button[data-command="note.link_passage"]')));
  await type("jn 3:16");
  await chord("Enter");
  await shot("web-well-notes-editing");
  for (const theme of ["light", "dark"]) {
    await load(`?theme=${theme}&autotest=1`);
    const pts = await phrasePoints(1, "eternal life");
    await drag(pts[0], pts[1]);
    await sleep(80);
    await shot(`web-well-selection${theme === "dark" ? "-dark" : ""}`);
  }
  await load("?theme=light&autotest=1");
  await type("p");
  await chord("Alt", "3");
  await shot("web-well-fallback-pen-in-notes");
}

/** B077: the menu states the owner reviews, the same on the Mac (docs/evidence/B077). */
async function menuScreenshots() {
  await mkdir(menuShotsDir, { recursive: true });
  const shot = async (name) => {
    const file = join(menuShotsDir, `${name}.png`);
    await writeFile(file, Buffer.from(await wd("GET", `${S}/screenshot`), "base64"));
    console.log(`screenshot ${file}`);
  };
  const hover = async (label, level = 0) => {
    const at = await js("const m = document.querySelectorAll('.ctx-menu')[arguments[1]]; const r = [...m.querySelectorAll('.ctx-row')].find((x) => x.querySelector('.ctx-label > span').textContent === arguments[0]); const b = r.getBoundingClientRect(); return [b.left + b.width / 2, b.top + b.height / 2];", label, level);
    await wd("POST", `${S}/actions`, { actions: [{ type: "pointer", id: "mouse", parameters: { pointerType: "mouse" }, actions: [{ type: "pointerMove", duration: 60, x: Math.round(at[0]), y: Math.round(at[1]) }] }] });
    await wd("DELETE", `${S}/actions`);
    await sleep(60);
  };
  await setWindow(1440, 900);
  await load("?theme=light&autotest=1");
  await rightClick(...(await wordPoint(2, "ἠγάπησεν")));
  await hover("Word study");
  await shot("web-menu-word-greek");
  await load("?theme=light&autotest=1");
  await rightClick(...(await wordPoint(1, "loved")));
  await hover("Word study");
  await shot("web-menu-word-bsb-disabled");
  await hover("Highlight");
  await sleep(60);
  await hover("Green", 1);
  await shot("web-menu-highlight-submenu");
  await load("?theme=light&autotest=1");
  const pts = await phrasePoints(1, "eternal life");
  await drag(pts[0], pts[1]);
  await rightClick((pts[0][0] + pts[1][0]) / 2, pts[0][1]);
  await shot("web-menu-selection");
  await load("?theme=light&autotest=1");
  await rightClick(...(await centre('#columns > .column[data-number="2"] .tab')));
  await hover("Move to");
  await shot("web-menu-tab");
  await load("?theme=light&autotest=1");
  await drag(pts[0], pts[1]);
  await click(...(await centre('#well button[data-command="selection.note"]')));
  await rightClick(...(await centre('#columns > .column[data-number="3"] a.passage-link')));
  await hover("Compare in");
  await shot("web-menu-note-link");
  await load("?theme=light&autotest=1");
  await rightClick(...(await wordPoint(1, "world")));
  await chooseRow("Find in this chapter");
  await chord("Meta", "g");
  await shot("web-menu-find-in-chapter");
  await load("?theme=dark&autotest=1");
  await rightClick(...(await wordPoint(1, "Son")));
  await hover("Copy");
  await shot("web-menu-dark");
}

/** B078: continuous reading, the same states as the Mac (docs/evidence/B078). */
/** B074: the same state before a quit and after the relaunch, the damaged-store notice, a tab
 *  drag in flight and undone, and the palette's rows from every workspace. */
async function storeScreenshots() {
  await mkdir(storeShotsDir, { recursive: true });
  const shot = async (name) => {
    const file = join(storeShotsDir, `${name}.png`);
    await writeFile(file, Buffer.from(await wd("GET", `${S}/screenshot`), "base64"));
    console.log(`screenshot ${file}`);
  };
  await setWindow(1440, 900);
  const query = `?theme=light&autotest=1&store=${uniqueStore("shots")}`;
  await load(query);
  await type("h");
  const pts = await phrasePoints(1, "eternal life");
  await drag(pts[0], pts[1]);
  await sleep(80);
  await type("p");
  // Under "condemn the world", John 3:17's second line.
  const at = await js("const el = document.querySelector('#columns > .column[data-number=\"1\"] [data-segment=\"John.3.17\"]'); const r = [...el.getClientRects()][1]; return [r.left + 250, r.bottom - 2];");
  await drag(at, [at[0] + 210, at[1] + 4], 12);
  await type("v");
  await click(...(await centre('#columns > .column[data-number="3"] .note-editor')));
  await js("const e = document.querySelector('#columns > .column[data-number=\"3\"] .note-editor'); const r = document.createRange(); r.selectNodeContents(e); r.collapse(false); const s = document.getSelection(); s.removeAllRanges(); s.addRange(r);");
  await chord("Enter");
  await chord("Meta", "b");
  await type("Grace");
  await chord("Meta", "b");
  await type(" in ");
  await click(...(await centre('#well button[data-command="note.link_passage"]')));
  await type("jn 3:17");
  await chord("Enter");
  await js("window.openlogosShell.run({ command_id: 'column.focus', args: { column: 1 } }); window.openlogosShell.run({ command_id: 'workspace.new' })");
  await navigate("Acts.2.4");
  await js("window.openlogosShell.run({ command_id: 'workspace.switch', args: { workspace_id: 'w1' } }); window.openlogosShell.run({ command_id: 'column.focus', args: { column: 1 } }); document.activeElement.blur(); document.getSelection().removeAllRanges()");
  await type("h");
  await js("window.openlogosShell.host.lastToast = null; document.getElementById('toast').classList.remove('show')");
  await sleep(300);
  await shot("web-store-before-quit");
  await load(query);
  await sleep(300);
  await shot("web-store-after-relaunch");
  // The palette finds tabs of every workspace.
  await chord("Meta", "k");
  await type("@a");
  await sleep(120);
  await shot("web-store-palette-workspaces");
  await chord("Escape");
  await js("document.getElementById('palette-field').value = ''");
  // A tab drag in flight (the drop slot drawn), then dropped and undone.
  const tabs = await js("return [...document.querySelectorAll('#columns > .column .tabstrip')].map((s) => [...s.querySelectorAll('.tab')].map((t) => { const r = t.getBoundingClientRect(); return [r.left, r.right, r.top + r.height / 2]; }))");
  const from = [(tabs[0][1][0] + tabs[0][1][1]) / 2, tabs[0][1][2]];
  const to = [tabs[1][0][1] + 24, tabs[1][0][2]];
  const moves = [];
  for (let i = 1; i <= 10; i++) moves.push({ type: "pointerMove", duration: 16, x: Math.round(from[0] + ((to[0] - from[0]) * i) / 10), y: Math.round(from[1] + ((to[1] - from[1]) * i) / 10) });
  await wd("POST", `${S}/actions`, { actions: [{ type: "pointer", id: "mouse", parameters: { pointerType: "mouse" }, actions: [{ type: "pointerMove", duration: 0, x: Math.round(from[0]), y: Math.round(from[1]) }, { type: "pointerDown", button: 0 }, ...moves] }] });
  await sleep(80);
  await shot("web-store-drag-in-flight");
  await wd("POST", `${S}/actions`, { actions: [{ type: "pointer", id: "mouse", parameters: { pointerType: "mouse" }, actions: [{ type: "pointerUp", button: 0 }] }] });
  await wd("DELETE", `${S}/actions`);
  await sleep(80);
  await chord("Meta", "z");
  await sleep(80);
  await js("window.openlogosShell.host.lastToast = null");
  await shot("web-store-drag-undone");
  // A damaged store: the notice on a fresh session.
  const name = uniqueStore("shots-damaged");
  await load(`?theme=light&autotest=1&store=${name}`);
  await idbPut(`openlogos-${name}`, "current", '{"format":"openlogos-user-store","schema_version":"1.0","sess');
  await load(`?theme=light&autotest=1&store=${name}`);
  await sleep(250);
  await shot("web-store-damaged-notice");
}

async function readScreenshots() {
  await mkdir(readShotsDir, { recursive: true });
  const shot = async (name, element = null) => {
    const file = join(readShotsDir, `${name}.png`);
    const png = element
      ? await wd("GET", `${S}/element/${Object.values(await wd("POST", `${S}/element`, { using: "css selector", value: element }))[0]}/screenshot`)
      : await wd("GET", `${S}/screenshot`);
    await writeFile(file, Buffer.from(png, "base64"));
    console.log(`screenshot ${file}`);
  };
  await setWindow(1440, 900);
  for (const theme of ["light", "dark"]) {
    await load(`?theme=${theme}&autotest=1`);
    await navigate("Ps.23"); // somewhere else first, so the jump is a jump
    await palette("John 3:16");
    await sleep(140); // mid-flash
    await shot(theme === "dark" ? "web-read-jump-dark" : "web-read-jump-john-3-16");
  }
  await load("?theme=light&autotest=1");
  await navigate("John.21.20");
  await scrollStream(1, 60, 400, "Acts.1");
  await scrollStream(1, -60, 4);
  await shot("web-read-john-acts-boundary");
  await load("?theme=light&autotest=1");
  await navigate("John.3.16");
  await scrollStream(1, 50, 30);
  await shot("web-read-linked-bsb-sblgnt");
  await load("?theme=light&autotest=1");
  await navigate("Ps.3");
  await shot("web-read-psalm-3-poetry");
  await load("?theme=light&autotest=1");
  await js("const r = window.openlogosShell.run; r({ command_id: 'tab.focus', args: { workspace_id: 'w1', tab_id: 't2' } }); r({ command_id: 'tab.link', args: { group: '1' } }); r({ command_id: 'tab.move_to_column', args: { column: 3 } }); r({ command_id: 'column.focus', args: { column: 1 } });");
  await palette("Mal 4:5");
  await sleep(1400);
  await shot("web-read-wlc-malachi-scheme");
  await load("?theme=light&autotest=1");
  await palette("John 22");
  await shot("web-read-invalid-john-22");
  await load("?theme=light&autotest=1");
  await navigate("John.4");
  await shot("web-read-placeholder-notice", '#columns > .column[data-number="1"] .scroller');
}

// MARK: run

let failures = 0;
let sessionId = null;
const summary = [];
try {
  const session = await wd("POST", "/session", { capabilities });
  sessionId = session.sessionId;
  S = `/session/${sessionId}`;
  await wd("POST", `${S}/timeouts`, { script: 900000 }); // the speed runs take minutes
  await setWindow(width, height);
  await load();
  const agent = await js("return [navigator.userAgent, window.openlogosShell.core.apiVersion(), innerWidth, innerHeight]");
  console.log(JSON.stringify({ browser: agent[0], core: agent[1], viewport: [agent[2], agent[3]], headed }));
  const names = only.length ? only : Object.keys(CASES);
  for (const name of names) {
    const problems = [];
    try {
      await load("?theme=light&autotest=1");
      await CASES[name](problems);
    } catch (error) {
      problems.push(`threw: ${String(error).slice(0, 300)}`);
    }
    // The page's Content-Security-Policy (index.html) must never have to refuse anything.
    try {
      for (const v of await js("return window.openlogosCspViolations || []")) problems.push(`content security policy refused: ${v}`);
    } catch (error) {
      problems.push(`could not read the policy violations: ${String(error).slice(0, 200)}`);
    }
    const passed = problems.length === 0;
    if (!passed) failures += 1;
    console.log(JSON.stringify({ name, passed, problems }));
  }
  summary.push(`Web shell: ${names.length - failures}/${names.length} cases passed`);

  if (benchOn) {
    const result = {};
    for (const [label, query] of [["seed", "?theme=light&autotest=1"], ["bench", "?theme=light&autotest=1&session=bench"]]) {
      await load(query);
      result[label] = await jsAsync("const done = arguments[arguments.length - 1]; window.openlogosShell.bench({ samples: arguments[0] }).then(done, (e) => done({ error: String(e) }));", samples);
      if (result[label].error) { failures += 1; console.log(JSON.stringify({ bench: label, error: result[label].error })); continue; }
      for (const row of result[label].rows) {
        console.log(JSON.stringify({ bench: label, ...row }));
        if (row.work_ms.n < samples) failures += 1;
      }
      // Trusted keystrokes typed through the driver, every prefix of the queries.
      await js("window.openlogosShell.host.recordKeys = true; window.openlogosShell.keyReport(true); document.getElementById('palette-field').focus()");
      const queries = ["jn 3:16", "move tab to column 3", "bench 7", "@rom 8", "theme dark", "new col", "#loved", "gen 1:1"];
      let typed = 0;
      while (typed < samples) {
        for (const q of queries) {
          await chord("Meta", "a");
          await type(q);
          typed += q.length;
        }
      }
      await sleep(100);
      const keys = await js("return window.openlogosShell.keyReport(true)");
      result[label].trusted_keystrokes = keys;
      console.log(JSON.stringify({ bench: label, name: "trusted keystroke → palette rendered", ...keys }));
      if (keys.n < samples) failures += 1;
      // B077: trusted right-clicks on a word → the menu drawn (the selection cleared between, so
      // each is a word menu: the core's menu, the word selected, the element laid out).
      await load(query);
      await js("window.openlogosShell.host.recordMenu = true; window.openlogosShell.menuReport(true); document.activeElement.blur()");
      const at = await wordPoint(1, "loved");
      for (let i = 0; i < samples; i++) {
        await js("document.getSelection().removeAllRanges()");
        await rightClick(...at);
        await chord("Escape");
      }
      await sleep(100);
      const menu = await js("window.openlogosShell.host.recordMenu = false; return window.openlogosShell.menuReport(true)");
      result[label].trusted_right_click = menu;
      console.log(JSON.stringify({ bench: label, name: "B077 trusted right-click → context menu drawn", ...menu }));
      if (menu.n < samples) failures += 1;
    }
    result.store = await storeBenchRun();
    if (benchJson) { await mkdir(dirname(benchJson), { recursive: true }); await writeFile(benchJson, JSON.stringify({ headed, ...result }, null, 2)); console.log(`bench json ${benchJson}`); }
    summary.push(`bench: ${headed ? "headed" : "headless"} Chrome, ${samples} samples per interaction`);
  }
  if (!benchOn && storeBenchOn) {
    const out = await storeBenchRun();
    if (benchJson) { await mkdir(dirname(benchJson), { recursive: true }); await writeFile(benchJson, JSON.stringify({ headed, store: out }, null, 2)); console.log(`bench json ${benchJson}`); }
  }
  if (shotsDir) await screenshots();
  if (wellShotsDir) await wellScreenshots();
  if (menuShotsDir) await menuScreenshots();
  if (readShotsDir) await readScreenshots();
  if (storeShotsDir) await storeScreenshots();
} catch (error) {
  console.error(String(error));
  failures = failures || 1;
} finally {
  if (sessionId) { try { await wd("DELETE", `/session/${sessionId}`); } catch {} }
  if (driver) driver.kill();
  server.close();
}
console.log(`Summary: ${summary.join("; ")}${failures ? ` — ${failures} failure(s)` : ""}`);
process.exit(failures ? 1 : 0);
