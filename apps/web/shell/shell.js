// The workspace shell on the web (B072, specs/22): the palette field, numbered columns of tabs
// and the floating tool palette, over one ShellHandle from the Rust core (B070).
//
// Every button, menu item, key and palette row dispatches a registered command through
// `run()` → `ShellHandle.apply`; when the core answers `handled: false` the host performs it
// (`perform()`: focus mode, the tool well's visibility, selection actions, export, focusing
// the palette), and the view is re-rendered from `handle.session()`: text size, line spacing
// and theme from `session.reading`, brush width and swatches from `session.ink`, column widths
// and scroll anchors from the workspace. Keys come from the registry (keys.js); nothing is
// bound that it does not list.
//
// Host view state, outside the session (docs/evidence/B072-web.md): the tool well's position
// and collapse, a column-rule drag until it ends in `column.resize`, and highlights and ink
// strokes (in memory until the annotation store, B074).
//
// Rendering is per column: a column's DOM is rebuilt only when its tabs, shown tab or passage
// change; focus, tool and theme changes touch classes and attributes only, so they cost a
// style pass, and a drag in the text is never cut short by a re-render. Hidden tabs and other
// workspaces keep no DOM (specs/22 §7: they release their rendered text).
import init, * as core from "../core-host/pkg/openlogos_ffi.js";
import { renderParagraph, selectionAnchors, anchorRange } from "../reader/surface.js";
import { BRUSHES, HIGHLIGHTER_WIDTH_FACTOR, TouchArbiter, compositeAt, strokeOutline, distanceToSegment } from "../ink/layer.js";
import { fixtureTextSource } from "./text-source.js";
import { seedSession, benchSession } from "./seed.js";
import { bindingsFrom, match } from "./keys.js";
import { humanPassage, shortcutGlyphs, linkWord } from "./format.js";

const params = new URLSearchParams(location.search);
const $ = (id) => document.getElementById(id);
const root = document.documentElement;

// Mirrors of design/tokens/shell.json that the script needs as numbers (the test compares them).
// The core's text_size_step 0 is the default size; a step indexes `sizes` from there.
export const TOKENS = {
  sizes: [15, 17, 19, 21, 24, 28, 32], defaultSize: 19,
  spacings: [1.35, 1.5, 1.7], defaultSpacing: 1.5,
  swatches: { yellow: ["#F5D90A", 0.35], green: ["#4CC38A", 0.30], blue: ["#5B9BFF", 0.30], pink: ["#F25C9A", 0.28] },
  themes: ["light", "sepia", "dark"],
};
const SWATCH_ORDER = ["yellow", "green", "blue", "pink"];

/** The swatch name of a `#RRGGBB` (the core stores colours), or null for another colour. */
function swatchName(hex) {
  const h = (hex || "").toUpperCase();
  return SWATCH_ORDER.find((k) => TOKENS.swatches[k][0] === h) || null;
}
/** Colour and painted opacity of a stored swatch; an unknown colour paints at 30 %. */
const swatchPaint = (hex) => { const n = swatchName(hex); return n ? TOKENS.swatches[n] : [hex, 0.3]; };

// Inline SVG for the tool well (20 × 20, stroked with currentColor).
const ICONS = {
  select: '<path d="M5.5 3.5v12l3.2-3 2.3 5 2-1-2.3-4.8h4.3z" fill="currentColor" stroke="none"/>',
  highlight: '<path d="M7 13.5l-2.5 3h5l1-1.2M7 13.5l1.8-4.2 6-6 2.4 2.4-6 6z"/><path d="M3 18h14" opacity=".5"/>',
  pen: '<path d="M4 16l.9-3.6L13 4.3l2.7 2.7-8.1 8.1z"/><path d="M11.5 5.8l2.7 2.7"/>',
  highlighter_pen: '<path d="M10.5 9.5l5.5-5.5 1 1-5.5 5.5"/><path d="M4 16.5c.3-2.6 1.4-4.6 3.6-4.6 1.4 0 2.2 1 2.1 2.2-.2 1.9-2.5 2.4-5.7 2.4z" fill="currentColor" fill-opacity=".35"/>',
  eraser: '<path d="M7.5 16.5h9"/><path d="M3.8 12.2l7.4-7.4 5 5-6.6 6.7H7.6z"/><path d="M7.4 8.6l5 5"/>',
  collect: '<path d="M10 3v8.5M6.8 8.5l3.2 3.2 3.2-3.2"/><path d="M3.5 12v4.5h13V12"/>',
};
const TOOLS = [
  ["select", "Select"], ["highlight", "Highlight"], ["pen", "Pen"],
  ["highlighter_pen", "Highlighter pen"], ["eraser", "Eraser"], ["collect", "Collect"],
];

const host = {
  handle: null, session: null, registry: [], specs: new Map(), bindings: [], text: null,
  focusMode: false, layout: null,
  views: new Map(), order: [],
  well: { top: null, collapsed: false },
  highlights: [], ink: new Map(),
  palette: { expanded: null, rows: [], index: -1, result: null, returnTo: null },
  menu: null, log: [], targets: new Map(), colourRules: new Set(),
};

// MARK: - Commands

function hasSelection() {
  const s = document.getSelection();
  return !!s && s.rangeCount > 0 && !s.isCollapsed && !!viewOfNode(s.getRangeAt(0).commonAncestorContainer);
}

/** Dispatches one registered command; the only path from the UI to the session. A quiet
 *  command (tab.record_scroll) changes nothing on screen, so it does not re-render. */
export function run(invocation) {
  let outcome;
  try {
    outcome = host.handle.apply(invocation, { has_selection: hasSelection() });
  } catch (error) {
    toast(error.reason_text || error.message);
    host.log.push({ command_id: invocation.command_id, error: error.kind || String(error) });
    return null;
  }
  host.log.push({ command_id: invocation.command_id, handled: outcome.handled });
  const spec = host.specs.get(invocation.command_id);
  if (spec && spec.quiet) { host.session = host.handle.session(); return outcome; }
  if (outcome.handled) {
    host.session = host.handle.session();
    if (invocation.command_id === "nav.navigate" && invocation.args && invocation.args.reference) {
      host.targets.set(activeTab().id, invocation.args.reference);
    }
  } else {
    perform(invocation);
  }
  render();
  return outcome;
}

/** The host's half of the commands the core leaves to it (`handled: false`). */
function perform({ command_id: id }) {
  switch (id) {
    case "palette.focus": focusPalette(); break;
    case "view.focus_mode": host.focusMode = !host.focusMode; break;
    case "view.tool_palette":
      // Collapse to the grip where it is, so the grip stays under the pointer.
      if (host.well.top == null) host.well.top = $("well").getBoundingClientRect().top;
      host.well.collapsed = !host.well.collapsed;
      break;
    case "selection.highlight": highlightSelection(); break;
    case "selection.collect": collectSelection(); break;
    case "selection.study": toast("Word study is not built yet"); break;
    case "view.collection_drawer": toast("The collection drawer is not built yet"); break;
    case "view.study_drawer": toast("The study drawer is not built yet"); break;
    case "workspace.export": exportWorkspace(); break;
    default: toast(`${id} has no host action`);
  }
}

function exportWorkspace() {
  const records = host.handle.records();
  const ws = activeWorkspace();
  const json = JSON.stringify(records.workspaces.find((w) => w.id === ws.id), null, 2);
  window.openlogosExport = json;
  if (params.get("autotest")) return;
  const a = document.createElement("a");
  a.href = URL.createObjectURL(new Blob([json], { type: "application/json" }));
  a.download = `${ws.title}.workspace.json`;
  a.click();
  URL.revokeObjectURL(a.href);
}

// MARK: - Session helpers

const activeWorkspace = () => host.session.workspaces.find((w) => w.id === host.session.active_workspace_id);
const activeColumn = () => { const ws = activeWorkspace(); return ws.columns.find((c) => c.id === ws.active_column_id); };
const activeTab = () => { const c = activeColumn(); return c.tabs.find((t) => t.id === c.shown_tab_id); };
const shownTab = (c) => c.tabs.find((t) => t.id === c.shown_tab_id);
const tabKey = (ws, tab) => `${ws.id}/${tab.id}`;

/** The core's short form of a reference (`Jn 3:16`, `Gen 1:1–2`), or the text as given. */
function short(reference) {
  try { return core.formatShort(reference); } catch { return reference; }
}

/** The strip label: resource short name and chapter (`BSB Jn 3`), whatever verse is shown. */
function tabLabel(tab) {
  const r = host.text.resource(tab.content_id);
  if (r.kind === "notes" || tab.kind === "notes" || !tab.reference) return r.short;
  return `${r.short} ${short(tab.reference.split("-")[0].split(".").slice(0, 2).join("."))}`;
}

/** The reading settings in effect: size in px, line spacing, resolved theme, measure cap. */
function reading() {
  const r = host.session.reading;
  const base = TOKENS.sizes.indexOf(TOKENS.defaultSize);
  const size = TOKENS.sizes[Math.max(0, Math.min(TOKENS.sizes.length - 1, base + r.text_size_step))];
  const theme = r.theme === "system" ? (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light") : r.theme;
  // The core's measure caps (layoutConstants); the accessibility cap from 28 pt up.
  const measure = size >= 28 ? host.layout.measure_cap_accessibility_pt : host.layout.measure_cap_standard_pt;
  return { size, spacing: r.line_spacing, theme, measure };
}

/** Runs `then` with column `number` active, focusing it first when it is not. */
function inColumn(number, then) {
  const ws = activeWorkspace();
  if (ws.columns[number - 1] && ws.columns[number - 1].id !== ws.active_column_id) run({ command_id: "column.focus", args: { column: number } });
  return then();
}

// MARK: - Rendering

export function render() {
  const r = reading();
  root.dataset.theme = r.theme;
  root.dataset.tool = host.session.tool;
  root.classList.toggle("focus-mode", host.focusMode);
  root.classList.toggle("well-collapsed", host.well.collapsed);
  root.style.setProperty("--reading-size", `${r.size}px`);
  root.style.setProperty("--reading-leading", String(r.spacing));
  root.style.setProperty("--measure", `${r.measure}px`);
  const [swatch, alpha] = swatchPaint(host.session.ink.swatch);
  root.style.setProperty("--swatch", swatch);
  root.style.setProperty("--swatch-opacity", String(alpha));
  renderSwitcher();
  renderColumns();
  renderWell();
  paintHighlights();
}

function renderSwitcher() {
  const s = host.session;
  const index = s.workspaces.findIndex((w) => w.id === s.active_workspace_id);
  const button = $("workspace-switcher");
  const text = `<b>${escapeHtml(activeWorkspace().title)}</b> <span class="count">· ${index + 1}/${s.workspaces.length}</span> <span aria-hidden="true">▾</span>`;
  if (button.innerHTML !== text) button.innerHTML = text;
  button.setAttribute("aria-label", `Workspace ${activeWorkspace().title}, ${index + 1} of ${s.workspaces.length}`);
  const size = $("text-menu").querySelector(".size");
  const pt = String(reading().size);
  if (size.textContent !== pt) size.textContent = pt;
}

function columnSignature(ws, column, number) {
  const tabs = column.tabs.map((t) => `${t.id}:${t.content_id}:${t.reference}:${t.link_group}:${t.pinned}`).join("|");
  return `${ws.id}#${number}#${column.shown_tab_id}#${tabs}#${host.targets.get(column.shown_tab_id) || ""}`;
}

function renderColumns() {
  const ws = activeWorkspace();
  const container = $("columns");
  const order = [];
  const live = new Set();
  ws.columns.forEach((column, i) => {
    const key = `${ws.id}/${column.id}`;
    live.add(key);
    const signature = columnSignature(ws, column, i + 1);
    let view = host.views.get(key);
    if (!view || view.signature !== signature) {
      view = buildColumn(ws, column, i + 1);
      view.signature = signature;
      host.views.set(key, view);
    }
    const active = column.id === ws.active_column_id;
    view.section.classList.toggle("active", active);
    view.section.setAttribute("aria-current", active ? "true" : "false");
    view.section.style.flexGrow = String(column.width_fraction);
    if (i > 0) order.push(ruleBetween(ws, i));
    order.push(view.section);
  });
  if (ws.notes_dock.open) order.push(dockView(ws));
  for (const key of [...host.views.keys()]) if (!live.has(key)) host.views.delete(key);
  // Re-insert only when the order changed: moving a node resets its scroll position.
  const same = order.length === host.order.length && order.every((n, i) => n === host.order[i]);
  if (!same) {
    container.replaceChildren(...order);
    for (const view of host.views.values()) if (view.section.isConnected) restoreScroll(view);
    host.order = order;
  }
  for (const view of host.views.values()) if (view.section.isConnected) sizeCanvas(view);
}

const rules = new Map();
function ruleBetween(ws, index) {
  const key = `${ws.id}/${ws.columns[index - 1].id}|${ws.columns[index].id}`;
  let rule = rules.get(key);
  if (!rule) {
    rule = document.createElement("div");
    rule.className = "rule";
    rule.setAttribute("role", "separator");
    rule.setAttribute("aria-orientation", "vertical");
    rule.setAttribute("aria-label", `Resize columns ${index} and ${index + 1}`);
    rule.title = "Drag to resize · double-click to make the columns equal";
    rule.addEventListener("pointerdown", (e) => startResize(e, rule, index));
    rule.addEventListener("dblclick", () => run({ command_id: "column.equalize" }));
    rules.set(key, rule);
  }
  return rule;
}

function buildColumn(ws, column, number) {
  const tab = shownTab(column);
  const resource = host.text.resource(tab.content_id);
  const section = document.createElement("section");
  section.className = "column";
  section.dataset.number = String(number);
  section.dataset.columnId = column.id;
  section.setAttribute("aria-label", `Column ${number}: ${tabLabel(tab)}`);

  // Tab strip
  const strip = document.createElement("div");
  strip.className = "tabstrip";
  strip.setAttribute("role", "tablist");
  strip.setAttribute("aria-label", `Column ${number} tabs`);
  for (const t of column.tabs) {
    const shown = t.id === column.shown_tab_id;
    const el = document.createElement("div");
    el.className = "tab";
    el.setAttribute("role", "tab");
    el.setAttribute("aria-selected", String(shown));
    el.tabIndex = shown ? 0 : -1;
    el.dataset.tabId = t.id;
    const label = document.createElement("span");
    label.textContent = tabLabel(t);
    const close = document.createElement("button");
    close.type = "button";
    close.className = "close";
    close.textContent = "×";
    close.setAttribute("aria-label", `Close ${tabLabel(t)}`);
    close.title = `Close tab (${shortcutGlyphs(shortcutOf("tab.close"))})`;
    close.addEventListener("click", (e) => {
      e.stopPropagation();
      if (activeTab().id !== t.id) run({ command_id: "tab.focus", args: { workspace_id: ws.id, tab_id: t.id } });
      run({ command_id: "tab.close" });
    });
    el.append(label, close);
    el.addEventListener("click", () => { if (activeTab().id !== t.id) run({ command_id: "tab.focus", args: { workspace_id: ws.id, tab_id: t.id } }); });
    strip.appendChild(el);
  }
  const add = document.createElement("button");
  add.type = "button";
  add.className = "add";
  add.textContent = "+";
  add.setAttribute("aria-label", "New tab");
  add.title = `New tab (${shortcutGlyphs(shortcutOf("tab.new"))})`;
  add.addEventListener("click", () => inColumn(number, () => run({ command_id: "tab.new" })));
  strip.appendChild(add);

  // Identity row
  const identity = document.createElement("div");
  identity.className = "identity";
  const badge = document.createElement("span");
  badge.className = "badge";
  badge.textContent = String(number);
  badge.setAttribute("aria-label", `Column ${number}`);
  const name = document.createElement("span");
  name.className = "name";
  name.textContent = resource.name;
  const passage = document.createElement("span");
  passage.className = "passage";
  passage.textContent = resource.kind === "notes" ? "" : humanPassage(core, tab.reference);
  const link = document.createElement("span");
  link.className = "link";
  link.textContent = linkWord(tab);
  const menu = document.createElement("button");
  menu.type = "button";
  menu.className = "pane-menu";
  menu.textContent = "…";
  menu.setAttribute("aria-label", `Column ${number} menu`);
  menu.setAttribute("aria-haspopup", "menu");
  menu.addEventListener("click", () => inColumn(number, () => openPaneMenu(menu)));
  identity.append(badge, name, passage, link, menu);

  // Text
  const scroller = document.createElement("div");
  scroller.className = "scroller";
  scroller.tabIndex = -1;
  scroller.setAttribute("role", "region");
  scroller.setAttribute("aria-label", `${resource.name} ${passage.textContent}`.trim());
  const page = document.createElement("div");
  page.className = "page";
  scroller.appendChild(page);
  const view = { section, scroller, page, ws, column, tab, resource, number, text: null, paragraph: null, canvas: null, key: tabKey(ws, tab) };
  const passageText = host.text.passage(tab.content_id, tab.reference);
  if (passageText) {
    const article = document.createElement("article");
    article.className = "measure";
    article.lang = resource.lang;
    const text = renderParagraph(passageText.paragraph, resource.dir, resource.lang);
    const attribution = document.createElement("p");
    attribution.className = "attribution";
    attribution.textContent = passageText.attribution;
    article.append(text, attribution);
    page.appendChild(article);
    view.text = text;
    view.paragraph = passageText.paragraph;
    const target = host.targets.get(tab.id);
    if (target && /\.\d+$/.test(target.split("-")[0])) {
      const verse = text.querySelector(`[data-segment="${target.split("-")[0]}"]`);
      if (verse) { verse.classList.add("target"); view.scrollTo = verse; }
    }
    const canvas = document.createElement("canvas");
    canvas.className = "margin";
    canvas.setAttribute("aria-hidden", "true");
    page.appendChild(canvas);
    view.canvas = canvas;
    attachInk(view);
  } else {
    page.appendChild(placeholder(ws, tab, resource));
  }
  scroller.addEventListener("scroll", () => {
    clearTimeout(view.scrollTimer);
    view.scrollTimer = setTimeout(() => recordScroll(view), 150);
  }, { passive: true });
  section.addEventListener("pointerdown", (e) => {
    if (e.target.closest(".tabstrip, .pane-menu")) return;
    if (activeWorkspace().active_column_id !== column.id) {
      const n = activeWorkspace().columns.findIndex((c) => c.id === column.id) + 1;
      if (n > 0) run({ command_id: "column.focus", args: { column: n } });
    }
  });
  section.append(strip, identity, scroller);
  return view;
}

/** specs/21 §11 "Unavailable source": the resource and passage, and what can be done. */
function placeholder(ws, tab, resource) {
  const box = document.createElement("div");
  box.className = "placeholder";
  const where = humanPassage(core, tab.reference);
  box.innerHTML = `<h2>${escapeHtml(resource.name)} · ${escapeHtml(where)}</h2>
    <p>This passage is not installed. The preview carries only the fixture passages (John 3:16–17, Genesis 1:1–2 and the study note) until the preview pack lands.</p>
    <div class="actions"><button type="button" data-act="choose">Choose another…</button><button type="button" data-act="close">Close tab</button></div>`;
  box.querySelector('[data-act="choose"]').addEventListener("click", () => run({ command_id: "palette.focus" }));
  box.querySelector('[data-act="close"]').addEventListener("click", () => run({ command_id: "tab.close" }));
  return box;
}

let dock = null;
function dockView(ws) {
  if (dock && dock.ws === ws.id) return dock.el;
  const el = document.createElement("aside");
  el.className = "dock";
  el.setAttribute("aria-label", "Notes dock");
  const h = document.createElement("h2");
  h.textContent = "Notes";
  const scroller = document.createElement("div");
  scroller.className = "scroller";
  const note = host.text.passage(ws.notes_dock.note_id || "fixture:note:1", null);
  const article = document.createElement("article");
  article.className = "measure";
  if (note) article.appendChild(renderParagraph(note.paragraph, "ltr", "en"));
  scroller.appendChild(article);
  el.append(h, scroller);
  dock = { ws: ws.id, el };
  return el;
}

/** Where the reader is: the segment and scalar at the top of the text, sent to the core as
 *  `tab.record_scroll` once scrolling settles (a quiet command: no re-render, no undo). */
function scrollAnchorOf(view) {
  if (!view.text || !view.section.isConnected) return null;
  const box = view.scroller.getBoundingClientRect();
  const text = view.text.getBoundingClientRect();
  const rtl = view.resource.dir === "rtl";
  const x = rtl ? Math.min(text.right, box.right) - 2 : Math.max(text.left, box.left) + 2;
  const y = box.top + 2;
  const caret = document.caretRangeFromPoint ? document.caretRangeFromPoint(x, y) : null;
  const node = caret && caret.startContainer;
  const verse = node && node.parentElement && node.parentElement.closest("[data-segment]");
  if (verse && view.text.contains(verse) && node.parentElement.classList.contains("t")) {
    return { segment_id: verse.dataset.segment, scalar: [...node.data.slice(0, caret.startOffset)].length };
  }
  // Between lines or above the text: the first segment whose bottom is below the top edge.
  for (const v of view.text.querySelectorAll("[data-segment]")) {
    if (v.getBoundingClientRect().bottom > y) return { segment_id: v.dataset.segment, scalar: 0 };
  }
  return null;
}

function recordScroll(view) {
  const anchor = scrollAnchorOf(view);
  if (!anchor) return;
  const old = view.tab.scroll_anchor;
  if (!old && view.scroller.scrollTop < 4) return;
  if (old && old.segment_id === anchor.segment_id && old.scalar === anchor.scalar) return;
  run({ command_id: "tab.record_scroll", args: { workspace_id: view.ws.id, tab_id: view.tab.id, ...anchor } });
  view.tab = { ...view.tab, scroll_anchor: anchor };
}

/** Scrolls a freshly built column to a navigation target, or to its tab's scroll_anchor. */
function restoreScroll(view) {
  if (view.scrollTo) { view.scroller.scrollTop = Math.max(0, view.scrollTo.offsetTop - 24); view.scrollTo = null; return; }
  const anchor = view.tab.scroll_anchor;
  const verse = anchor && view.text && view.text.querySelector(`[data-segment="${CSS.escape(anchor.segment_id)}"]`);
  if (!verse) return;
  const node = verse.querySelector(".t").firstChild;
  let top = verse.getBoundingClientRect().top;
  if (node && anchor.scalar > 0) {
    const r = document.createRange();
    const at = [...node.data].slice(0, anchor.scalar).join("").length;
    r.setStart(node, Math.min(at, node.length));
    r.setEnd(node, Math.min(at + 1, node.length));
    top = r.getBoundingClientRect().top;
  }
  view.scroller.scrollTop += top - view.scroller.getBoundingClientRect().top - 1;
}

function viewOfNode(node) {
  for (const view of host.views.values()) if (view.text && view.section.isConnected && view.text.contains(node)) return view;
  return null;
}

// MARK: - Column resizing: a drag previews the core's rule and ends in `column.resize`

function startResize(event, rule, index) {
  event.preventDefault();
  const ws = activeWorkspace();
  const shares = ws.columns.map((c) => c.width_fraction);
  const views = ws.columns.map((c) => host.views.get(`${ws.id}/${c.id}`));
  const total = $("columns").getBoundingClientRect().width;
  const x0 = event.clientX;
  // A tenth of the window at least (the core accepts down to 0.05), the rest for the others.
  const min = 0.1, max = 1 - min * (ws.columns.length - 1);
  let share = shares[index - 1];
  rule.setPointerCapture(event.pointerId);
  rule.classList.add("dragging");
  // column.resize gives column N its share and scales the others in proportion; the preview
  // does the same, so the drop does not jump.
  const apply = () => {
    const others = 1 - shares[index - 1];
    views.forEach((v, i) => { v.section.style.flexGrow = String(i === index - 1 ? share : (shares[i] * (1 - share)) / others); });
  };
  const move = (e) => {
    share = Math.max(min, Math.min(max, shares[index - 1] + (e.clientX - x0) / total));
    apply();
  };
  const up = () => {
    rule.classList.remove("dragging");
    rule.removeEventListener("pointermove", move);
    rule.removeEventListener("pointerup", up);
    if (Math.abs(share - shares[index - 1]) > 1e-4) run({ command_id: "column.resize", args: { column: index, number: share } });
    for (const view of host.views.values()) if (view.section.isConnected) sizeCanvas(view);
  };
  rule.addEventListener("pointermove", move);
  rule.addEventListener("pointerup", up);
}

// MARK: - Highlights (anchors through the core, as the B005 reader)

function selectionInView() {
  const s = document.getSelection();
  if (!s || s.rangeCount === 0 || s.isCollapsed) return null;
  const range = s.getRangeAt(0);
  const view = viewOfNode(range.commonAncestorContainer);
  if (!view) return null;
  const trip = selectionAnchors(core, view.paragraph, view.text, range);
  return trip ? { view, trip } : null;
}

function highlightSelection() {
  const found = selectionInView();
  if (!found) return toast("Select some text first");
  host.highlights.push({ edition_id: found.view.paragraph.edition_id, anchors: found.trip.anchors, swatch: host.session.ink.swatch.toUpperCase() });
  document.getSelection().removeAllRanges();
}

function collectSelection() {
  const found = selectionInView();
  if (!found) return toast("Select some text first");
  const ids = found.trip.anchors.map((a) => a.segment_id);
  const first = ids[0], last = ids[ids.length - 1];
  const where = /^[A-Za-z0-9]+\.\d+\.\d+$/.test(first) ? short(first === last ? first : `${first}-${last}`) : found.view.resource.name;
  host.collected = (host.collected || 0) + 1;
  document.getSelection().removeAllRanges();
  toast(`Collected: ${where}`);
}

/** The highlight name a swatch paints under: the token swatches have their rules in
 *  shell.css; another colour gets a rule here, at 30 %. */
function highlightName(hex) {
  const known = swatchName(hex);
  if (known) return `ol-${known}`;
  const name = `ol-c${hex.slice(1).toLowerCase()}`;
  if (!host.colourRules.has(name)) {
    host.colourRules.add(name);
    let sheet = $("swatch-rules");
    if (!sheet) { sheet = document.createElement("style"); sheet.id = "swatch-rules"; document.head.appendChild(sheet); }
    sheet.textContent += `::highlight(${name}) { background-color: color-mix(in srgb, ${hex} 30%, transparent); }\n`;
  }
  return name;
}

function paintHighlights() {
  if (!("highlights" in CSS)) return;
  const bySwatch = new Map(SWATCH_ORDER.map((s) => [`ol-${s}`, []]));
  for (const view of host.views.values()) {
    if (!view.text || !view.section.isConnected) continue;
    for (const h of host.highlights) {
      if (h.edition_id !== view.paragraph.edition_id) continue;
      for (const anchor of h.anchors) {
        if (!view.paragraph.segments.some((s) => s.segment_id === anchor.segment_id)) continue;
        const name = highlightName(h.swatch);
        if (!bySwatch.has(name)) bySwatch.set(name, []);
        bySwatch.get(name).push(anchorRange(core, view.paragraph, view.text, anchor));
      }
    }
  }
  for (const [name, ranges] of bySwatch) CSS.highlights.set(name, new Highlight(...ranges));
}

// A drag with the Highlight or Collect tool ends in the command, with no menu (specs/22 §4).
document.addEventListener("pointerup", () => {
  const tool = host.session && host.session.tool;
  if (tool !== "highlight" && tool !== "collect") return;
  // After the browser has settled the selection for this pointerup.
  setTimeout(() => { if (hasSelection()) run({ command_id: tool === "highlight" ? "selection.highlight" : "selection.collect" }); }, 0);
});

// MARK: - Ink in the margin layer (B039's brushes and compositing, ../ink/layer.js)

function inkOf(key) {
  let ink = host.ink.get(key);
  if (!ink) { ink = { strokes: [], deleted: new Set(), serial: 0 }; host.ink.set(key, ink); }
  return ink;
}

function sizeCanvas(view) {
  if (!view.canvas) return;
  const dpr = window.devicePixelRatio || 1;
  const w = view.page.scrollWidth, h = Math.max(view.page.scrollHeight, view.scroller.clientHeight);
  if (view.canvas.width === Math.round(w * dpr) && view.canvas.height === Math.round(h * dpr)) return;
  view.canvas.width = Math.round(w * dpr);
  view.canvas.height = Math.round(h * dpr);
  view.canvas.style.width = `${w}px`;
  view.canvas.style.height = `${h}px`;
  repaintInk(view);
}

function repaintInk(view, live = null) {
  const ctx = view.canvas.getContext("2d");
  const dpr = window.devicePixelRatio || 1;
  ctx.clearRect(0, 0, view.canvas.width, view.canvas.height);
  const ink = host.ink.get(view.key);
  const strokes = ink ? ink.strokes.filter((s) => !ink.deleted.has(s.id)) : [];
  if (live) strokes.push(live);
  for (const stroke of strokes) if (stroke.points.length >= 2) compositeAt(ctx, stroke, strokeOutline(core, stroke), dpr);
}

function attachInk(view) {
  const canvas = view.canvas;
  const arbiter = new TouchArbiter("stylus");
  let live = null, scheduled = false;
  const point = (e) => { const r = canvas.getBoundingClientRect(); return { x: e.clientX - r.left, y: e.clientY - r.top }; };
  const contact = (e) => (e.pointerType === "pen" ? "stylus" : e.pointerType === "touch" ? "finger" : "pointer");
  const frame = () => { if (scheduled) return; scheduled = true; requestAnimationFrame(() => { scheduled = false; repaintInk(view, live); }); };
  const erase = (p) => {
    const ink = inkOf(view.key);
    for (let i = ink.strokes.length - 1; i >= 0; i--) {
      const s = ink.strokes[i];
      if (ink.deleted.has(s.id)) continue;
      for (let j = 1; j < s.points.length; j++) {
        if (distanceToSegment(p, s.points[j - 1], s.points[j]) <= s.width / 2 + 4) { ink.deleted.add(s.id); repaintInk(view); return; }
      }
    }
  };
  canvas.addEventListener("pointerdown", (e) => {
    // A finger always scrolls and selects (specs/22 §4); a pen or mouse uses the tool.
    const decision = arbiter.decide({ id: e.pointerId, contact: contact(e), phase: "began", pane: 0, radius: null });
    if (decision !== "draw") return;
    e.preventDefault();
    canvas.setPointerCapture(e.pointerId);
    const tool = host.session.tool;
    if (tool === "eraser") { live = { eraser: true }; erase(point(e)); return; }
    const brush = tool === "highlighter_pen" ? BRUSHES.highlighter : BRUSHES.pen;
    // The session's brush width; the highlighter band is wider (B039's factor).
    const width = host.session.ink.width * (tool === "highlighter_pen" ? HIGHLIGHTER_WIDTH_FACTOR : 1);
    const color = tool === "highlighter_pen" ? host.session.ink.swatch : getComputedStyle(root).getPropertyValue("--accent").trim();
    const p = point(e);
    live = { id: `stroke-${++inkOf(view.key).serial}`, tool: brush.tool, brush_version: brush.version, width, color, opacity: brush.opacity, points: [{ ...p, pressure: e.pointerType === "pen" ? e.pressure : null, elapsed_ms: 0 }], started: e.timeStamp };
  });
  canvas.addEventListener("pointermove", (e) => {
    if (!live) return;
    if (live.eraser) { erase(point(e)); return; }
    const events = typeof e.getCoalescedEvents === "function" && e.getCoalescedEvents().length ? e.getCoalescedEvents() : [e];
    for (const c of events) live.points.push({ ...point(c), pressure: c.pointerType === "pen" ? c.pressure : null, elapsed_ms: Math.max(0, c.timeStamp - live.started) });
    frame();
  });
  const end = (e) => {
    arbiter.decide({ id: e.pointerId, contact: contact(e), phase: e.type === "pointercancel" ? "cancelled" : "ended", pane: 0, radius: null });
    if (!live) return;
    if (!live.eraser && live.points.length >= 2 && e.type !== "pointercancel") {
      const { started, ...stroke } = live;
      inkOf(view.key).strokes.push(stroke);
    }
    live = null;
    repaintInk(view);
  };
  canvas.addEventListener("pointerup", end);
  canvas.addEventListener("pointercancel", end);
}

// MARK: - The floating tool palette

function shortcutOf(commandId) {
  const spec = host.specs.get(commandId);
  return spec && spec.shortcuts.web.length ? spec.shortcuts.web[0] : "";
}

function buildWell() {
  const well = $("well");
  well.innerHTML = "";
  const grip = document.createElement("div");
  grip.className = "grip";
  grip.setAttribute("role", "button");
  grip.tabIndex = 0;
  grip.setAttribute("aria-label", "Tool palette grip: drag to move, double-click to collapse");
  grip.title = "Drag to move · double-click to collapse";
  grip.innerHTML = '<svg viewBox="0 0 18 6" aria-hidden="true"><g fill="currentColor"><circle cx="3" cy="1.5" r="1"/><circle cx="7" cy="1.5" r="1"/><circle cx="11" cy="1.5" r="1"/><circle cx="15" cy="1.5" r="1"/><circle cx="3" cy="4.5" r="1"/><circle cx="7" cy="4.5" r="1"/><circle cx="11" cy="4.5" r="1"/><circle cx="15" cy="4.5" r="1"/></g></svg>';
  grip.addEventListener("dblclick", () => run({ command_id: "view.tool_palette" }));
  grip.addEventListener("keydown", (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); run({ command_id: "view.tool_palette" }); } });
  grip.addEventListener("pointerdown", (e) => {
    const start = e.clientY, top0 = well.getBoundingClientRect().top;
    grip.setPointerCapture(e.pointerId);
    const move = (m) => {
      const columns = $("columns").getBoundingClientRect();
      host.well.top = Math.max(columns.top + 8, Math.min(columns.bottom - well.offsetHeight - 8, top0 + m.clientY - start));
      placeWell();
    };
    const up = () => { grip.removeEventListener("pointermove", move); grip.removeEventListener("pointerup", up); };
    grip.addEventListener("pointermove", move);
    grip.addEventListener("pointerup", up);
  });
  well.appendChild(grip);
  for (const [id, title] of TOOLS) {
    const b = document.createElement("button");
    b.type = "button";
    b.className = "tool";
    b.dataset.tool = id;
    const key = shortcutGlyphs(shortcutOf(`tool.${id}`));
    b.title = `${title} (${key})`;
    b.setAttribute("aria-label", title);
    b.setAttribute("aria-keyshortcuts", shortcutOf(`tool.${id}`));
    b.innerHTML = `<svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${ICONS[id]}</svg>`;
    b.addEventListener("click", () => run({ command_id: `tool.${id}` }));
    well.appendChild(b);
  }
  well.appendChild(document.createElement("hr"));
  // Two overlapping wells: the current one chooses a swatch (tool.swatch), the alternate one
  // swaps them (tool.swap_swatches, X).
  const swatches = document.createElement("div");
  swatches.className = "swatches";
  swatches.innerHTML = '<button type="button" class="alternate"></button><button type="button" class="current" aria-haspopup="menu"></button>';
  swatches.querySelector(".alternate").addEventListener("click", () => run({ command_id: "tool.swap_swatches" }));
  swatches.querySelector(".current").addEventListener("click", (e) => openSwatchMenu(e.currentTarget));
  well.appendChild(swatches);
  // The width dot steps up through the core's widths (tool.width_up) and wraps to the
  // narrowest (tool.width) after the widest.
  const dot = document.createElement("button");
  dot.type = "button";
  dot.className = "width-dot";
  dot.innerHTML = "<span></span>";
  dot.addEventListener("click", () => {
    const state = commandStates().get("tool.width_up");
    run(state && state.disabled ? { command_id: "tool.width", args: { number: 1 } } : { command_id: "tool.width_up" });
  });
  well.appendChild(dot);
}

function renderWell() {
  const well = $("well");
  well.classList.toggle("collapsed", host.well.collapsed);
  const tool = host.session.tool;
  const toolTitle = TOOLS.find(([id]) => id === tool)[1];
  well.setAttribute("aria-label", `Tools, current: ${toolTitle}`);
  for (const b of well.querySelectorAll(".tool")) {
    const on = b.dataset.tool === tool;
    if (b.getAttribute("aria-pressed") !== String(on)) b.setAttribute("aria-pressed", String(on));
  }
  const { swatch, alternate_swatch: alternate, width: w } = host.session.ink;
  const label = (hex) => swatchName(hex) || hex;
  const sw = well.querySelector(".swatches");
  const cur = sw.querySelector(".current"), alt = sw.querySelector(".alternate");
  cur.style.background = swatch;
  alt.style.background = alternate;
  cur.title = `Swatch: ${label(swatch)} (choose)`;
  cur.setAttribute("aria-label", `Current swatch ${label(swatch)}. Choose a swatch`);
  alt.title = `Alternate: ${label(alternate)} (${shortcutGlyphs(shortcutOf("tool.swap_swatches"))} swaps)`;
  alt.setAttribute("aria-label", `Alternate swatch ${label(alternate)}. Swap swatches`);
  const dot = well.querySelector(".width-dot");
  dot.setAttribute("aria-label", `Brush width ${w}. Change`);
  dot.title = `Brush width ${w}`;
  dot.firstChild.style.width = dot.firstChild.style.height = `${Math.min(18, w + 3)}px`;
  placeWell();
}

function placeWell() {
  const well = $("well");
  if (host.well.top == null) {
    const columns = $("columns").getBoundingClientRect();
    well.style.top = `${columns.top + (columns.height - well.offsetHeight) / 2}px`;
  } else {
    well.style.top = `${host.well.top}px`;
  }
}

// MARK: - Menus: workspace switcher, text menu, column menu

function closeMenu(restore = true) {
  if (!host.menu) return;
  const { el, button } = host.menu;
  el.remove();
  button.setAttribute("aria-expanded", "false");
  host.menu = null;
  if (restore) button.focus();
}

function openMenu(button, build, role = "menu") {
  const reopen = host.menu && host.menu.button === button;
  closeMenu(false);
  if (reopen) return;
  const el = document.createElement("div");
  el.className = "menu";
  el.setAttribute("role", role);
  build(el);
  document.body.appendChild(el);
  const r = button.getBoundingClientRect();
  el.style.top = `${r.bottom + 4}px`;
  el.style.left = `${Math.max(8, Math.min(window.innerWidth - el.offsetWidth - 8, r.right - el.offsetWidth))}px`;
  button.setAttribute("aria-expanded", "true");
  host.menu = { el, button };
  const first = el.querySelector('[role^="menuitem"]:not([aria-disabled="true"]), button');
  if (first) first.focus();
}

/** A menu item that runs a command, disabled with the core's reason (`disabled_text`) when it
 *  cannot; `extra.disabled` overrides it (null: always enabled). */
function commandItem(menu, label, invocation, states, extra = {}) {
  const b = document.createElement("button");
  b.type = "button";
  b.setAttribute("role", extra.checked == null ? "menuitem" : "menuitemradio");
  if (extra.checked != null) b.setAttribute("aria-checked", String(extra.checked));
  const state = states.get(invocation.command_id);
  const disabled = extra.disabled !== undefined ? extra.disabled : state && state.disabled_text;
  const key = shortcutGlyphs(shortcutOf(invocation.command_id));
  b.innerHTML = `<span>${extra.checked != null ? `<span class="check">${extra.checked ? "✓" : ""}</span>` : ""}${escapeHtml(label)}</span>${disabled ? `<span class="why">${escapeHtml(disabled)}</span>` : key && !extra.noKey ? `<span class="key">${escapeHtml(key)}</span>` : ""}`;
  if (disabled) b.setAttribute("aria-disabled", "true");
  b.addEventListener("click", () => {
    if (disabled) return;
    closeMenu(false);
    if (extra.then) extra.then(); else run(invocation);
  });
  menu.appendChild(b);
}

function commandStates() {
  return new Map(host.handle.commands({ has_selection: hasSelection() }).map((s) => [s.spec.id, s]));
}

function openSwitcher() {
  const button = $("workspace-switcher");
  openMenu(button, (menu) => {
    const states = commandStates();
    for (const ws of host.session.workspaces) {
      commandItem(menu, ws.title, { command_id: "workspace.switch", args: { workspace_id: ws.id } }, states, { checked: ws.id === host.session.active_workspace_id, disabled: null, noKey: true });
    }
    menu.appendChild(document.createElement("hr"));
    commandItem(menu, "Next workspace", { command_id: "workspace.next" }, states);
    commandItem(menu, "Previous workspace", { command_id: "workspace.previous" }, states);
    menu.appendChild(document.createElement("hr"));
    commandItem(menu, "New workspace", { command_id: "workspace.new" }, states);
    commandItem(menu, "Duplicate workspace", { command_id: "workspace.duplicate" }, states);
    commandItem(menu, "Rename workspace…", { command_id: "workspace.rename" }, states, { disabled: null, then: () => focusPalette("Rename workspace ") });
    commandItem(menu, "Export workspace", { command_id: "workspace.export" }, states);
    commandItem(menu, "Close workspace", { command_id: "workspace.close" }, states);
  });
}

function openTextMenu() {
  const button = $("text-menu");
  openMenu(button, (menu) => {
    const size = document.createElement("div");
    size.className = "group";
    size.innerHTML = `<div class="group-label">Text size</div><div class="segmented"><button type="button" data-c="view.text_size_down" aria-label="Smaller text">A−</button><span class="size-label">${reading().size} pt</span><button type="button" data-c="view.text_size_reset" aria-label="Default text size">A</button><button type="button" data-c="view.text_size_up" aria-label="Larger text">A+</button></div>`;
    const spacing = document.createElement("div");
    spacing.className = "group";
    spacing.innerHTML = `<div class="group-label">Line spacing</div><div class="segmented" role="radiogroup" aria-label="Line spacing">${TOKENS.spacings.map((v, i) => `<button type="button" role="radio" aria-checked="${v === host.session.reading.line_spacing}" data-spacing="${v}">${["Tight", "Normal", "Loose"][i]}</button>`).join("")}</div>`;
    const theme = document.createElement("div");
    theme.className = "group";
    theme.innerHTML = `<div class="group-label">Theme</div><div class="segmented" role="radiogroup" aria-label="Theme">${TOKENS.themes.map((t) => `<button type="button" role="radio" aria-checked="${t === reading().theme}" data-theme="${t}">${t[0].toUpperCase()}${t.slice(1)}</button>`).join("")}</div>`;
    menu.append(size, spacing, theme);
    const refresh = () => { closeMenu(false); openTextMenu(); };
    size.querySelectorAll("[data-c]").forEach((b) => b.addEventListener("click", () => { run({ command_id: b.dataset.c }); refresh(); }));
    spacing.querySelectorAll("[data-spacing]").forEach((b) => b.addEventListener("click", () => { run({ command_id: "view.line_spacing", args: { number: Number(b.dataset.spacing) } }); refresh(); }));
    theme.querySelectorAll("[data-theme]").forEach((b) => b.addEventListener("click", () => { run({ command_id: "view.theme", args: { text: b.dataset.theme } }); refresh(); }));
  }, "dialog");
}

function openSwatchMenu(button) {
  openMenu(button, (menu) => {
    const states = commandStates();
    for (const name of SWATCH_ORDER) {
      const hex = TOKENS.swatches[name][0];
      commandItem(menu, `${name[0].toUpperCase()}${name.slice(1)}`, { command_id: "tool.swatch", args: { text: hex } }, states, { checked: host.session.ink.swatch.toUpperCase() === hex, disabled: null, noKey: true });
    }
  });
}

function openPaneMenu(button) {
  openMenu(button, (menu) => {
    const states = commandStates();
    const ws = activeWorkspace();
    commandItem(menu, "Link to group 1", { command_id: "tab.link", args: { group: "1" } }, states);
    commandItem(menu, "Pin tab", { command_id: "tab.pin" }, states);
    commandItem(menu, "Follow active passage", { command_id: "tab.follow" }, states);
    menu.appendChild(document.createElement("hr"));
    commandItem(menu, "New tab", { command_id: "tab.new" }, states);
    commandItem(menu, "Close tab", { command_id: "tab.close" }, states);
    commandItem(menu, "Reopen closed tab", { command_id: "tab.reopen_closed" }, states);
    const here = ws.columns.findIndex((c) => c.id === ws.active_column_id) + 1;
    for (let n = 1; n <= ws.columns.length; n++) {
      if (n === here) continue;
      commandItem(menu, `Move tab to column ${n}`, { command_id: "tab.move_to_column", args: { column: n } }, states, { noKey: true, disabled: null });
    }
    menu.appendChild(document.createElement("hr"));
    commandItem(menu, "New column", { command_id: "column.new" }, states);
    commandItem(menu, "Move column left", { command_id: "column.move_left" }, states);
    commandItem(menu, "Move column right", { command_id: "column.move_right" }, states);
    commandItem(menu, "Close column", { command_id: "column.close" }, states);
    commandItem(menu, ws.notes_dock.open ? "Hide notes dock" : "Show notes dock", { command_id: "view.notes_dock" }, states);
  });
}

// MARK: - The palette

const field = $("palette-field");
const results = $("palette-results");

function focusPalette(text) {
  if (document.activeElement !== field) host.palette.returnTo = document.activeElement;
  if (text != null) field.value = text;
  field.focus();
  if (text == null) field.select();
  queryPalette();
}

function closePalette(restore) {
  results.hidden = true;
  field.setAttribute("aria-expanded", "false");
  host.palette.expanded = null;
  host.palette.rows = [];
  host.palette.index = -1;
  if (restore) {
    const back = host.palette.returnTo;
    host.palette.returnTo = null;
    field.blur();
    const view = host.views.get(`${activeWorkspace().id}/${activeColumn().id}`);
    const target = back && back !== document.body && back.isConnected && view && view.section.contains(back) ? back : view && view.scroller;
    if (target) target.focus({ preventScroll: true });
  }
}

/** Every keystroke asks the core; nothing is debounced (specs/22 §7: one frame). */
export function queryPalette() {
  const text = field.value;
  const result = host.handle.paletteQuery(text, { platform: "web", has_selection: hasSelection(), expanded: host.palette.expanded });
  host.palette.result = result;
  renderPalette(result);
  return result;
}

const SECTION_TITLES = { passage: "Passage", commands: "Commands", tabs: "Tabs and workspaces", text: "Text" };

function renderPalette(result) {
  const rows = [];
  const parts = [];
  for (const group of result.groups) {
    if (group.items.length === 0) continue;
    parts.push(`<h3 id="ph-${group.section}">${SECTION_TITLES[group.section]}</h3>`);
    for (const item of group.items) {
      const i = rows.length;
      rows.push({ kind: "item", item, section: group.section });
      const title = item.title;
      const sub = item.disabled ? item.disabled_text : item.subtitle || "";
      parts.push(`<div class="row${item.disabled ? " disabled" : ""}" role="option" id="pr-${i}" data-i="${i}" aria-selected="false"${item.disabled ? ' aria-disabled="true"' : ""}><span class="title">${escapeHtml(title)}</span>${item.shortcut ? `<span class="key">${escapeHtml(shortcutGlyphs(item.shortcut))}</span>` : ""}${sub ? `<span class="sub">${escapeHtml(sub)}</span>` : ""}</div>`);
    }
    if (group.total > group.items.length) {
      const i = rows.length;
      rows.push({ kind: "more", section: group.section });
      parts.push(`<div class="row more" role="option" id="pr-${i}" data-i="${i}" aria-selected="false"><span class="title">Show all ${group.total}</span><span class="key">⌥↩</span></div>`);
    }
  }
  if (result.notice) parts.push(`<div class="hint">${escapeHtml(result.notice)}</div>`);
  if (rows.length === 0) {
    parts.push(field.value.trim()
      ? `<div class="hint">Nothing matches “${escapeHtml(field.value.trim())}”.</div>`
      : `<div class="hint">Type a passage (<kbd>Jn 3:16</kbd>), a word, a command or a tab. <kbd>&gt;</kbd> commands · <kbd>@</kbd> tabs and workspaces · <kbd>#</kbd> text · <kbd>:</kbd> passage · <kbd>@2</kbd> at the end opens in column 2.</div>`);
  }
  results.innerHTML = parts.join("");
  host.palette.rows = rows;
  results.hidden = false;
  field.setAttribute("aria-expanded", "true");
  const firstEnabled = rows.findIndex((r) => r.kind === "item" && !r.item.disabled);
  select(firstEnabled >= 0 ? firstEnabled : rows.length ? 0 : -1);
}

function select(i) {
  const old = results.querySelector('[aria-selected="true"]');
  if (old) old.setAttribute("aria-selected", "false");
  host.palette.index = i;
  if (i < 0) { field.removeAttribute("aria-activedescendant"); return; }
  const row = $(`pr-${i}`);
  row.setAttribute("aria-selected", "true");
  field.setAttribute("aria-activedescendant", row.id);
  row.scrollIntoView({ block: "nearest" });
}

function expand(section) {
  host.palette.expanded = section;
  queryPalette();
}

function activate(i, { expandSection = false } = {}) {
  const row = host.palette.rows[i];
  if (!row) return;
  if (row.kind === "more" || expandSection) { expand(row.section); return; }
  const { item } = row;
  if (item.disabled) {
    // A command that needs its argument: put its title in the field to type it after.
    if (item.disabled.kind === "needs_argument" && item.section === "commands") { field.value = `${item.title} `; host.palette.expanded = null; queryPalette(); return; }
    toast(item.disabled_text);
    return;
  }
  // Selection commands need the selection the field's focus does not take away.
  closePalette(item.invocation.command_id !== "palette.focus");
  field.value = "";
  run(item.invocation);
}

field.addEventListener("focus", () => { if (results.hidden) queryPalette(); });
field.addEventListener("input", (e) => {
  host.palette.expanded = null;
  queryPalette();
  if (e.isTrusted && host.recordKeys) {
    // From the keydown that typed the character (the input event is created after it).
    const start = host.keyDownAt ?? e.timeStamp;
    void document.body.offsetHeight;
    const sample = { done: performance.now() - start };
    keyLatency.push(sample);
    requestAnimationFrame(() => { sample.frame = performance.now() - start; });
  }
});
field.addEventListener("keydown", (e) => {
  host.keyDownAt = e.timeStamp;
  const rows = host.palette.rows;
  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    if (!rows.length) return;
    const d = e.key === "ArrowDown" ? 1 : -1;
    select((host.palette.index + d + rows.length) % rows.length);
  } else if (e.key === "Enter") {
    e.preventDefault();
    activate(host.palette.index, { expandSection: e.altKey });
  } else if (e.key === "Tab" && !e.shiftKey && rows[host.palette.index]) {
    const row = rows[host.palette.index];
    const group = host.palette.result.groups.find((g) => g.section === row.section);
    if (group && group.total > group.items.length) { e.preventDefault(); expand(row.section); }
  } else if (e.key === "Escape") {
    e.preventDefault();
    e.stopPropagation();
    closePalette(true);
  }
});
results.addEventListener("pointerdown", (e) => e.preventDefault()); // keep the field focused
results.addEventListener("click", (e) => {
  const row = e.target.closest(".row");
  if (row) activate(Number(row.dataset.i));
});
document.addEventListener("pointerdown", (e) => {
  if (!results.hidden && !e.target.closest(".field-wrap")) closePalette(false);
  if (host.menu && !host.menu.el.contains(e.target) && !host.menu.button.contains(e.target)) closeMenu(false);
});

// MARK: - Keyboard: the registry's bindings

function inTextField(el) {
  return !!el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable);
}

document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && host.menu) { e.preventDefault(); closeMenu(true); return; }
  const typing = inTextField(document.activeElement);
  const invocation = match(host.bindings, e, typing);
  if (!invocation) return;
  // In the field, ⌘Z and the history keys are the field's own.
  if (typing && ["edit.undo", "edit.redo", "nav.back", "nav.forward"].includes(invocation.command_id)) return;
  e.preventDefault();
  if (typing && invocation.command_id !== "palette.focus") closePalette(false);
  run(invocation);
}, true);

$("workspace-switcher").addEventListener("click", openSwitcher);
$("text-menu").addEventListener("click", openTextMenu);
window.addEventListener("resize", () => { placeWell(); for (const v of host.views.values()) if (v.section.isConnected) sizeCanvas(v); });

// MARK: - Toast

let toastTimer = 0;
function toast(message) {
  const el = $("toast");
  el.textContent = message;
  el.classList.add("show");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.remove("show"), 2400);
  host.lastToast = message;
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
}

// MARK: - Measurement (specs/22 §7)

const nextFrame = () => new Promise((ok) => requestAnimationFrame(ok));

function stats(values) {
  const sorted = [...values].sort((a, b) => a - b);
  const q = (p) => sorted[Math.min(sorted.length - 1, Math.floor(p * sorted.length))];
  return { n: sorted.length, p50: +q(0.5).toFixed(3), p95: +q(0.95).toFixed(3), max: +sorted[sorted.length - 1].toFixed(3) };
}

/** One interaction, `samples` times, each started as a task at a random point of a frame (as
 *  an input event arrives): `work` is the synchronous script, style and layout of the change
 *  (performance.now() around the action with a forced layout), `frame` the time from the start
 *  of the action to the requestAnimationFrame callback of the next frame, the one that paints
 *  it (so it includes waiting for that frame: up to one 16.7 ms interval at 60 Hz). */
async function measure(name, samples, action) {
  const work = [], frame = [];
  for (let i = 0; i < samples; i++) {
    await nextFrame();
    await new Promise((ok) => setTimeout(ok, Math.random() * 15));
    const t0 = performance.now();
    action(i);
    void document.body.offsetHeight;
    const t1 = performance.now();
    const t2 = await new Promise((ok) => requestAnimationFrame(() => ok(performance.now())));
    work.push(t1 - t0);
    frame.push(t2 - t0);
  }
  return { name, work_ms: stats(work), frame_ms: stats(frame) };
}

/** Trusted keystrokes (typed by the WebDriver runner into the field): from the event's
 *  timeStamp to the end of the handler (results rendered and laid out) and to the next frame. */
const keyLatency = [];
export function keyReport(reset) {
  const out = keyLatency.length ? { n: keyLatency.length, handler_done_ms: stats(keyLatency.map((k) => k.done)), next_frame_ms: stats(keyLatency.map((k) => k.frame)) } : { n: 0 };
  if (reset) keyLatency.length = 0;
  return out;
}

export async function bench({ samples = 200 } = {}) {
  closePalette(false);
  const out = [];
  const queries = ["jn 3:16", "move tab to column 3", "bench 7", "@rom 8", "theme dark", "new col"];
  const keystrokes = queries.flatMap((q) => [...q].map((_, i) => q.slice(0, i + 1)));
  field.focus();
  out.push(await measure("palette keystroke (passage, commands, tabs)", Math.max(samples, keystrokes.length), (i) => {
    field.value = keystrokes[i % keystrokes.length];
    field.dispatchEvent(new Event("input"));
  }));
  closePalette(false);
  field.blur();
  out.push(await measure("palette keystroke with the text section (#)", samples, (i) => {
    const q = ["#l", "#lo", "#lov", "#love", "#world", "#θεὸς"][i % 6];
    field.value = q;
    field.dispatchEvent(new Event("input"));
  }));
  closePalette(false);
  out.push(await measure("tab switch within a column (tab.next)", samples, () => run({ command_id: "tab.next" })));
  const n = activeWorkspace().columns.length;
  out.push(await measure("column focus (column.focus N)", samples, (i) => run({ command_id: "column.focus", args: { column: (i % n) + 1 } })));
  out.push(await measure(`workspace switch (workspace.next, ${host.session.workspaces.length} workspaces)`, samples, () => run({ command_id: "workspace.next" })));
  const tools = TOOLS.map(([id]) => `tool.${id}`);
  out.push(await measure("tool change (tool.*)", samples, (i) => run({ command_id: tools[i % tools.length] })));
  run({ command_id: "tool.select" });
  out.push(await measure("open a chapter in a tab, warm (nav.navigate)", samples, (i) => run({ command_id: "nav.navigate", args: { reference: ["John.3", "Gen.1", "John.3.16", "Gen.1.2"][i % 4] } })));
  return { user_agent: navigator.userAgent, dpr: window.devicePixelRatio, viewport: [innerWidth, innerHeight], workspaces: host.session.workspaces.length, tabs: host.session.workspaces.reduce((a, w) => a + w.columns.reduce((b, c) => b + c.tabs.length, 0), 0), rows: out };
}

// MARK: - Boot

/** A compact picture of what is on screen, for the tests. */
export function snapshot() {
  const ws = activeWorkspace();
  return {
    workspace: ws.title, workspaces: host.session.workspaces.map((w) => w.title), tool: host.session.tool, theme: reading().theme,
    size: reading().size, spacing: host.session.reading.line_spacing, swatches: [swatchName(host.session.ink.swatch) || host.session.ink.swatch, swatchName(host.session.ink.alternate_swatch) || host.session.ink.alternate_swatch], width: host.session.ink.width,
    widths: activeWorkspace().columns.map((c) => +c.width_fraction.toFixed(4)), anchors: activeWorkspace().columns.map((c) => shownTab(c).scroll_anchor || null), wellCollapsed: host.well.collapsed, focusMode: host.focusMode,
    active: ws.columns.findIndex((c) => c.id === ws.active_column_id) + 1,
    columns: ws.columns.map((c, i) => ({ number: i + 1, tabs: c.tabs.map(tabLabel), shown: tabLabel(shownTab(c)), reference: shownTab(c).reference, link: linkWord(shownTab(c)) })),
    dom: [...document.querySelectorAll("#columns > .column")].map((s) => ({
      number: s.dataset.number, active: s.classList.contains("active"), badge: s.querySelector(".badge").textContent,
      tabs: [...s.querySelectorAll(".tab")].map((t) => t.firstChild.textContent), shown: s.querySelector('.tab[aria-selected="true"]')?.firstChild.textContent,
      identity: s.querySelector(".identity").textContent, text: !!s.querySelector(".measure .text"), placeholder: !!s.querySelector(".placeholder"),
    })),
    highlights: host.highlights.map((h) => ({ swatch: swatchName(h.swatch) || h.swatch, anchors: h.anchors.map((a) => ({ segment_id: a.segment_id, exact: a.quote ? a.quote.exact : "" })) })),
    ink: [...host.ink.entries()].map(([k, v]) => ({ tab: k, strokes: v.strokes.length, erased: v.deleted.size })),
    toast: host.lastToast || null, focus: document.activeElement ? (document.activeElement.id || document.activeElement.className) : null,
    paletteOpen: !results.hidden, log: host.log.slice(-20),
  };
}

async function main() {
  await init();
  host.text = fixtureTextSource(core);
  host.registry = core.shellRegistry();
  host.specs = new Map(host.registry.map((s) => [s.id, s]));
  host.bindings = bindingsFrom(host.registry);
  host.layout = core.layoutConstants();
  // `?theme=` seeds the session's reading theme (for captures); otherwise it follows the system.
  const theme = params.get("theme");
  const session = params.get("session") === "bench" ? benchSession() : seedSession();
  if (TOKENS.themes.includes(theme)) session.reading.theme = theme;
  host.handle = new core.ShellHandle(session);
  host.handle.useFixtureText();
  host.handle.setResourceTitles(host.text.titles());
  host.session = host.handle.session();
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => { if (host.session.reading.theme === "system") render(); });
  buildWell();
  render();
  window.openlogosShell = { core, host, run, render, snapshot, bench, keyReport, queryPalette, focusPalette, closePalette, tokens: TOKENS, activeWorkspace, activeTab };
  window.openlogosReady = true;
}

main().catch((error) => {
  console.error(error);
  window.openlogosError = String(error && error.stack || error);
  document.body.insertAdjacentHTML("beforeend", `<p style="position:fixed;bottom:0;left:0;right:0;padding:12px;background:#fee;color:#900;font:13px system-ui">failed: ${escapeHtml(error.message || error)}</p>`);
});
