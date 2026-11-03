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
// The tool well is the core's (B076, specs/22 §4.1): `toolWell(context)` for what has focus (a
// column's tab or the notes dock) and whether text is selected, redrawn on every change of
// either; the host maps its icon names to SVG and performs the note and selection actions.
//
// Host view state, outside the session (docs/evidence/B072-web.md, B076-web.md): the tool well's position
// and collapse, a column-rule drag until it ends in `column.resize`, and highlights and ink
// strokes (in memory until the annotation store, B074), and the notes as edited (B074 too).
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
import { openContextMenu, closeContextMenu, isOpen as contextMenuOpen, openMenuElement } from "./context-menu.js";

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

// Inline SVG for the tool well (20 × 20, stroked with currentColor), keyed by the core's neutral
// icon names (specs/22 §4.1). The Mac app maps the same names to SF Symbols of the same meaning;
// the table is in docs/evidence/B076-web.md.
const glyph = (x, size, text, extra = "", y = 15.5) => `<text x="${x}" y="${y}" font-size="${size}" text-anchor="middle" fill="currentColor" stroke="none" ${extra} font-family="system-ui, -apple-system, sans-serif">${text}</text>`;
const ICONS = {
  select: '<path d="M5.5 3.5v12l3.2-3 2.3 5 2-1-2.3-4.8h4.3z" fill="currentColor" stroke="none"/>',
  "word-study": '<path d="M5.8 3.5h9.7v13H5.8A1.8 1.8 0 0 1 4 14.7V5.3a1.8 1.8 0 0 1 1.8-1.8z"/><path d="M4 14.7a1.8 1.8 0 0 1 1.8-1.8h9.7"/>' + glyph(10, 9, "α", 'font-weight="600"', 10.5),
  highlight: '<path d="M7 13.5l-2.5 3h5l1-1.2M7 13.5l1.8-4.2 6-6 2.4 2.4-6 6z"/><path d="M3 18h14" opacity=".5"/>',
  pen: '<path d="M4 16l.9-3.6L13 4.3l2.7 2.7-8.1 8.1z"/><path d="M11.5 5.8l2.7 2.7"/>',
  "highlighter-pen": '<path d="M10.5 9.5l5.5-5.5 1 1-5.5 5.5"/><path d="M4 16.5c.3-2.6 1.4-4.6 3.6-4.6 1.4 0 2.2 1 2.1 2.2-.2 1.9-2.5 2.4-5.7 2.4z" fill="currentColor" fill-opacity=".35"/>',
  eraser: '<path d="M7.5 16.5h9"/><path d="M3.8 12.2l7.4-7.4 5 5-6.6 6.7H7.6z"/><path d="M7.4 8.6l5 5"/>',
  collect: '<path d="M10 3v8.5M6.8 8.5l3.2 3.2 3.2-3.2"/><path d="M3.5 12v4.5h13V12"/>',
  type: '<path d="M3 15.5l3.6-10 3.6 10M4.4 12h4.4"/><path d="M14.5 4v12M12.8 4h3.4M12.8 16h3.4"/>',
  bold: glyph(10, 15, "B", 'font-weight="700"'),
  italic: glyph(10, 15, "I", 'font-style="italic" font-family="ui-serif, Georgia, serif"'),
  heading: glyph(7.5, 14, "A", 'font-weight="600"') + glyph(15.5, 9, "A", 'font-weight="600"'),
  list: '<circle cx="4.5" cy="5.5" r="1" fill="currentColor" stroke="none"/><circle cx="4.5" cy="10" r="1" fill="currentColor" stroke="none"/><circle cx="4.5" cy="14.5" r="1" fill="currentColor" stroke="none"/><path d="M8 5.5h8.5M8 10h8.5M8 14.5h8.5"/>',
  "link-passage": '<path d="M8.6 11.4a3 3 0 0 0 4.2 0l2.6-2.6a3 3 0 0 0-4.2-4.2l-1 1"/><path d="M11.4 8.6a3 3 0 0 0-4.2 0l-2.6 2.6a3 3 0 0 0 4.2 4.2l1-1"/>',
  "highlight-now": '<path d="M7 13.5l-2.5 3h5l1-1.2M7 13.5l1.8-4.2 6-6 2.4 2.4-6 6z"/><path d="M3 18h14" opacity=".5"/>',
  note: '<path d="M11.5 16.5H5.2a1.2 1.2 0 0 1-1.2-1.2V4.7a1.2 1.2 0 0 1 1.2-1.2h9.6a1.2 1.2 0 0 1 1.2 1.2V10"/><path d="M6.8 7h6.4M6.8 10h4"/><path d="M15 12.5v5M12.5 15h5"/>',
  search: '<circle cx="8.5" cy="8.5" r="4.8"/><path d="M12 12l4.5 4.5"/>',
  "copy-reference": '<rect x="7" y="7" width="9.5" height="9.5" rx="1.5"/><path d="M13 7V4.8a1.3 1.3 0 0 0-1.3-1.3H4.8a1.3 1.3 0 0 0-1.3 1.3v6.9A1.3 1.3 0 0 0 4.8 13H7"/>',
  study: '<path d="M5.8 3.5h9.7v13H5.8A1.8 1.8 0 0 1 4 14.7V5.3a1.8 1.8 0 0 1 1.8-1.8z"/><path d="M4 14.7a1.8 1.8 0 0 1 1.8-1.8h9.7"/><path d="M7.5 6.5h5M7.5 9h5"/>',
  // B077: the context menu's icons (menu::MENU_ICONS); the Mac app maps them to SF Symbols.
  "search-all": '<circle cx="8.5" cy="8.5" r="4.8"/><path d="M12 12l4.5 4.5"/><path d="M3.7 8.5h9.6M8.5 3.7c-1.6 1.4-2.4 3-2.4 4.8s.8 3.4 2.4 4.8c1.6-1.4 2.4-3 2.4-4.8s-.8-3.4-2.4-4.8"/>',
  lemma: '<circle cx="8.5" cy="8.5" r="4.8"/><path d="M12 12l4.5 4.5"/>' + glyph(8.5, 7, "α", 'font-weight="600"', 11),
  find: '<rect x="3" y="3.5" width="14" height="13" rx="1.5"/><circle cx="9" cy="9.5" r="2.8"/><path d="M11 11.5l3 3"/>',
  compare: '<rect x="3" y="4" width="6" height="12" rx="1"/><rect x="11" y="4" width="6" height="12" rx="1"/><path d="M4.8 7h2.4M4.8 9.5h2.4M12.8 7h2.4M12.8 9.5h2.4"/>',
  open: '<path d="M10.5 3.5h6v6M16.5 3.5l-7 7"/><path d="M14 11.5v4a1 1 0 0 1-1 1H4.5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h4"/>',
  column: '<rect x="3.5" y="3.5" width="13" height="13" rx="1.5"/><path d="M10 3.5v13"/>',
  "new-column": '<rect x="3.5" y="3.5" width="8" height="13" rx="1.5"/><path d="M15 7v6M12 10h6"/>',
  "cross-references": '<path d="M5 4.5h7M5 8h4"/><path d="M8 12.5h7M11 16h4"/><path d="M13.5 5.5l2 2-2 2M6.5 10.5l-2 2 2 2"/>',
  copy: '<rect x="7" y="7" width="9.5" height="9.5" rx="1.5"/><path d="M13 7V4.8a1.3 1.3 0 0 0-1.3-1.3H4.8a1.3 1.3 0 0 0-1.3 1.3v6.9A1.3 1.3 0 0 0 4.8 13H7"/>',
  "copy-passage": '<rect x="7" y="7" width="9.5" height="9.5" rx="1.5"/><path d="M13 7V4.8a1.3 1.3 0 0 0-1.3-1.3H4.8a1.3 1.3 0 0 0-1.3 1.3v6.9A1.3 1.3 0 0 0 4.8 13H7"/><path d="M9.3 10h4.9M9.3 12.8h3.2"/>',
  cut: '<circle cx="6" cy="14.5" r="2.2"/><circle cx="14" cy="14.5" r="2.2"/><path d="M7.5 12.8L14 3.5M12.5 12.8L6 3.5"/>',
  paste: '<rect x="4" y="4.5" width="12" height="12.5" rx="1.5"/><rect x="7.2" y="3" width="5.6" height="3" rx="1"/><path d="M7 10h6M7 13h4"/>',
  "remove-link": '<path d="M8.6 11.4a3 3 0 0 0 4.2 0l2.6-2.6a3 3 0 0 0-4.2-4.2l-1 1"/><path d="M11.4 8.6a3 3 0 0 0-4.2 0l-2.6 2.6a3 3 0 0 0 4.2 4.2l1-1"/><path d="M3.5 3.5l13 13"/>',
  close: '<path d="M5.5 5.5l9 9M14.5 5.5l-9 9"/>',
  "close-others": '<rect x="3" y="5" width="6" height="10" rx="1"/><path d="M12 7.5l5 5M17 7.5l-5 5"/>',
  duplicate: '<rect x="7" y="7" width="9.5" height="9.5" rx="1.5"/><path d="M13 7V4.8a1.3 1.3 0 0 0-1.3-1.3H4.8a1.3 1.3 0 0 0-1.3 1.3v6.9A1.3 1.3 0 0 0 4.8 13H7"/><path d="M11.75 9.5v4.5M9.5 11.75h4.5"/>',
  move: '<path d="M3.5 10h12M12 6.5l3.5 3.5-3.5 3.5"/>',
  pin: '<path d="M8 3.5h4l-.5 4.5 2.5 2.5H6L8.5 8z"/><path d="M10 10.5v6"/>',
  follow: '<path d="M4 6.5h9.5M10.5 3.5l3 3-3 3"/><path d="M16 13.5H6.5M9.5 10.5l-3 3 3 3"/>',
  link: '<path d="M8.6 11.4a3 3 0 0 0 4.2 0l2.6-2.6a3 3 0 0 0-4.2-4.2l-1 1"/><path d="M11.4 8.6a3 3 0 0 0-4.2 0l-2.6 2.6a3 3 0 0 0 4.2 4.2l1-1"/>',
  focus: '<path d="M3.5 7V4.5a1 1 0 0 1 1-1H7M13 3.5h2.5a1 1 0 0 1 1 1V7M16.5 13v2.5a1 1 0 0 1-1 1H13M7 16.5H4.5a1 1 0 0 1-1-1V13"/><circle cx="10" cy="10" r="2"/>',
  "move-left": '<path d="M16.5 10h-12M8 6.5L4.5 10 8 13.5"/>',
  "move-right": '<path d="M3.5 10h12M12 6.5l3.5 3.5-3.5 3.5"/>',
  "new-tab": '<path d="M3.5 16.5V6a1 1 0 0 1 1-1h4l1.5 2h5.5a1 1 0 0 1 1 1v8.5z"/><path d="M10 9.5v4.5M7.75 11.75h4.5"/>',
  equalize: '<rect x="3" y="4" width="14" height="12" rx="1.5"/><path d="M7.7 4v12M12.3 4v12"/>',
  "close-column": '<rect x="3.5" y="3.5" width="13" height="13" rx="1.5"/><path d="M7.5 7.5l5 5M12.5 7.5l-5 5"/>',
  "paste-passage": '<rect x="4" y="4.5" width="12" height="12.5" rx="1.5"/><rect x="7.2" y="3" width="5.6" height="3" rx="1"/><path d="M7 11.5h6M10.5 9l2.5 2.5-2.5 2.5"/>',
  "text-size": glyph(7, 15, "A", 'font-family="ui-serif, Georgia, serif"', 15.5) + glyph(14.5, 10, "A", 'font-family="ui-serif, Georgia, serif"', 15.5),
  "text-larger": glyph(8, 15, "A", 'font-family="ui-serif, Georgia, serif"', 15.5) + '<path d="M15 5v5M12.5 7.5h5"/>',
  "text-smaller": glyph(8, 11, "A", 'font-family="ui-serif, Georgia, serif"', 15.5) + '<path d="M12.5 7.5h5"/>',
  "text-reset": glyph(10, 14, "A", 'font-family="ui-serif, Georgia, serif"', 15.5),
  theme: '<circle cx="10" cy="10" r="6.5"/><path d="M10 3.5a6.5 6.5 0 0 1 0 13z" fill="currentColor" stroke="none"/>',
};
const svg = (icon) => `<svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${ICONS[icon] || ""}</svg>`;

const host = {
  handle: null, session: null, registry: [], specs: new Map(), bindings: [], text: null,
  focusMode: false, layout: null,
  views: new Map(), order: [],
  well: { top: null, collapsed: false },
  highlights: [], ink: new Map(),
  palette: { expanded: null, rows: [], index: -1, result: null, returnTo: null },
  menu: null, log: [], targets: new Map(), colourRules: new Set(), labels: new Map(),
  // B076: what has focus for the core's context (`tab`, `notes_dock`; `empty` never arises, a
  // column always has a tab), the last tool well drawn, the notes as edited (HTML per note id,
  // in memory until B074) and the column a passage link navigates.
  focus: "tab", toolWell: null, notes: new Map(), readingColumn: 1,
  lexicon: new Map(), // "word@reference" → the language of the text it was clicked in
};

// MARK: - Commands

function hasSelection() {
  const s = document.getSelection();
  return !!s && s.rangeCount > 0 && !s.isCollapsed && !!viewOfNode(s.getRangeAt(0).commonAncestorContainer);
}

/** What the core takes as context: whether text is selected and what has focus. */
const context = () => ({ has_selection: hasSelection(), focus: host.focus });

/** Dispatches one registered command; the only path from the UI to the session. A quiet
 *  command (tab.record_scroll) changes nothing on screen, so it does not re-render. A context
 *  menu's rows run under the menu's own `context` (B077). */
export function run(invocation, ctx = context()) {
  let outcome;
  try {
    outcome = host.handle.apply(invocation, ctx);
  } catch (error) {
    toast(error.reason_text || error.message);
    host.log.push({ command_id: invocation.command_id, error: error.kind || String(error) });
    return null;
  }
  host.log.push({ command_id: invocation.command_id, handled: outcome.handled });
  const spec = host.specs.get(invocation.command_id);
  if (spec && spec.quiet) { sync(); return outcome; }
  if (outcome.handled) {
    sync();
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
function perform({ command_id: id, args }) {
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
    case "selection.note": noteSelection(); break;
    case "selection.search": searchSelection(); break;
    case "selection.copy_reference": copyReference(); break;
    case "note.bold": noteFormat("bold"); break;
    case "note.italic": noteFormat("italic"); break;
    case "note.heading": noteFormat("heading"); break;
    case "note.list": noteFormat("list"); break;
    case "note.link_passage": linkPassage(args); break;
    case "view.collection_drawer": toast("The collection drawer is not built yet"); break;
    case "view.study_drawer": toast("The study drawer is not built yet"); break;
    case "workspace.export": exportWorkspace(); break;
    // B077: the context menu's host commands (specs/22 §4.2).
    case "search.text": searchText(args); break;
    case "copy.text": copyText(args.text, `Copied “${args.text}”`); break;
    case "copy.reference": copyText(humanPassage(core, args.reference), `Copied ${humanPassage(core, args.reference)}`); break;
    case "copy.passage": copyPassage(args); break;
    case "host.cut": case "host.copy": case "host.paste": clipboardAction(id.slice(5)); break;
    case "nav.paste_passage": pastePassage(); break;
    case "selection.highlight_in": highlightSelection(args.text); break;
    case "note.remove_link": removeLink(args.reference); break;
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

/** Reads the session and the core's labelled view of it (`view()`: each tab's resource title
 *  and chapter_label) after a change. */
function sync() {
  host.session = host.handle.session();
  const labels = new Map();
  for (const ws of host.handle.view().workspaces) {
    for (const column of ws.columns) for (const t of column.tabs) labels.set(`${ws.id}/${t.id}`, t.chapter_label ? `${t.resource_title} ${t.chapter_label}` : t.resource_title);
  }
  host.labels = labels;
}

/** The core's short form of a reference (`Jn 3:16`, `Gen 1:1–2`), or the text as given. */
function short(reference) {
  try { return core.formatShort(reference); } catch { return reference; }
}

/** The strip label from the core: resource title and chapter_label (`BSB Jn 3`), or the title
 *  alone for a tab with no passage (`Note`). */
function tabLabel(tab, ws = activeWorkspace()) {
  return host.labels.get(`${ws.id}/${tab.id}`) || tab.content_id;
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
  const ws = activeWorkspace();
  if (host.focus === "notes_dock" && !ws.notes_dock.open) host.focus = "tab";
  if (activeTab().kind === "reading") host.readingColumn = ws.columns.findIndex((c) => c.id === ws.active_column_id) + 1;
  // The well for what has focus; the tool in effect (not the stored one) drives the text.
  fetchWell();
  if (host.find && !host.find.view.text.isConnected) clearFind();
  root.dataset.theme = r.theme;
  root.dataset.tool = host.toolWell.effective_tool;
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
  const tabs = column.tabs.map((t) => `${t.id}:${t.content_id}:${t.reference}:${t.link_group}:${t.pinned}:${t.lexicon_word || ""}`).join("|");
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
  section.setAttribute("aria-label", `Column ${number}: ${tabLabel(tab, ws)}`);

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
    label.textContent = tabLabel(t, ws);
    const close = document.createElement("button");
    close.type = "button";
    close.className = "close";
    close.textContent = "×";
    close.setAttribute("aria-label", `Close ${tabLabel(t, ws)}`);
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
  if (resource.kind === "notes") {
    const article = document.createElement("article");
    article.className = "measure";
    article.appendChild(noteEditor(tab.content_id, `${resource.name}, column ${number}`));
    page.appendChild(article);
  } else if (resource.kind === "lexicon") {
    page.appendChild(lexiconPage(tab));
  } else if (passageText) {
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
    text.addEventListener("click", (e) => studyWordAt(view, e));
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
    // A right-click focuses nothing: the context menu carries the focus change for its rows.
    if (e.button !== 0 || e.target.closest(".tabstrip, .pane-menu")) return;
    if (host.focus !== "tab") { host.focus = "tab"; render(); }
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
  const noteId = ws.notes_dock.note_id || "fixture:note:1";
  if (dock && dock.ws === ws.id && dock.note === noteId) return dock.el;
  const el = document.createElement("aside");
  el.className = "dock";
  el.setAttribute("aria-label", "Notes dock");
  const h = document.createElement("h2");
  h.textContent = "Notes";
  const scroller = document.createElement("div");
  scroller.className = "scroller";
  const article = document.createElement("article");
  article.className = "measure";
  article.appendChild(noteEditor(noteId, "Notes dock"));
  scroller.appendChild(article);
  el.append(h, scroller);
  dock = { ws: ws.id, note: noteId, el };
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

function highlightSelection(swatch = host.session.ink.swatch) {
  const found = selectionInView();
  if (!found) return toast("Select some text first");
  host.highlights.push({ edition_id: found.view.paragraph.edition_id, anchors: found.trip.anchors, swatch: swatch.toUpperCase() });
  document.getSelection().removeAllRanges();
}

/** The verses a selection spans as an OSIS reference (`John.3.16`, `John.3.16-John.3.17`), or
 *  null when its segments are not verses. */
function selectionPassage(found) {
  const ids = found.trip.anchors.map((a) => a.segment_id);
  const first = ids[0], last = ids[ids.length - 1];
  return /^[A-Za-z0-9]+\.\d+\.\d+$/.test(first) ? (first === last ? first : `${first}-${last}`) : null;
}

function collectSelection() {
  const found = selectionInView();
  if (!found) return toast("Select some text first");
  const passage = selectionPassage(found);
  const where = passage ? short(passage) : found.view.resource.name;
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

/** The core's well for the current context (specs/22 §4.1). */
function fetchWell() {
  const ctx = context();
  host.toolWell = host.handle.toolWell(ctx);
  host.wellSelection = ctx.has_selection;
}

/** The selection group follows the selection: the well alone is redrawn when text becomes
 *  selected or stops being selected. */
export function selectionChanged() {
  if (!host.toolWell || hasSelection() === host.wellSelection) return;
  fetchWell();
  renderWell();
}
document.addEventListener("selectionchange", selectionChanged);

/** Focus moving between a column and the notes dock changes the context; the palette field
 *  and menus keep the context they were opened from. */
document.addEventListener("focusin", (e) => {
  if (!host.session || e.target === field || e.target.closest(".menu, .ctx-menu")) return;
  const focus = e.target.closest(".dock") ? "notes_dock" : "tab";
  if (focus !== host.focus) { host.focus = focus; render(); }
});

// A drag with the Highlight or Collect tool ends in the command, with no menu (specs/22 §4).
document.addEventListener("pointerup", () => {
  const tool = host.toolWell && host.toolWell.effective_tool;
  if (tool !== "highlight" && tool !== "collect") return;
  // After the browser has settled the selection for this pointerup.
  setTimeout(() => { if (hasSelection()) run({ command_id: tool === "highlight" ? "selection.highlight" : "selection.collect" }); }, 0);
});

// MARK: - Selection actions (B076)

/** The selected words as the core quotes them (no verse markers), and where they are. */
function selectedQuote(found) {
  return found.trip.anchors.map((a) => (a.quote ? a.quote.exact : "")).join(" ").trim();
}

/** Note on it: the quote and its reference appended to the open note as a quote paragraph,
 *  opening the dock when no note is open. */
function noteSelection() {
  const found = selectionInView();
  if (!found) return toast("Select some text first");
  const passage = selectionPassage(found);
  let editor = openNoteEditor();
  if (!editor) {
    run({ command_id: "view.notes_dock" });
    editor = openNoteEditor();
  }
  if (!editor) return;
  const quote = document.createElement("blockquote");
  quote.append(`“${selectedQuote(found)}” — `, passage ? passageLink(passage) : found.view.resource.name);
  editor.appendChild(quote);
  noteChanged(editor);
  document.getSelection().removeAllRanges();
}

/** Search this: the selected words in the palette field, the palette open. */
function searchSelection() {
  const found = selectionInView();
  if (!found) return toast("Select some text first");
  focusPalette(selectedQuote(found));
}

/** Copy reference: the selection's passage in the core's human form. */
function copyReference() {
  const found = selectionInView();
  if (!found) return toast("Select some text first");
  const passage = selectionPassage(found);
  const text = passage ? humanPassage(core, passage) : found.view.resource.name;
  host.copied = text;
  if (navigator.clipboard) navigator.clipboard.writeText(text).catch(() => {});
  toast(`Copied ${text}`);
}

// MARK: - Word study (B076): a click on a word with Word study in effect

const LANGUAGES = { grc: "Greek", hbo: "Hebrew", he: "Hebrew", heb: "Hebrew", arc: "Aramaic" };
const wordSegmenter = typeof Intl !== "undefined" && Intl.Segmenter ? new Intl.Segmenter("und", { granularity: "word" }) : null;

/** The word under a point of a column's text, as it appears there, and its verse. */
function wordAt(view, x, y) {
  const caret = document.caretRangeFromPoint ? document.caretRangeFromPoint(x, y) : null;
  return caret ? wordAtCaret(view, caret.startContainer, caret.startOffset) : null;
}

/** The word at a DOM point of a column's text, with its text node and UTF-16 offsets there
 *  (B077 selects it as the context menu opens). */
function wordAtCaret(view, node, offset) {
  if (!node || node.nodeType !== 3 || !node.parentElement.classList.contains("t") || !view.text.contains(node)) return null;
  const verse = node.parentElement.closest("[data-segment]");
  if (!wordSegmenter) return null;
  for (const part of wordSegmenter.segment(node.data)) {
    if (part.index <= offset && offset <= part.index + part.segment.length && part.isWordLike) return { word: part.segment, segment_id: verse.dataset.segment, node, start: part.index, end: part.index + part.segment.length };
  }
  return null;
}

function studyWordAt(view, event) {
  if (host.toolWell.effective_tool !== "word_study" || !document.getSelection().isCollapsed) return;
  const found = wordAt(view, event.clientX, event.clientY);
  if (!found) return;
  // tab.open_lexicon reuses the Lexicon tab in the next column or opens one (a new column when
  // there is none), and focuses it; the word's language is the host's to show.
  host.lexicon.set(`${found.word}@${found.segment_id}`, view.resource.lang);
  run({ command_id: "tab.open_lexicon", args: { text: found.word, reference: found.segment_id } });
}

/** The lexicon tab's page: the word as clicked, its language and passage, and an honest line
 *  that the entries are not there yet (no glosses are made up). */
function lexiconPage(tab) {
  const lang = host.lexicon.get(`${tab.lexicon_word}@${tab.reference}`);
  const article = document.createElement("article");
  article.className = "measure lexicon";
  const word = document.createElement("h2");
  word.className = "word";
  const where = document.createElement("p");
  where.className = "where";
  if (tab.lexicon_word) {
    word.textContent = tab.lexicon_word;
    if (lang) word.lang = lang;
    word.dir = "auto";
    where.textContent = [LANGUAGES[lang], humanPassage(core, tab.reference)].filter(Boolean).join(" · ");
  } else {
    word.textContent = "Lexicon";
    where.textContent = "Choose Word study and click a Greek or Hebrew word.";
  }
  const pending = document.createElement("p");
  pending.className = "pending";
  pending.textContent = "Lexicon entries arrive with the lexicon import.";
  article.append(word, where, pending);
  return article;
}

// MARK: - Notes (B076): an editable note in memory until B074

/** A note's HTML as edited, first the fixture note's text as one paragraph. */
function noteHtml(noteId) {
  if (!host.notes.has(noteId)) {
    const note = host.text.passage(noteId, null);
    const text = note ? note.paragraph.segments.map((s) => s.text).join(note.paragraph.joiner) : "";
    host.notes.set(noteId, text ? `<p>${escapeHtml(text)}</p>` : "<p><br></p>");
  }
  return host.notes.get(noteId);
}

/** A rich-text editor over one note; the notes tab and the dock showing the same note stay in
 *  step. A passage link navigates the reading column when clicked. */
function noteEditor(noteId, label) {
  const el = document.createElement("div");
  el.className = "note-editor";
  el.contentEditable = "true";
  el.spellcheck = false;
  el.dataset.note = noteId;
  el.setAttribute("role", "textbox");
  el.setAttribute("aria-multiline", "true");
  el.setAttribute("aria-label", `Note: ${label}`);
  el.innerHTML = noteHtml(noteId);
  el.addEventListener("input", () => noteChanged(el));
  el.addEventListener("click", (e) => {
    const link = e.target.closest("a.passage-link");
    if (!link) return;
    e.preventDefault();
    host.focus = "tab";
    run({ command_id: "nav.navigate", args: { reference: link.dataset.reference, column: host.readingColumn } });
  });
  return el;
}

function noteChanged(editor) {
  host.notes.set(editor.dataset.note, editor.innerHTML);
  for (const other of document.querySelectorAll(".note-editor")) {
    if (other !== editor && other.dataset.note === editor.dataset.note) other.innerHTML = editor.innerHTML;
  }
}

/** A passage link: the human reference, not editable as text, carrying the OSIS reference. */
function passageLink(reference) {
  const a = document.createElement("a");
  a.className = "passage-link";
  a.href = "#";
  a.contentEditable = "false";
  a.dataset.reference = reference;
  a.textContent = humanPassage(core, reference);
  a.title = `Go to ${a.textContent}`;
  return a;
}

/** The note on screen: the focused editor, else the dock's when it has focus, else a notes tab
 *  shown in the active workspace (the active column's first), else the open dock's. */
function openNoteEditor() {
  const focused = document.activeElement && document.activeElement.closest && document.activeElement.closest(".note-editor");
  if (focused) return focused;
  const editors = [...document.querySelectorAll("#columns .note-editor")];
  if (host.focus === "notes_dock") { const d = editors.find((e) => e.closest(".dock")); if (d) return d; }
  const active = host.views.get(`${activeWorkspace().id}/${activeColumn().id}`);
  return editors.find((e) => active && active.section.contains(e)) || editors.find((e) => e.closest(".column")) || editors.find((e) => e.closest(".dock")) || null;
}

/** Focuses the editor, keeping a caret already in it, else at its end. */
function caretIn(editor) {
  const s = document.getSelection();
  const inside = s.rangeCount > 0 && editor.contains(s.getRangeAt(0).commonAncestorContainer);
  if (document.activeElement !== editor) editor.focus({ preventScroll: true });
  if (!inside) {
    const r = document.createRange();
    r.selectNodeContents(editor);
    r.collapse(false);
    s.removeAllRanges();
    s.addRange(r);
  }
}

/** Bold, italic, heading and list on the note editor (`note.*` come back unhandled). */
function noteFormat(kind) {
  const editor = openNoteEditor();
  if (!editor) return toast("Open a note first");
  caretIn(editor);
  if (kind === "bold" || kind === "italic") document.execCommand(kind);
  else if (kind === "list") document.execCommand("insertUnorderedList");
  else if (kind === "heading") {
    const node = document.getSelection().anchorNode;
    const block = node && (node.nodeType === 1 ? node : node.parentElement).closest("h3, p, li, blockquote, div:not(.note-editor)");
    document.execCommand("formatBlock", false, block && block.tagName === "H3" ? "p" : "h3");
  }
  noteChanged(editor);
}

/** Link passage: the passage at the caret the palette was opened from (or the note's end). */
function linkPassage(args) {
  const reference = args && args.reference;
  if (!reference) { focusPalette("Link passage "); return; }
  const saved = host.noteRange;
  host.noteRange = null;
  const editor = (saved && saved.editor.isConnected && saved.editor) || openNoteEditor();
  if (!editor) return toast("Open a note first");
  editor.focus({ preventScroll: true });
  const s = document.getSelection();
  let range = saved && saved.editor === editor ? saved.range : null;
  if (!range) { range = document.createRange(); range.selectNodeContents(editor.lastElementChild || editor); range.collapse(false); }
  range.deleteContents();
  const link = passageLink(reference);
  const space = document.createTextNode(" ");
  range.insertNode(space);
  range.insertNode(link);
  range.setStartAfter(space);
  range.collapse(true);
  s.removeAllRanges();
  s.addRange(range);
  noteChanged(editor);
}

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
    const tool = host.toolWell.effective_tool;
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

/** The grip and an empty body; the groups are drawn from the core's `toolWell` (renderWell). */
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
  // A press in the well keeps the text selection and the note editor's caret.
  well.addEventListener("pointerdown", (e) => { if (!e.target.closest(".grip")) e.preventDefault(); });
  well.addEventListener("click", (e) => {
    const b = e.target.closest("button[data-command]");
    if (b) pressWellItem(b);
  });
}

/** One well button: `Label (key)` in the tooltip, `Label (key) — reason` when disabled. */
function wellButton(item) {
  const b = document.createElement("button");
  b.type = "button";
  b.dataset.command = item.command_id;
  b.dataset.icon = item.icon;
  if (item.tool) b.dataset.tool = item.tool;
  if (item.key) b.setAttribute("aria-keyshortcuts", item.key);
  b.setAttribute("aria-label", item.label);
  return b;
}

function buildWellGroup(group) {
  const el = document.createElement("div");
  el.className = `group ${group.kind}`;
  el.dataset.kind = group.kind;
  el.setAttribute("role", "group");
  el.setAttribute("aria-label", { tools: "Tools", ink: "Ink", selection: "Selection" }[group.kind]);
  if (group.kind === "ink") {
    // Two overlapping wells: the current one chooses a swatch (tool.swatch), the alternate one
    // swaps them (tool.swap_swatches, X); then the width dot.
    const [swatch, alternate, width] = group.items;
    const swatches = document.createElement("div");
    swatches.className = "swatches";
    const alt = wellButton(alternate), cur = wellButton(swatch);
    alt.className = "alternate";
    cur.className = "current";
    cur.setAttribute("aria-haspopup", "menu");
    swatches.append(alt, cur);
    const dot = wellButton(width);
    dot.className = "width-dot";
    dot.innerHTML = "<span></span>";
    el.append(swatches, dot);
  } else {
    for (const item of group.items) {
      const b = wellButton(item);
      b.className = "tool";
      b.innerHTML = svg(item.icon);
      el.appendChild(b);
    }
  }
  return el;
}

/** A click on a well item: a disabled one says why; the ink items open their choices. */
function pressWellItem(b) {
  const item = host.toolWell.groups.flatMap((g) => g.items).find((i) => i.command_id === b.dataset.command);
  if (!item) return;
  if (item.disabled_text) { toast(item.disabled_text); return; }
  switch (item.command_id) {
    case "tool.swatch": openSwatchMenu(b); return;
    case "tool.width": {
      // The width dot steps up through the core's widths (tool.width_up) and wraps to the
      // narrowest (tool.width) after the widest.
      const state = commandStates().get("tool.width_up");
      run(state && state.disabled ? { command_id: "tool.width", args: { number: 1 } } : { command_id: "tool.width_up" });
      return;
    }
    case "note.link_passage": focusPalette(`${item.label} `); return;
    default: run({ command_id: item.command_id });
  }
}

/** The well as the core decides it for what has focus (specs/22 §4.1): the groups are rebuilt
 *  only when their items change, so a context change swaps buttons in place; the well's top
 *  stays where it is and a selection group grows downward. */
function renderWell() {
  const well = $("well");
  const toolWell = host.toolWell;
  well.classList.toggle("collapsed", host.well.collapsed);
  well.dataset.context = toolWell.context;
  const active = toolWell.groups[0].items.find((i) => i.active);
  well.setAttribute("aria-label", `Tools, current: ${active ? active.label : toolWell.effective_tool}`);
  const signature = toolWell.groups.map((g) => `${g.kind}:${g.items.map((i) => i.command_id).join(",")}`).join("|");
  if (well.dataset.signature !== signature) {
    well.dataset.signature = signature;
    const parts = [];
    toolWell.groups.forEach((g, i) => { if (i > 0) parts.push(document.createElement("hr")); parts.push(buildWellGroup(g)); });
    well.replaceChildren(well.querySelector(".grip"), ...parts);
  }
  for (const item of toolWell.groups.flatMap((g) => g.items)) {
    const b = well.querySelector(`button[data-command="${item.command_id}"]`);
    if (!b) continue;
    const key = shortcutGlyphs(item.key);
    let title = key ? `${item.label} (${key})` : item.label;
    if (item.disabled_text) title = `${title} — ${item.disabled_text}`;
    if (item.command_id === "tool.swatch") title = `Swatch: ${swatchName(item.value) || item.value} (choose)`;
    if (item.command_id === "tool.swap_swatches") title = `Alternate: ${swatchName(item.value) || item.value} (${key} swaps)`;
    if (item.command_id === "tool.width") title = `Brush width ${item.value}`;
    if (b.title !== title) b.title = title;
    if (item.tool) {
      if (b.getAttribute("aria-pressed") !== String(item.active)) b.setAttribute("aria-pressed", String(item.active));
    }
    b.toggleAttribute("aria-disabled", !!item.disabled_text);
    if (item.value != null) {
      if (item.icon === "brush-width") {
        b.setAttribute("aria-label", `Brush width ${item.value}. Change`);
        b.firstChild.style.width = b.firstChild.style.height = `${Math.min(18, Number(item.value) + 3)}px`;
      } else {
        b.style.background = item.value;
        const name = swatchName(item.value) || item.value;
        b.setAttribute("aria-label", item.icon === "swatch" ? `Current swatch ${name}. Choose a swatch` : `Alternate swatch ${name}. Swap swatches`);
      }
    }
  }
  placeWell();
}

/** Centred over the columns when first drawn and after a window resize; a change of context
 *  or a selection group never moves the top (the well grows and shrinks downward), unless the
 *  window is too short for it: then it moves up just enough to stay on screen. */
function placeWell() {
  const well = $("well");
  const columns = $("columns").getBoundingClientRect();
  if (host.well.top == null && host.well.autoTop == null) {
    host.well.autoTop = Math.round(columns.top + (columns.height - well.offsetHeight) / 2);
  }
  const top = host.well.top == null ? host.well.autoTop : host.well.top;
  well.style.top = `${Math.max(columns.top + 8, Math.min(top, columns.bottom - 8 - well.offsetHeight))}px`;
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
  return new Map(host.handle.commands(context()).map((s) => [s.spec.id, s]));
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

// MARK: - Context menu (B077, specs/22 §4.2): the core's menu for what was right-clicked

const VERSE = /^[A-Za-z0-9]+\.\d+\.\d+$/;
const viewOfSection = (section) => [...host.views.values()].find((v) => v.section === section) || null;
const activeView = () => host.views.get(`${activeWorkspace().id}/${activeColumn().id}`) || null;

/** The tab a note editor belongs to: its column's Notes tab, or none for the dock. */
function notesTabOf(editor) {
  const view = viewOfSection(editor.closest("#columns > .column"));
  return view ? view.tab.id : null;
}

function pointInSelection(x, y) {
  const s = document.getSelection();
  return s.rangeCount > 0 && [...s.getRangeAt(0).getClientRects()].some((r) => x >= r.left && x <= r.right && y >= r.top && y <= r.bottom);
}

function selectionTarget(found) {
  const ids = found.trip.anchors.map((a) => a.segment_id);
  const first = ids[0], last = ids[ids.length - 1];
  return {
    kind: "selection", text: selectedQuote(found), reference: VERSE.test(first) ? first : null,
    end_reference: VERSE.test(last) && last !== first ? last : null, resource_id: found.view.tab.content_id, tab_id: found.view.tab.id,
  };
}

function wordTarget(view, word) {
  const start = [...word.node.data.slice(0, word.start)].length;
  return {
    kind: "word", text: word.word, reference: VERSE.test(word.segment_id) ? word.segment_id : null, segment_id: word.segment_id,
    scalar_start: start, scalar_end: start + [...word.word].length, resource_id: view.tab.content_id, tab_id: view.tab.id,
  };
}

/** What a right-click at (x, y) on `el` is on, as the core's ContextTarget; null leaves the
 *  platform menu (the palette field, the tool well, the shell's own menus). Without a point
 *  (the keyboard) a word comes from `caret`. */
function targetAt(el, x, y, caret = null) {
  if (!el || !el.closest || el.closest(".topbar, .well, .menu, .ctx-menu, .toast")) return null;
  const columnEl = el.closest("#columns > .column");
  const number = columnEl ? Number(columnEl.dataset.number) : null;
  const tabEl = el.closest(".tabstrip .tab");
  if (tabEl) return { target: { kind: "tab", tab_id: tabEl.dataset.tabId }, el: tabEl };
  if (el.closest(".identity")) return { target: { kind: "column", column: number }, el: el.closest(".identity") };
  const editor = el.closest(".note-editor");
  if (editor) {
    const tab_id = notesTabOf(editor);
    const link = el.closest("a.passage-link");
    if (link) return { target: { kind: "note_link", reference: link.dataset.reference, tab_id }, editor, link, el: link };
    const s = document.getSelection();
    const text = s.rangeCount > 0 && !s.isCollapsed && editor.contains(s.getRangeAt(0).commonAncestorContainer) ? s.toString().trim() : "";
    return { target: { kind: "note_text", text: text || null, tab_id }, editor, el: editor };
  }
  const dockEl = el.closest(".dock");
  if (dockEl) return { target: { kind: "note_text", text: null, tab_id: null }, editor: dockEl.querySelector(".note-editor"), el: dockEl };
  if (!columnEl) return null;
  const view = viewOfSection(columnEl);
  if (view && view.text && view.text.contains(el)) {
    const found = selectionInView();
    if (found && found.view === view && (x == null || pointInSelection(x, y))) return { target: selectionTarget(found), view, el };
    const word = x != null ? wordAt(view, x, y) : caret ? wordAtCaret(view, caret.node, caret.offset) : null;
    if (word) return { target: wordTarget(view, word), view, word, el };
  }
  return { target: { kind: "background", column: number }, el: columnEl };
}

/** Selects a right-clicked word, as macOS does: the menu's Mark rows act on it. */
function selectWord(word) {
  const r = document.createRange();
  r.setStart(word.node, word.start);
  r.setEnd(word.node, word.end);
  const s = document.getSelection();
  s.removeAllRanges();
  s.addRange(r);
}

function restoreRange(range) {
  if (!range) return;
  const s = document.getSelection();
  s.removeAllRanges();
  s.addRange(range);
}

/** Asks the core for the menu (opening it changes nothing in the session) and draws it. */
export function showContextMenu(hit, x, y, keyboard = false) {
  if (hit.word) selectWord(hit.word);
  let menu;
  try {
    menu = host.handle.contextMenu(hit.target, context());
  } catch (error) {
    toast(error.message || String(error));
    return null;
  }
  const s = document.getSelection();
  host.ctx = { menu, hit, range: s.rangeCount > 0 ? s.getRangeAt(0).cloneRange() : null, returnTo: document.activeElement };
  host.lastTarget = hit.target;
  openContextMenu(menu, {
    x, y, keyboard, icon: svg, glyphs: shortcutGlyphs,
    onChoose: chooseContextItem,
    onClose: (restore) => {
      if (!restore || !host.ctx) return;
      const { returnTo, range } = host.ctx;
      if (returnTo && returnTo.isConnected && returnTo !== document.body) returnTo.focus({ preventScroll: true });
      restoreRange(range);
    },
  });
  return menu;
}

/** A chosen row: the selection and note caret the menu was opened on are put back (keyboard
 *  navigation moved focus into the menu), the menu's focus change runs, then the row, both
 *  under the menu's context. */
function chooseContextItem(item) {
  const { menu, hit, range } = host.ctx;
  if (hit.editor && hit.editor.isConnected) hit.editor.focus({ preventScroll: true });
  restoreRange(range);
  if (menu.focus) run(menu.focus, menu.context);
  run({ command_id: item.command_id, args: item.args }, menu.context);
  host.ctx = null;
}

/** ⇧F10 or the context-menu key: the menu for the selection, else the word at the caret in
 *  reading text or the caret in a note, else the focused tab, else the active column. */
function contextMenuKey(e) {
  if (!(e.key === "ContextMenu" || (e.key === "F10" && e.shiftKey && !e.metaKey && !e.ctrlKey && !e.altKey))) return false;
  if (document.activeElement === field) return false; // the field keeps the platform menu
  e.preventDefault();
  host.keyMenuAt = performance.now();
  const s = document.getSelection();
  const range = s.rangeCount > 0 ? s.getRangeAt(0) : null;
  const active = document.activeElement;
  const startEl = range && (range.startContainer.nodeType === 1 ? range.startContainer : range.startContainer.parentElement);
  let hit = null, rect = null;
  if (range && startEl && (viewOfNode(range.startContainer) || (active && active.closest && active.closest(".note-editor") && active.contains(startEl)))) {
    hit = targetAt(startEl, null, null, { node: range.startContainer, offset: range.startOffset });
    rect = range.getBoundingClientRect();
  }
  if (!hit && active && active.closest && active.closest(".tabstrip .tab, .note-editor")) hit = targetAt(active, null, null);
  if (!hit) {
    const view = activeView();
    hit = view && { target: { kind: "background", column: view.number }, el: view.scroller };
  }
  if (!hit) return true;
  if (hit.word) { selectWord(hit.word); rect = document.getSelection().getRangeAt(0).getBoundingClientRect(); }
  if (!rect || (!rect.width && !rect.height)) {
    const r = hit.el.getBoundingClientRect();
    rect = hit.target.kind === "background" ? { left: r.left + 24, bottom: r.top + 24 } : r;
  }
  showContextMenu(hit, rect.left, rect.bottom + 2, true);
  return true;
}

document.addEventListener("contextmenu", (e) => {
  if (!host.session || e.shiftKey) return; // ⇧-right-click: the platform's own menu
  // The context-menu key's own contextmenu event, after contextMenuKey opened the menu.
  if (performance.now() - (host.keyMenuAt ?? -1e9) < 250) { e.preventDefault(); return; }
  const hit = targetAt(e.target, e.clientX, e.clientY);
  if (!hit) return;
  e.preventDefault();
  showContextMenu(hit, e.clientX, e.clientY);
  if (host.recordMenu) {
    // From the trusted event's timeStamp to the menu laid out, and to the next frame.
    void document.body.offsetHeight;
    const sample = { done: performance.now() - e.timeStamp };
    menuLatency.push(sample);
    requestAnimationFrame(() => { sample.frame = performance.now() - e.timeStamp; });
  }
});
// The baseline for the same trusted right-click: its pointerdown (no work) to the next frame,
// so the menu's own cost can be told from where the browser delivers the event in the frame.
document.addEventListener("pointerdown", (e) => {
  if (!host.recordMenu || e.button !== 2) return;
  const sample = {};
  menuBaseline.push(sample);
  requestAnimationFrame(() => { sample.frame = performance.now() - e.timeStamp; });
}, true);
const menuLatency = [], menuBaseline = [];
export function menuReport(reset) {
  const frames = (list) => stats(list.filter((k) => k.frame != null).map((k) => k.frame));
  const out = menuLatency.length ? { n: menuLatency.length, handler_done_ms: stats(menuLatency.map((k) => k.done)), next_frame_ms: frames(menuLatency), baseline_pointerdown_next_frame_ms: menuBaseline.length ? frames(menuBaseline) : null } : { n: 0 };
  if (reset) { menuLatency.length = 0; menuBaseline.length = 0; }
  return out;
}

// MARK: - The context menu's host commands (B077)

function copyText(text, message) {
  host.copied = text;
  if (navigator.clipboard) navigator.clipboard.writeText(text).catch(() => {});
  toast(message);
}

/** The text of a verse or verse range of a resource, as the text source has it. */
function verseText(resourceId, reference) {
  const passage = resourceId && reference && host.text.passage(resourceId, reference);
  if (!passage) return "";
  const [first, last = first] = reference.split("-");
  const segments = passage.paragraph.segments;
  const i = segments.findIndex((x) => x.segment_id === first);
  const j = segments.findIndex((x) => x.segment_id === last);
  return i < 0 ? "" : segments.slice(i, Math.max(i, j) + 1).map((x) => x.text).join(" ");
}

/** copy.passage: `“text” — John 3:16 (BSB)`, the shape Note on it quotes in (B076). */
function copyPassage({ text, reference, resource_id: resource }) {
  const where = humanPassage(core, reference);
  const words = text || verseText(resource, reference);
  const edition = resource ? ` (${host.text.resource(resource).short})` : "";
  copyText(`“${words}” — ${where}${edition}`, `Copied ${where} with its text`);
}

/** The clipboard's text; the shell's own last copy when the browser does not grant reading. */
function readClipboard() {
  const read = navigator.clipboard && navigator.clipboard.readText ? navigator.clipboard.readText() : Promise.reject(new Error("no clipboard"));
  return read.then((text) => text || host.copied || "", () => host.copied || "");
}

/** Cut, Copy and Paste on the selection or the note caret the menu was opened on. */
function clipboardAction(kind) {
  const s = document.getSelection();
  const editor = host.ctx && host.ctx.hit.editor;
  if (kind === "paste") {
    const range = s.rangeCount > 0 ? s.getRangeAt(0).cloneRange() : null;
    readClipboard().then((text) => {
      if (!text) return toast("The clipboard is empty");
      if (!editor || !editor.isConnected) return;
      editor.focus({ preventScroll: true });
      restoreRange(range);
      document.execCommand("insertText", false, text);
      noteChanged(editor);
    });
    return;
  }
  const text = s.toString();
  host.copied = text;
  if (!document.execCommand(kind)) {
    if (navigator.clipboard) navigator.clipboard.writeText(text).catch(() => {});
    if (kind === "cut" && editor) document.execCommand("delete");
  }
  if (editor) noteChanged(editor);
}

/** A passage in pasted text: all of it, else the reference copy.passage writes after the
 *  dash, else the first run that reads as a reference. */
function passageIn(text) {
  const tries = [text.trim()];
  const dash = /—\s*([^—(]+?)\s*(\(|$)/.exec(text);
  if (dash) tries.push(dash[1]);
  for (const m of text.matchAll(/(?:[1-3] ?)?[A-Z][a-z]+\.? \d+(?::\d+(?:[–-]\d+)?)?/g)) tries.push(m[0]);
  for (const t of tries) {
    try { return core.formatOsis(core.parseReference(t)); } catch { /* not a passage */ }
  }
  return null;
}

function pastePassage() {
  readClipboard().then((text) => {
    const reference = passageIn(text);
    if (!reference) return toast("The clipboard has no passage");
    run({ command_id: "nav.navigate", args: { reference } });
  });
}

/** Remove link: the passage link becomes its words. */
function removeLink(reference) {
  const hit = host.ctx && host.ctx.hit;
  const link = hit && hit.link && hit.link.isConnected ? hit.link : document.querySelector(`.note-editor a.passage-link[data-reference="${CSS.escape(reference)}"]`);
  if (!link) return;
  const editor = link.closest(".note-editor");
  link.replaceWith(document.createTextNode(link.textContent));
  noteChanged(editor);
}

const SCOPE_LABELS = { all: "All editions", workspace: "This workspace" };

/** search.text: the palette's Text section with the words and a scope chip; the chapter scope
 *  finds in the column instead. */
function searchText({ text, scope, resource_id: resource, reference }) {
  if (scope === "chapter") { findInChapter(text, resource, reference); return; }
  host.palette.scope = { scope: scope || "all", resource: resource || null };
  focusPalette(`#${text}`);
}

/** The resources a scoped search keeps (null: all). */
function scopeResources() {
  const scope = host.palette.scope;
  if (!scope || scope.scope === "all") return null;
  if (scope.scope === "edition") return new Set([scope.resource]);
  return new Set(activeWorkspace().columns.flatMap((c) => c.tabs.map((t) => t.content_id)));
}

function renderScopeChip() {
  const chip = $("scope-chip");
  const scope = host.palette.scope;
  chip.hidden = !scope;
  field.classList.toggle("scoped", !!scope);
  if (!scope) return;
  const label = scope.scope === "edition" ? `This edition · ${host.text.resource(scope.resource).short}` : SCOPE_LABELS[scope.scope] || scope.scope;
  chip.firstChild.textContent = label;
  chip.title = `Searching ${label.toLowerCase()}`;
}

// MARK: - Find in this chapter (search.text, scope chapter)

function findInChapter(text, resource, reference) {
  clearFind();
  const views = [...host.views.values()].filter((v) => v.text && v.section.isConnected && v.tab.content_id === resource);
  const view = views.find((v) => v === activeView()) || views[0];
  const where = humanPassage(core, reference);
  if (!view) return toast(`${where} is not open`);
  document.getSelection().removeAllRanges();
  const needle = text.toLocaleLowerCase();
  const ranges = [];
  for (const t of view.text.querySelectorAll(".t")) {
    const node = t.firstChild;
    if (!node || !needle) continue;
    const hay = node.data.toLocaleLowerCase();
    if (hay.length !== node.data.length) continue; // lower-casing changed the offsets
    for (let i = hay.indexOf(needle); i >= 0; i = hay.indexOf(needle, i + needle.length)) {
      const r = document.createRange();
      r.setStart(node, i);
      r.setEnd(node, i + needle.length);
      ranges.push(r);
    }
  }
  if (!ranges.length) return toast(`No matches for “${text}” in ${where}`);
  host.find = { view, ranges, index: 0, text, where };
  showFind();
  toast(ranges.length === 1 ? `1 match in ${where}` : `${ranges.length} matches in ${where}`);
}

function showFind() {
  const { view, ranges, index } = host.find;
  if (!("highlights" in CSS)) return;
  CSS.highlights.set("ol-find", new Highlight(...ranges));
  CSS.highlights.set("ol-find-current", new Highlight(ranges[index]));
  const r = ranges[index].getBoundingClientRect(), box = view.scroller.getBoundingClientRect();
  if (r.top < box.top || r.bottom > box.bottom) view.scroller.scrollTop += r.top - box.top - box.height / 3;
}

function clearFind() {
  host.find = null;
  if ("highlights" in CSS) { CSS.highlights.delete("ol-find"); CSS.highlights.delete("ol-find-current"); }
}

/** While matches are shown: ⌘G / ⇧⌘G next and previous, Esc clears (host view keys, like Esc
 *  closing the palette; not registered commands). */
function findKey(e) {
  if (!host.find) return false;
  if (e.key === "Escape" && !host.menu && results.hidden) { e.preventDefault(); clearFind(); return true; }
  if (e.code !== "KeyG" || !(e.metaKey || e.ctrlKey) || e.altKey) return false;
  e.preventDefault();
  const f = host.find;
  f.index = (f.index + (e.shiftKey ? -1 : 1) + f.ranges.length) % f.ranges.length;
  showFind();
  toast(`${f.index + 1} of ${f.ranges.length} in ${f.where}`);
  return true;
}

// MARK: - The palette

const field = $("palette-field");
const results = $("palette-results");

function focusPalette(text) {
  if (document.activeElement !== field) {
    host.palette.returnTo = document.activeElement;
    // Link passage inserts at the caret the palette was opened from.
    const s = document.getSelection();
    const editor = s.rangeCount > 0 && s.anchorNode && (s.anchorNode.nodeType === 1 ? s.anchorNode : s.anchorNode.parentElement).closest(".note-editor");
    host.noteRange = editor ? { editor, range: s.getRangeAt(0).cloneRange() } : null;
  }
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
  host.palette.scope = null;
  renderScopeChip();
  if (restore) {
    const back = host.palette.returnTo;
    host.palette.returnTo = null;
    field.blur();
    const view = host.views.get(`${activeWorkspace().id}/${activeColumn().id}`);
    const inPlace = back && back !== document.body && back.isConnected && ((view && view.section.contains(back)) || (host.focus === "notes_dock" && back.closest(".dock")));
    const target = inPlace ? back : view && view.scroller;
    if (target) target.focus({ preventScroll: true });
  }
}

/** Every keystroke asks the core; nothing is debounced (specs/22 §7: one frame). */
export function queryPalette() {
  const text = field.value;
  // A scoped search (B077) asks for every text row and keeps those of the scope's editions.
  const expanded = host.palette.expanded || (host.palette.scope ? "text" : null);
  const result = host.handle.paletteQuery(text, { platform: "web", ...context(), expanded });
  const keep = scopeResources();
  if (keep) {
    const shorts = [...keep].map((id) => `${host.text.resource(id).short} · `);
    for (const group of result.groups) {
      if (group.section !== "text") continue;
      group.items = group.items.filter((item) => shorts.some((p) => item.title.startsWith(p)));
      group.total = group.items.length;
    }
  }
  host.palette.result = result;
  renderScopeChip();
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
  // The context menu takes every key while it is open (type-ahead letters are not tool keys).
  if (contextMenuOpen()) return;
  if (contextMenuKey(e)) return;
  if (findKey(e)) return;
  if (e.key === "Escape" && host.menu) { e.preventDefault(); closeMenu(true); return; }
  const typing = inTextField(document.activeElement);
  const invocation = match(host.bindings, e, typing);
  if (!invocation) return;
  // ⌘B and ⌘I are the note editor's, only while it has focus (specs/22 §6).
  if (invocation.command_id.startsWith("note.") && !document.activeElement.closest(".note-editor")) return;
  // In the field, ⌘Z and the history keys are the field's own.
  if (typing && ["edit.undo", "edit.redo", "nav.back", "nav.forward"].includes(invocation.command_id)) return;
  e.preventDefault();
  if (typing && invocation.command_id !== "palette.focus") closePalette(false);
  run(invocation);
}, true);

$("workspace-switcher").addEventListener("click", openSwitcher);
$("text-menu").addEventListener("click", openTextMenu);
window.addEventListener("resize", () => { host.well.autoTop = null; placeWell(); for (const v of host.views.values()) if (v.section.isConnected) sizeCanvas(v); });

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
async function measure(name, samples, action, prepare = null) {
  const work = [], frame = [];
  for (let i = 0; i < samples; i++) {
    if (prepare) prepare(i);
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
  const tools = ["select", "highlight", "pen", "highlighter_pen", "eraser", "collect"].map((id) => `tool.${id}`);
  out.push(await measure("tool change (tool.*)", samples, (i) => run({ command_id: tools[i % tools.length] })));
  run({ command_id: "tool.select" });
  // B076: the well changes with the context. Columns by what they show (the seed has a
  // translation, an original and a notes column; the bench session only translations).
  const columns = activeWorkspace().columns.map((c, i) => ({ n: i + 1, tab: shownTab(c) }));
  const translation = columns.find((c) => c.tab.kind === "reading" && !["grc", "he", "hbo"].includes(host.text.resource(c.tab.content_id).lang));
  const original = columns.find((c) => c.tab.kind === "reading" && ["grc", "he", "hbo"].includes(host.text.resource(c.tab.content_id).lang));
  const notes = columns.find((c) => c.tab.kind === "notes");
  if (translation && original) {
    out.push(await measure("B076 column focus translation ↔ original (well changes)", samples, (i) => run({ command_id: "column.focus", args: { column: i % 2 ? original.n : translation.n } })));
  }
  if (translation && notes) {
    out.push(await measure("B076 translation → notes (well changes)", samples, (i) => run({ command_id: "column.focus", args: { column: i % 2 ? notes.n : translation.n } })));
  }
  if (translation) {
    run({ command_id: "column.focus", args: { column: translation.n } });
    const view = host.views.get(`${activeWorkspace().id}/${activeColumn().id}`);
    const t = view.text.querySelector(".t").firstChild;
    out.push(await measure("B076 selection appears → selection group drawn", samples, (i) => {
      const sel = document.getSelection();
      if (i % 2) { sel.removeAllRanges(); } else { const r = document.createRange(); r.setStart(t, 4); r.setEnd(t, 12); sel.removeAllRanges(); sel.addRange(r); }
      selectionChanged();
    }));
    document.getSelection().removeAllRanges();
    selectionChanged();
  }
  // B077: the context menu. Opening is the core's menu plus drawing it; choosing is the focus
  // change and the row with its effect (the menu is opened outside the timed part).
  const first = activeWorkspace().columns.findIndex((c) => shownTab(c).kind === "reading" && host.views.get(`${activeWorkspace().id}/${c.id}`).text) + 1;
  if (first > 0) {
    run({ command_id: "column.focus", args: { column: first } });
    const view = activeView();
    const node = view.text.querySelector(".t").firstChild;
    const word = wordAtCaret(view, node, 1);
    const tabEl = view.section.querySelector(".tab");
    const hits = {
      word: () => ({ target: wordTarget(view, word), view, word, el: view.text }),
      tab: () => targetAt(tabEl, null, null),
      background: () => ({ target: { kind: "background", column: first }, el: view.scroller }),
    };
    for (const [kind, hit] of Object.entries(hits)) {
      out.push(await measure(`B077 context menu opens (${kind}: core menu + drawn)`, samples, () => showContextMenu(hit(), 300, 200), () => closeContextMenu(false)));
    }
    closeContextMenu(false);
    const row = (menu, id, scope) => menu.sections.flatMap((x) => x.items).find((i) => i.command_id === id && (!scope || i.args.scope === scope));
    const compare = row(host.handle.contextMenu(hits.word().target, context()), "tab.open_edition");
    if (compare) {
      out.push(await measure(`B077 choose → effect: ${compare.label} (tab.open_edition)`, samples, () => {
        chooseContextItem(row(host.ctx.menu, "tab.open_edition"));
      }, () => { closePalette(false); run({ command_id: "column.focus", args: { column: first } }); showContextMenu(hits.word(), 300, 200); closeContextMenu(false); }));
    }
    out.push(await measure("B077 choose → effect: Search in all editions (palette with results)", samples, () => {
      chooseContextItem(row(host.ctx.menu, "search.text", "all"));
    }, () => { closePalette(false); showContextMenu(hits.word(), 300, 200); closeContextMenu(false); }));
    closePalette(false);
    document.getSelection().removeAllRanges();
  }
  out.push(await measure("open a chapter in a tab, warm (nav.navigate)", samples, (i) => run({ command_id: "nav.navigate", args: { reference: ["John.3", "Gen.1", "John.3.16", "Gen.1.2"][i % 4] } })));
  return { user_agent: navigator.userAgent, dpr: window.devicePixelRatio, viewport: [innerWidth, innerHeight], workspaces: host.session.workspaces.length, tabs: host.session.workspaces.reduce((a, w) => a + w.columns.reduce((b, c) => b + c.tabs.length, 0), 0), rows: out };
}

// MARK: - Boot

function contextMenuSnapshot() {
  const el = openMenuElement();
  if (!el) return null;
  return {
    focused: document.activeElement && document.activeElement.closest(".ctx-row") ? document.activeElement.querySelector(".ctx-label > span").textContent : null,
    menus: [...el.querySelectorAll(".ctx-menu")].map((m) => {
      const r = m.getBoundingClientRect();
      return {
        label: m.getAttribute("aria-label"), title: m.querySelector(".ctx-title")?.textContent || null,
        sections: m.querySelectorAll(":scope > [role=group]").length, rect: [r.left, r.top, r.right, r.bottom],
        rows: [...m.querySelectorAll(".ctx-row")].map((row) => ({
          label: row.querySelector(".ctx-label > span").textContent, command: row.dataset.command, role: row.getAttribute("role"),
          disabled: row.querySelector(".ctx-why")?.textContent || null, end: row.querySelector(".ctx-end").textContent,
          submenu: row.hasAttribute("aria-haspopup"), checked: row.getAttribute("aria-checked"), swatch: !!row.querySelector(".ctx-swatch"),
        })),
      };
    }),
  };
}

/** A compact picture of what is on screen, for the tests. */
export function snapshot() {
  const ws = activeWorkspace();
  return {
    workspace: ws.title, workspaces: host.session.workspaces.map((w) => w.title), tool: host.session.tool, theme: reading().theme,
    size: reading().size, spacing: host.session.reading.line_spacing, swatches: [swatchName(host.session.ink.swatch) || host.session.ink.swatch, swatchName(host.session.ink.alternate_swatch) || host.session.ink.alternate_swatch], width: host.session.ink.width,
    widths: activeWorkspace().columns.map((c) => +c.width_fraction.toFixed(4)), anchors: activeWorkspace().columns.map((c) => shownTab(c).scroll_anchor || null), wellCollapsed: host.well.collapsed, focusMode: host.focusMode,
    active: ws.columns.findIndex((c) => c.id === ws.active_column_id) + 1,
    columns: ws.columns.map((c, i) => ({ number: i + 1, tabs: c.tabs.map((t) => tabLabel(t, ws)), shown: tabLabel(shownTab(c), ws), reference: shownTab(c).reference, link: linkWord(shownTab(c)) })),
    dom: [...document.querySelectorAll("#columns > .column")].map((s) => ({
      number: s.dataset.number, active: s.classList.contains("active"), badge: s.querySelector(".badge").textContent,
      tabs: [...s.querySelectorAll(".tab")].map((t) => t.firstChild.textContent), shown: s.querySelector('.tab[aria-selected="true"]')?.firstChild.textContent,
      identity: s.querySelector(".identity").textContent, text: !!s.querySelector(".measure .text, .note-editor, .lexicon"), placeholder: !!s.querySelector(".placeholder"),
    })),
    highlights: host.highlights.map((h) => ({ swatch: swatchName(h.swatch) || h.swatch, anchors: h.anchors.map((a) => ({ segment_id: a.segment_id, exact: a.quote ? a.quote.exact : "" })) })),
    ink: [...host.ink.entries()].map(([k, v]) => ({ tab: k, strokes: v.strokes.length, erased: v.deleted.size })),
    toast: host.lastToast || null, focus: document.activeElement ? (document.activeElement.id || document.activeElement.className) : null,
    paletteOpen: !results.hidden, log: host.log.slice(-20),
    // B076: the well as drawn, what has focus, the notes as edited, the last copied reference.
    well: {
      context: $("well").dataset.context, effective: host.toolWell.effective_tool, focus: host.focus,
      groups: [...$("well").querySelectorAll(".group")].map((g) => ({ kind: g.dataset.kind, items: [...g.querySelectorAll("button[data-command]")].map((b) => b.dataset.command) })),
      pressed: [...$("well").querySelectorAll('button[aria-pressed="true"]')].map((b) => b.dataset.command),
      top: $("well").getBoundingClientRect().top,
    },
    notes: Object.fromEntries(host.notes), copied: host.copied || null,
    // B077: the context menu as drawn (every open level), the last target, find and scope.
    contextMenu: contextMenuSnapshot(), target: host.lastTarget || null, selection: document.getSelection().toString(),
    find: host.find ? { count: host.find.ranges.length, index: host.find.index, where: host.find.where, current: host.find.ranges[host.find.index].toString() } : null,
    scope: host.palette.scope ? { ...host.palette.scope, chip: $("scope-chip").firstChild.textContent } : null,
    paletteValue: field.value,
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
  host.handle.setResourceLanguages(host.text.languages());
  sync();
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => { if (host.session.reading.theme === "system") render(); });
  buildWell();
  render();
  $("scope-chip").lastChild.addEventListener("click", () => { host.palette.scope = null; field.focus(); queryPalette(); });
  window.openlogosShell = { core, host, run, render, selectionChanged, snapshot, bench, keyReport, menuReport, queryPalette, focusPalette, closePalette, tokens: TOKENS, activeWorkspace, activeTab, showContextMenu, closeContextMenu };
  window.openlogosReady = true;
}

main().catch((error) => {
  console.error(error);
  window.openlogosError = String(error && error.stack || error);
  document.body.insertAdjacentHTML("beforeend", `<p style="position:fixed;bottom:0;left:0;right:0;padding:12px;background:#fee;color:#900;font:13px system-ui">failed: ${escapeHtml(error.message || error)}</p>`);
});
