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
const headed = args.includes("--headed");
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

const K = { Meta: "", Alt: "", Shift: "", Control: "", Enter: "", Escape: "", Tab: "", ArrowDown: "", ArrowUp: "" };
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
const groupsOf = (well) => well.groups.map((g) => [g.kind, g.items]);

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
    expect(p, r.commands === 70, `${r.commands} commands`);
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
    const box = await js("const r = document.querySelector('#columns > .column[data-number=\"1\"] .measure').getBoundingClientRect(); return [r.left, r.top, r.width];");
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
    await chord("Meta", "k");
    await type("rom 8");
    await chord("Enter");
    const s = await snap();
    expect(p, s.dom[0].placeholder && s.dom[1].placeholder, `Rom 8 shows the placeholder in the group: ${JSON.stringify(s.dom.map((c) => c.placeholder))}`);
    const text = await js("return document.querySelector('.placeholder h2').textContent");
    expect(p, text === "Berean Standard Bible · Rom 8", `placeholder "${text}"`);
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
    // Large text so the passage scrolls; scroll column 1, let it settle, check the core has it.
    await setWindow(1024, 420);
    for (let i = 0; i < 4; i++) await js("window.openlogosShell.run({ command_id: 'view.text_size_up' })");
    const room = await js("const s = document.querySelector('#columns > .column .scroller'); s.scrollTop = 200; return [s.scrollHeight, s.clientHeight, s.scrollTop];");
    expect(p, room[2] > 100, `column 1 scrolls (${room})`);
    await sleep(400);
    let s = await snap();
    const a = s.anchors[0];
    expect(p, a && /^John\.3\.1[67]$/.test(a.segment_id) && a.scalar >= 0, `scroll anchor ${JSON.stringify(a)}`);
    expect(p, s.log.some((l) => l.command_id === "tab.record_scroll" && l.handled), "tab.record_scroll dispatched");
    // Away and back: the column is rebuilt from the session and lands on the same anchor.
    const top = await js("return document.querySelector('#columns > .column .scroller').scrollTop");
    await chord("Alt", "]");
    await chord("Alt", "[");
    await sleep(300);
    const again = await js("return document.querySelector('#columns > .column .scroller').scrollTop");
    expect(p, Math.abs(again - top) < 40, `scroll restored from the anchor (${top} → ${again})`);
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
    expect(p, note.includes("<h3>Heading</h3>") && /<ul><li>item/.test(note), `heading and list: ${note.slice(-160)}`);
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

  async "only-registered-commands"(p) {
    // Drive a bit of everything, then check every dispatch was a registry id.
    await chord("Alt", "2"); await type("h"); await chord("Alt", "n"); await chord("Meta", "z");
    const r = await js("const h = window.openlogosShell.host; const ids = new Set(h.registry.map((s) => s.id)); return h.log.filter((l) => !ids.has(l.command_id)).map((l) => l.command_id);");
    expect(p, r.length === 0, `unregistered dispatches ${r}`);
  },
};

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
  await chord("Alt", "2");
  await shot("web-well-original-lexicon");
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
    }
    if (benchJson) { await mkdir(dirname(benchJson), { recursive: true }); await writeFile(benchJson, JSON.stringify({ headed, ...result }, null, 2)); console.log(`bench json ${benchJson}`); }
    summary.push(`bench: ${headed ? "headed" : "headless"} Chrome, ${samples} samples per interaction`);
  }
  if (shotsDir) await screenshots();
  if (wellShotsDir) await wellScreenshots();
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
