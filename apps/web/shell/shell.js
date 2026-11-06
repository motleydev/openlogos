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
// and collapse, a column-rule drag until it ends in `column.resize`, a tab drag until it ends
// in `tab.move`. Notes, highlights and ink are the core's (B074, specs/24): the page converts
// at the edge, and saves the core's user store to IndexedDB (store.js) and restores it at
// launch.
//
// Rendering is per column: a column's DOM is rebuilt only when its tabs or shown tab change or
// its tab is navigated; focus, tool and theme changes touch classes and attributes only, so
// they cost a style pass, and a drag in the text is never cut short by a re-render. Hidden tabs
// and other workspaces keep no DOM (specs/22 §7: they release their rendered text).
//
// A reading tab is one continuous scroll through its edition (B078, specs/23; reading.js): the
// reading handle's chapter blocks, a window of them around the reading line. Scrolling moves the
// tab's passage quietly (`tab.record_scroll`) and carries linked tabs along; labels change in
// place and the column is not rebuilt for it.
import init, * as core from "../core-host/pkg/openlogos_ffi.js";
import { selectionAnchors } from "../reader/surface.js";
import { BRUSHES, HIGHLIGHTER_WIDTH_FACTOR, TouchArbiter, compositeAt, strokeOutline, distanceToSegment } from "../ink/layer.js";
import { fixtureTextSource } from "./text-source.js";
import { seedSession, benchSession } from "./seed.js";
import { bindingsFrom, match } from "./keys.js";
import { humanPassage, shortcutGlyphs, linkWord } from "./format.js";
import { openContextMenu, closeContextMenu, isOpen as contextMenuOpen, openMenuElement } from "./context-menu.js";
import { ReadingStream, PLACEHOLDER_NOTICE, PLACEHOLDER_REASON, chapterOfSegment } from "./reading.js";
import { Chunks, installPacks, packBase } from "./pack.js";
import { TextSearch, wantsText } from "./search.js";
import { UserStore, refusedNotice, issuesNotice, WRITE_FAILED_NOTICE } from "./store.js";
import { renderNote, noteFromEditor } from "./note-form.js";

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
/** The root classes the tool in effect sets (shell.css); no style rule reads `data-tool`. */
const TOOL_CLASSES = [["tool-ink", ["pen", "highlighter_pen", "eraser"]], ["tool-eraser", ["eraser"]], ["tool-highlight", ["highlight"]], ["tool-word-study", ["word_study"]]];

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
  // B074 (specs/24): the core holds the notes, highlights and ink; the host keeps what it last
  // fetched for each rendered block (keyed by a version bumped on every change) and the eraser's
  // count for the tests.
  hlVersion: 0, hlCache: new Map(), inkCache: new Map(), erased: new Map(),
  palette: { expanded: null, rows: [], index: -1, result: null, returnTo: null },
  menu: null, log: [], colourRules: new Set(), labels: new Map(),
  // B076: what has focus for the core's context (`tab`, `notes_dock`; `empty` never arises, a
  // column always has a tab), the last tool well drawn and the column a passage link navigates.
  focus: "tab", toolWell: null, readingColumn: 1,
  // B074: the user store (IndexedDB; null when `?store=off`), the change count last saved, the
  // pending save, consecutive failed writes, and what the launch restore did and cost.
  store: null, lastSaved: 0, saveTimer: 0, saveFailures: 0, saves: [], restore: null,
  // B078 (specs/23): the reading handle (placeholder editions until B071's packs), and per tab
  // (`ws/tab`) the passage its stream was last put at or reported (a reference that differs is
  // a navigation: the column is rebuilt there), a navigation serial in the column signature,
  // and the tabs whose next build flashes the typed verses.
  reading: null, editions: new Set(), known: new Map(), navSerial: new Map(), flash: new Set(),
  frames: null,
  // B071: the preview pack's chunks, the search worker, how stored highlights on another
  // revision fared.
  chunks: null, search: null, reanchors: {},
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
  changed();
  host.log.push(invocation.command_id === "tab.record_scroll"
    ? { command_id: invocation.command_id, handled: outcome.handled, tab_id: invocation.args.tab_id, segment_id: invocation.args.segment_id }
    : { command_id: invocation.command_id, handled: outcome.handled });
  const spec = host.specs.get(invocation.command_id);
  if (spec && spec.quiet) { sync(); followPositions(outcome); return outcome; }
  if (outcome.handled) {
    sync();
    followPositions(outcome);
  } else {
    perform(invocation);
  }
  render();
  return outcome;
}

// MARK: - Saving (B074, specs/24 §4): the core's store, 1 s after the last change and on hide

/** After anything that may change what the store writes: a save is due when the core's change
 *  count moved since the last one, `save_debounce_ms` after the last change. */
function changed() {
  if (!host.store || host.handle.changeCount() === host.lastSaved) return;
  clearTimeout(host.saveTimer);
  host.saveTimer = setTimeout(() => saveNow(), host.saveDebounce);
}

/** Exports on the main thread (under a millisecond, specs/24 §4) and hands the string to
 *  IndexedDB, whose write runs off it. A failed write keeps the previous value and leaves the
 *  save due, so the next change retries; three in a row are said once. */
export function saveNow() {
  clearTimeout(host.saveTimer);
  flushNotes();
  host.saveTimer = 0;
  if (!host.store) return Promise.resolve(false);
  const count = host.handle.changeCount();
  if (count === host.lastSaved) return Promise.resolve(false);
  const t0 = performance.now();
  const json = host.handle.exportStore();
  const exported = performance.now() - t0;
  host.lastSaved = count;
  host.inflight = json;
  const sample = { export_ms: exported, bytes: json.length };
  host.saves.push(sample);
  if (host.saves.length > 1000) host.saves.shift();
  return host.store.write(json).then(() => {
    sample.write_ms = performance.now() - t0;
    host.saveFailures = 0;
    if (host.inflight === json) { host.inflight = null; host.store.clearPending(); }
    return true;
  }, (error) => {
    console.warn("save failed", error);
    if (host.lastSaved === count) host.lastSaved = -1;
    host.saveFailures += 1;
    if (host.saveFailures === 3) notice(WRITE_FAILED_NOTICE);
    return false;
  });
}

/** On hide: save now, and keep the newest store not yet committed in the page's synchronous
 *  storage too. A page closed or reloaded is torn down before IndexedDB commits a transaction
 *  started as it goes (Chrome drops it), so the next launch takes that copy first. */
function saveOnHide() {
  saveNow();
  if (host.store && host.inflight) host.store.keepPending(host.inflight);
}
document.addEventListener("visibilitychange", () => { if (document.visibilityState === "hidden") saveOnHide(); });
window.addEventListener("pagehide", saveOnHide);

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
    case "selection.study": if (!refusePlaceholder(selectionInView())) toast("Word study is not built yet"); break;
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
/** A reading tab whose edition the reading handle has: one continuous scroll (B078). */
const isStream = (tab) => !!tab && tab.kind === "reading" && host.editions.has(tab.content_id);
const sessionTab = (wsId, tabId) => {
  const ws = host.session.workspaces.find((w) => w.id === wsId);
  return ws && ws.columns.flatMap((c) => c.tabs).find((t) => t.id === tabId);
};

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
  for (const [name, tools] of TOOL_CLASSES) root.classList.toggle(name, tools.includes(host.toolWell.effective_tool));
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
  // A reflow (text size, spacing, column widths, focus mode) keeps each stream on the verse it
  // had at the top (B078).
  const layout = `${r.size}|${r.spacing}|${r.measure}|${ws.id}|${ws.columns.map((c) => c.width_fraction.toFixed(4)).join(",")}|${host.focusMode}|${host.well.collapsed}|${ws.notes_dock.open}`;
  if (layout !== host.layoutKey) { host.layoutKey = layout; restoreStreams(); }
  renderWell();
  paintHighlights();
}

function restoreStreams() {
  for (const view of host.views.values()) if (view.stream && view.section.isConnected) { view.stream.restore(); placeBand(view, true); }
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

/** What a column's DOM is built from. A reading stream's passage is not in it (B078): the
 *  passage follows the scroll, and a navigation bumps the tab's serial instead. */
function columnSignature(ws, column, number) {
  const tabs = column.tabs.map((t) => `${t.id}:${t.content_id}:${isStream(t) ? `n${host.navSerial.get(tabKey(ws, t)) || 0}` : t.reference}:${t.link_group}:${t.pinned}:${t.lexicon_word || ""}`).join("|");
  return `${ws.id}#${number}#${column.shown_tab_id}#${tabs}`;
}

/** A stream whose tab's passage or mark is no longer what it was put at or reported was
 *  navigated by something without a position (an undo, a restore): rebuild it there. */
function noteNavigation(ws, tab) {
  if (!isStream(tab)) return;
  const key = tabKey(ws, tab);
  const known = host.known.get(key);
  if (known && (known.reference !== tab.reference || known.highlight !== (tab.highlight_range || null))) {
    host.navSerial.set(key, (host.navSerial.get(key) || 0) + 1);
  }
}

function renderColumns() {
  const ws = activeWorkspace();
  const container = $("columns");
  const order = [];
  const live = new Set();
  ws.columns.forEach((column, i) => {
    const key = `${ws.id}/${column.id}`;
    live.add(key);
    noteNavigation(ws, shownTab(column));
    const signature = columnSignature(ws, column, i + 1);
    let view = host.views.get(key);
    if (!view || view.signature !== signature) {
      view = buildColumn(ws, column, i + 1);
      view.signature = signature;
      host.views.set(key, view);
    } else {
      view.tab = shownTab(column);
      view.column = column;
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
    // A rebuilt column takes its old one's place, and the others keep their scroll (B078: a
    // stream holds several chapters); a new order re-inserts every column.
    const inPlace = order.length === host.order.length && order.every((n, i) => n === host.order[i] || !n.isConnected);
    const moved = new Set();
    if (inPlace) order.forEach((n, i) => { if (n !== host.order[i]) { container.replaceChild(n, host.order[i]); moved.add(n); } });
    else { container.replaceChildren(...order); for (const n of order) moved.add(n); }
    for (const view of host.views.values()) if (view.section.isConnected && moved.has(view.section)) restoreScroll(view);
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
    el.addEventListener("pointerdown", (e) => { if (e.button === 0 && !e.target.closest(".close")) startTabDrag(e, el, ws, column, t); });
    el.addEventListener("click", () => {
      if (el.dragged) { el.dragged = false; return; }
      if (activeTab().id !== t.id) run({ command_id: "tab.focus", args: { workspace_id: ws.id, tab_id: t.id } });
    });
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
  const view = { section, scroller, page, ws, column, tab, resource, number, text: null, stream: null, canvas: null, key: tabKey(ws, tab) };
  if (isStream(tab)) {
    buildStream(view);
  } else if (resource.kind === "notes") {
    const article = document.createElement("article");
    article.className = "measure";
    article.appendChild(noteEditor(tab.content_id, `${resource.name}, column ${number}`));
    page.appendChild(article);
  } else if (resource.kind === "lexicon") {
    page.appendChild(lexiconPage(tab));
  } else {
    page.appendChild(placeholder(ws, tab, resource));
  }
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

// MARK: - Continuous reading (B078, specs/23): the stream of chapter blocks

/** A reading tab's page: the sticky header and the stream, opened at the tab's passage once
 *  the column is in the document (restoreScroll); or specs/21's not-in-this-edition state with
 *  the core's words when the edition has no such passage. */
function buildStream(view) {
  const { tab, resource, scroller, page } = view;
  let location = null, error = null;
  try {
    location = tab.reference ? host.reading.locate(tab.content_id, tab.reference) : null;
  } catch (e) {
    error = e;
    // B071: a follower's verse past the edition's last (SBLGNT Rom 16:25): nobody typed it, so
    // it opens at the last verse at or below it, as a live follow does. A typed verse is the
    // tab's mark too, and gets specs/21's state.
    const chapter = chapterOfSegment(tab.reference);
    if (e.kind === "verse_out_of_range" && tab.highlight_range !== tab.reference && chapter) {
      location = { chapter_key: chapter, segment_id: tab.reference.split("-")[0] };
      error = null;
    }
  }
  host.known.set(view.key, { reference: tab.reference, highlight: tab.highlight_range || null });
  if (!location) {
    host.flash.delete(view.key);
    page.appendChild(error ? notInEdition(tab, resource, error) : placeholder(view.ws, tab, resource));
    return;
  }
  const header = document.createElement("div");
  header.className = "reading-header";
  scroller.insertBefore(header, page);
  const article = document.createElement("article");
  article.className = "measure stream";
  article.lang = resource.lang;
  const text = document.createElement("div");
  text.className = "text";
  text.dir = resource.dir;
  text.lang = resource.lang;
  text.addEventListener("click", (e) => studyWordAt(view, e));
  article.appendChild(text);
  page.appendChild(article);
  const canvas = document.createElement("canvas");
  canvas.className = "margin";
  canvas.setAttribute("aria-hidden", "true");
  page.appendChild(canvas);
  view.text = text;
  view.canvas = canvas;
  view.stream = new ReadingStream({
    core, reading: host.reading, chunks: host.chunks, resource, scroller, text, header,
    onTop: (anchor) => reportTop(view, anchor),
    onBlocks: () => { attributeBlocks(view); paintHighlights(); placeBand(view, true); },
    onFrame: (ms) => { placeBand(view); if (host.frames) host.frames.push(ms); },
    onSettled: () => { if (host.streamSettled) host.streamSettled(); },
  });
  // A scrolled tab comes back to its anchor; a navigation puts the passage on the reading line
  // and flashes its typed verses.
  const anchor = tab.scroll_anchor;
  view.pendingOpen = anchor
    ? { chapterKey: chapterOfSegment(anchor.segment_id) || location.chapter_key, segmentId: anchor.segment_id, scalar: anchor.scalar, mark: tab.highlight_range || null, flash: false }
    : { chapterKey: location.chapter_key, segmentId: location.segment_id, scalar: 0, mark: tab.highlight_range || null, flash: host.flash.has(view.key) };
  host.flash.delete(view.key);
  attachInk(view);
}

/** specs/21 §11 for a passage the edition does not have: the core's words ("John has no
 *  chapter 22", "Genesis is not in SBLGNT"), never another edition's text. */
function notInEdition(tab, resource, error) {
  const box = document.createElement("div");
  box.className = "placeholder not-in-edition";
  box.innerHTML = `<h2>${escapeHtml(resource.name)} · ${escapeHtml(humanPassage(core, tab.reference))}</h2>
    <p>${escapeHtml(error.message || String(error))}.</p>
    <div class="actions"><button type="button" data-act="choose">Choose another…</button><button type="button" data-act="close">Close tab</button></div>`;
  box.querySelector('[data-act="choose"]').addEventListener("click", () => run({ command_id: "palette.focus" }));
  box.querySelector('[data-act="close"]').addEventListener("click", () => run({ command_id: "tab.close" }));
  return box;
}

/** The fixture verses' licence line at the end of each block that holds one. */
function attributeBlocks(view) {
  for (const b of view.stream.blocks) {
    if (b.attributed) continue;
    b.attributed = true;
    if (!b.data.verses.some((v) => !v.placeholder)) continue;
    const fixture = host.text.passage(view.tab.content_id, b.key);
    if (!fixture) continue;
    const line = document.createElement("p");
    line.className = "attribution";
    line.textContent = fixture.attribution;
    b.section.appendChild(line);
  }
}

/** The reader moved the verse at the top: `tab.record_scroll` (quiet), which carries linked
 *  tabs along (followPositions). */
function reportTop(view, anchor) {
  const t0 = performance.now();
  run({ command_id: "tab.record_scroll", args: { workspace_id: view.ws.id, tab_id: view.tab.id, ...anchor } });
  if (host.followTimes) host.followTimes.push(performance.now() - t0);
}

function streamViewOf(key) {
  for (const view of host.views.values()) if (view.stream && view.key === key && view.section.isConnected) return view;
  return null;
}

/** B078: what a command moved (`effects.positions`, specs/23 §6). A navigation (`scrolled:
 *  false`) rebuilds each listed tab's column at its passage and flashes its typed verses; a
 *  scroll moves each linked follower without animation (and the follower does not report
 *  it); every moved tab's labels change in place, the reading column is not rebuilt. */
function followPositions(outcome) {
  const positions = (outcome && outcome.effects && outcome.effects.positions) || [];
  if (!positions.length) return;
  for (const p of positions) {
    const key = `${p.workspace_id}/${p.tab_id}`;
    if (!p.scrolled) {
      host.navSerial.set(key, (host.navSerial.get(key) || 0) + 1);
      if (p.highlight_range) host.flash.add(key);
      continue;
    }
    host.known.set(key, { reference: p.reference, highlight: p.highlight_range || null });
    if (p.driver || !p.segment_id) continue;
    const view = streamViewOf(key);
    if (view) view.stream.follow(p.segment_id);
  }
  refreshLabels();
}

/** Tab labels and identity rows from the session, written only where they changed. */
function refreshLabels() {
  for (const view of host.views.values()) {
    if (!view.section.isConnected) continue;
    const wsId = view.ws.id;
    for (const el of view.section.querySelectorAll(".tabstrip .tab")) {
      const label = host.labels.get(`${wsId}/${el.dataset.tabId}`);
      if (label && el.firstChild.textContent !== label) {
        el.firstChild.textContent = label;
        el.querySelector(".close").setAttribute("aria-label", `Close ${label}`);
      }
    }
    if (!view.stream) continue;
    const tab = sessionTab(wsId, view.tab.id);
    const passage = view.section.querySelector(".identity .passage");
    const text = tab ? humanPassage(core, tab.reference) : "";
    if (passage.textContent !== text) {
      passage.textContent = text;
      view.scroller.setAttribute("aria-label", `${view.resource.name} ${text}`);
      view.section.setAttribute("aria-label", `Column ${view.number}: ${host.labels.get(`${wsId}/${view.tab.id}`)}`);
    }
  }
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

/** Opens a freshly built stream at its tab's passage or scroll anchor, or puts a moved one
 *  back on the verse it had (moving a node resets its scroll position). */
function restoreScroll(view) {
  if (!view.stream) return;
  const o = view.pendingOpen;
  view.pendingOpen = null;
  if (o) view.stream.open(o.chapterKey, o); else view.stream.restore();
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

// MARK: - Tab drag (B074): a drag ends in `tab.move`, undoable

/** Within this far of the columns' trailing edge, a drop makes a new column. */
const NEW_COLUMN_ZONE = 56;

/** Where a tab dropped at (x, y) goes: a slot of a column's strip (the slot as drawn before the
 *  move), the end of a column dropped on its text, or a new column at the trailing edge; null
 *  off the columns. */
function dropTarget(x, y) {
  const ws = activeWorkspace();
  const box = $("columns").getBoundingClientRect();
  if (y < box.top || y > box.bottom || x < box.left) return null;
  const lastView = host.views.get(`${ws.id}/${ws.columns[ws.columns.length - 1].id}`);
  const last = lastView && lastView.section.getBoundingClientRect();
  if (last && x >= last.right - NEW_COLUMN_ZONE && x <= last.right + 8 && y > last.top + 40) return { new_column: true, column: ws.columns.length + 1 };
  const hit = document.elementFromPoint(x, y);
  const section = hit && hit.closest("#columns > .column");
  if (!section) return null;
  const number = Number(section.dataset.number);
  const column = ws.columns[number - 1];
  if (!column) return null;
  const strip = section.querySelector(".tabstrip");
  const r = strip.getBoundingClientRect();
  if (y >= r.top && y <= r.bottom) {
    const tabs = [...strip.querySelectorAll(".tab")];
    const index = tabs.filter((el) => { const b = el.getBoundingClientRect(); return b.left + b.width / 2 < x; }).length;
    return { column: number, index, strip: true };
  }
  return { column: number, index: column.tabs.length, strip: false };
}

/** Whether a drop changes nothing: back into its own slot, or a new column from a column that
 *  holds only this tab (the core refuses both; nothing is sent). */
function noOpDrop(target, column, tabId) {
  const ws = activeWorkspace();
  const from = ws.columns.findIndex((c) => c.id === column.id) + 1;
  const at = column.tabs.findIndex((t) => t.id === tabId);
  if (target.new_column) return column.tabs.length === 1;
  return target.column === from && (target.index === at || target.index === at + 1);
}

/** Draws where the tab would go: a bar at a strip slot, the column outlined, or the new-column
 *  zone labelled "New column" (the Mac's words). */
function showDrop(target) {
  let bar = $("drop-slot");
  if (!bar) { bar = document.createElement("div"); bar.id = "drop-slot"; document.body.appendChild(bar); }
  for (const el of document.querySelectorAll("#columns > .column.drop-target")) el.classList.remove("drop-target");
  bar.hidden = !target;
  if (!target) return;
  const ws = activeWorkspace();
  if (target.new_column) {
    const r = host.views.get(`${ws.id}/${ws.columns[ws.columns.length - 1].id}`).section.getBoundingClientRect();
    Object.assign(bar.style, { left: `${r.right - NEW_COLUMN_ZONE}px`, top: `${r.top}px`, width: `${NEW_COLUMN_ZONE}px`, height: `${r.height}px` });
    bar.className = "zone";
    bar.textContent = "New column";
    return;
  }
  const view = host.views.get(`${ws.id}/${ws.columns[target.column - 1].id}`);
  if (!target.strip) { view.section.classList.add("drop-target"); bar.hidden = true; return; }
  const strip = view.section.querySelector(".tabstrip");
  const tabs = [...strip.querySelectorAll(".tab")];
  const sr = strip.getBoundingClientRect();
  const x = target.index < tabs.length ? tabs[target.index].getBoundingClientRect().left : (tabs.length ? tabs[tabs.length - 1].getBoundingClientRect().right : sr.left);
  Object.assign(bar.style, { left: `${x - 1}px`, top: `${sr.top + 4}px`, width: "2px", height: `${sr.height - 8}px` });
  bar.className = "slot";
  bar.textContent = "";
}

function startTabDrag(event, el, ws, column, tab) {
  const x0 = event.clientX, y0 = event.clientY;
  let ghost = null, target = null;
  el.setPointerCapture(event.pointerId);
  const move = (e) => {
    if (!ghost) {
      if (Math.hypot(e.clientX - x0, e.clientY - y0) < 5) return;
      ghost = document.createElement("div");
      ghost.className = "tab-ghost";
      ghost.textContent = el.firstChild.textContent;
      document.body.appendChild(ghost);
      el.classList.add("dragging");
    }
    ghost.style.transform = `translate(${e.clientX + 8}px, ${e.clientY + 8}px)`;
    target = dropTarget(e.clientX, e.clientY);
    if (target && noOpDrop(target, column, tab.id)) target = null;
    showDrop(target);
  };
  const up = (e) => {
    el.removeEventListener("pointermove", move);
    el.removeEventListener("pointerup", up);
    el.removeEventListener("pointercancel", up);
    if (!ghost) return;
    ghost.remove();
    el.classList.remove("dragging");
    showDrop(null);
    el.dragged = true;
    setTimeout(() => { el.dragged = false; }, 0);
    if (e.type === "pointercancel" || !target) return;
    const args = target.new_column ? { tab_id: tab.id, new_column: true, column: target.column } : { tab_id: tab.id, column: target.column, index: target.index };
    run({ command_id: "tab.move", args });
  };
  el.addEventListener("pointermove", move);
  el.addEventListener("pointerup", up);
  el.addEventListener("pointercancel", up);
}

// MARK: - Highlights (anchors through the core, as the B005 reader)

function selectionInView() {
  const s = document.getSelection();
  if (!s || s.rangeCount === 0 || s.isCollapsed) return null;
  const range = s.getRangeAt(0);
  const view = viewOfNode(range.commonAncestorContainer);
  if (!view) return null;
  const trip = streamAnchors(view, range);
  return trip ? { view, trip, range, placeholder: view.stream.touchesPlaceholder(range) } : null;
}

/** A selection in a reading stream: each block it touches is one core paragraph (B078), so
 *  the range is cut at the blocks' text and their anchors are put together in order. */
function streamAnchors(view, range) {
  const anchors = [], ranges = [];
  for (const b of view.stream.blocks) {
    if (!range.intersectsNode(b.verses)) continue;
    const part = document.createRange();
    part.selectNodeContents(b.verses);
    if (range.compareBoundaryPoints(Range.START_TO_START, part) > 0) part.setStart(range.startContainer, range.startOffset);
    if (range.compareBoundaryPoints(Range.END_TO_END, part) < 0) part.setEnd(range.endContainer, range.endOffset);
    if (part.collapsed) continue;
    let trip = null;
    try { trip = selectionAnchors(core, b.paragraph, b.verses, part); } catch { trip = null; }
    if (!trip) continue;
    anchors.push(...trip.anchors);
    ranges.push(...trip.ranges);
  }
  return anchors.length ? { anchors, ranges } : null;
}

/** Placeholder verses are never quoted, collected or studied (specs/23 §5): says so. */
function refusePlaceholder(found) {
  if (!found || !found.placeholder) return false;
  toast(PLACEHOLDER_REASON);
  return true;
}

/** B074: the selection's anchors as one stored highlight (scalar spans, one per segment, in
 *  text order) into the core, which refuses one on placeholder text (`placeholder_text`, said
 *  in the words of specs/23 §5). */
function highlightSelection(swatch = host.session.ink.swatch) {
  const found = selectionInView();
  if (!found) return toast("Select some text first");
  const anchors = found.trip.anchors;
  const highlight = {
    resource_id: anchors[0].edition_id, content_revision: anchors[0].content_revision,
    spans: anchors.map((a) => ({ segment_id: a.segment_id, start: a.range.start, end: a.range.end })), color: swatch.toUpperCase(),
  };
  try {
    host.handle.addHighlight(highlight, host.reading);
  } catch (error) {
    toast(error.kind === "placeholder_text" ? PLACEHOLDER_REASON : error.message || String(error));
    return;
  }
  highlightsChanged(highlight.resource_id, highlight.spans);
  changed();
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
  if (refusePlaceholder(found)) return;
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

/** After a highlight changed: its chapters are fetched again (all when none is named) and
 *  every block works out its paint again. */
function highlightsChanged(resource = null, spans = null) {
  if (resource && spans) for (const x of spans) host.hlCache.delete(`${resource}|${chapterOfSegment(x.segment_id)}`);
  else host.hlCache.clear();
  host.hlVersion += 1;
}

/** UTF-16 index of a Unicode scalar offset in `text`. */
function utf16At(text, scalar) {
  let i = 0;
  for (let n = 0; n < scalar && i < text.length; n++) i += text.codePointAt(i) > 0xffff ? 2 : 1;
  return i;
}

/** B074: what a rendered block paints, `[highlight name, Range]` in the core's order (oldest
 *  first, `highlightsIn`); worked out once per block and again only after a highlight changed
 *  (a block's text never changes while it is rendered). A span of another chapter is painted
 *  by that chapter's block. */
function blockHighlights(view, b) {
  if (b.hlVersion === host.hlVersion) return b.hlRanges;
  b.hlVersion = host.hlVersion;
  b.hlRanges = [];
  const key = `${view.tab.content_id}|${b.key}`;
  let marks = host.hlCache.get(key);
  if (!marks) { marks = host.handle.highlightsIn(view.tab.content_id, b.key); host.hlCache.set(key, marks); }
  if (!marks.length) return b.hlRanges;
  if (!b.textNodes) {
    b.textNodes = new Map();
    for (const v of b.verses.querySelectorAll("[data-segment]")) {
      const t = v.querySelector(".t");
      if (t && t.firstChild) b.textNodes.set(v.dataset.segment, t.firstChild);
    }
  }
  for (const h of marks) {
    const name = highlightName(h.color);
    for (const stored of h.spans) {
      const node = b.textNodes.get(stored.segment_id);
      if (!node) continue;
      const span = h.content_revision === b.data.content_revision ? stored : reanchored(h, stored, b);
      if (!span) continue;
      const r = document.createRange();
      r.setStart(node, Math.min(node.length, utf16At(node.data, span.start)));
      r.setEnd(node, Math.min(node.length, utf16At(node.data, span.end)));
      b.hlRanges.push([name, r]);
    }
  }
  return b.hlRanges;
}

/** B071: a highlight stored on another revision of the text (the fixture overlays' `bsb-2024`
 *  under the pack's), found again by the core's `reanchor`, which never attaches to a different
 *  passage. Painted where it lands, and the stored highlight is left as it was. What the core
 *  cannot place, or a revision whose text this host no longer has, is not painted and is kept. */
function reanchored(highlight, span, block) {
  const verse = block.data.verses.find((v) => v.segment_id === span.segment_id);
  const old = host.text.fixtureRevision(highlight.resource_id, highlight.content_revision, span.segment_id);
  let result = null;
  if (verse && old) {
    const anchor = { edition_id: highlight.resource_id, content_revision: highlight.content_revision, segment_id: span.segment_id, range: { start: span.start, end: span.end }, tokens: [], quote: null };
    const now = { content_revision: block.data.content_revision, text: verse.text, tokens: verse.tokens.map((t) => ({ id: core.parseTokenId(t.id), range: { start: t.start, end: t.end } })) };
    try { result = core.reanchor(anchor, old, now); } catch (error) { console.warn("reanchor failed", error); }
  }
  host.reanchors[result ? result.status : "unknown_revision"] = (host.reanchors[result ? result.status : "unknown_revision"] || 0) + 1;
  return result && (result.status === "exact" || result.status === "moved") ? result.value.range : null;
}

/** The highlights of every rendered block, set again only when the blocks on screen or the
 *  highlights changed. */
function paintHighlights() {
  if (!("highlights" in CSS)) return;
  const blocks = [];
  for (const view of host.views.values()) {
    if (!view.stream || !view.section.isConnected) continue;
    for (const b of view.stream.blocks) blocks.push([view, b]);
  }
  const last = host.painted;
  if (last && last.version === host.hlVersion && last.blocks.length === blocks.length && last.blocks.every((x, i) => x[1] === blocks[i][1])) return;
  host.painted = { version: host.hlVersion, blocks };
  const bySwatch = new Map(SWATCH_ORDER.map((s) => [`ol-${s}`, []]));
  for (const [view, b] of blocks) {
    for (const [name, range] of blockHighlights(view, b)) {
      if (!bySwatch.has(name)) bySwatch.set(name, []);
      bySwatch.get(name).push(range);
    }
  }
  for (const [name, ranges] of bySwatch) CSS.highlights.set(name, new Highlight(...ranges));
}

/** The core's well for the current context (specs/22 §4.1). */
function fetchWell() {
  const ctx = context();
  host.toolWell = host.handle.toolWell(ctx);
  host.wellSelection = ctx.has_selection;
  // B078: on placeholder verses the quoting and collecting actions say why they are off.
  if (ctx.has_selection) {
    const found = selectionInView();
    if (found && found.placeholder) {
      for (const g of host.toolWell.groups) for (const item of g.items) if (QUOTE_COMMANDS.has(item.command_id)) item.disabled_text = PLACEHOLDER_REASON;
    }
  }
}

/** The commands that quote, collect or study the text under them (specs/23 §5). */
const QUOTE_COMMANDS = new Set(["selection.highlight", "selection.highlight_in", "selection.collect", "selection.note", "selection.study", "copy.passage", "tab.open_lexicon", "search.lemma"]);

/** The core's menu with its quoting rows disabled, for a word or selection on placeholder
 *  verses (a submenu of them goes; one quoting row inside a submenu is disabled). */
function withPlaceholderRows(menu) {
  const off = (item) => ({ ...item, disabled_text: PLACEHOLDER_REASON, submenu: [] });
  const items = (list) => list.map((item) => {
    if (QUOTE_COMMANDS.has(item.command_id)) return off(item);
    return item.submenu && item.submenu.length ? { ...item, submenu: items(item.submenu) } : item;
  });
  return { ...menu, sections: menu.sections.map((sec) => ({ ...sec, items: items(sec.items) })) };
}

/** Whether a context-menu hit is on placeholder verses. */
function onPlaceholder(hit) {
  if (!hit.view || !hit.view.stream) return false;
  if (hit.word) return !!hit.word.node.parentElement.closest(".verse.placeholder-verse");
  if (hit.target.kind === "selection") { const found = selectionInView(); return !!(found && found.placeholder); }
  return false;
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
  if (refusePlaceholder(found)) return;
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
  if (found.node.parentElement.closest(".verse.placeholder-verse")) { toast(PLACEHOLDER_REASON); return; }
  // tab.open_lexicon reuses the Lexicon tab in the next column or opens one (a new column when
  // there is none), and focuses it; the core records the clicked edition's language on the tab
  // (`lexicon_language`, B074), so a restored tab looks up in the same lexicon.
  run({ command_id: "tab.open_lexicon", args: { text: found.word, reference: found.segment_id } });
}

/** The lexicon tab's page: the word as clicked, its language and passage, and an honest line
 *  that the entries are not there yet (no glosses are made up). */
function lexiconPage(tab) {
  const lang = tab.lexicon_language || null;
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

// MARK: - Notes (B076; B074: the core holds them, specs/24 §2.1)

/** Fills an editor from the core's note (`note(id)`), or, for a note the core does not have
 *  yet, the fixture note's text as one paragraph (saved on its first edit). */
function loadNote(editor, noteId) {
  const stored = host.handle.note(noteId);
  if (stored) { renderNote(editor, stored, passageLink); return; }
  const note = host.text.passage(noteId, null);
  const text = note ? note.paragraph.segments.map((s) => s.text).join(note.paragraph.joiner) : "";
  renderNote(editor, { id: noteId, blocks: [{ kind: "paragraph", spans: text ? [{ text }] : [] }] }, passageLink);
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
  // An edit another editor has not handed to the core yet goes first, so this one shows it.
  flushNotes();
  loadNote(el, noteId);
  el.addEventListener("input", () => noteChanged(el));
  el.addEventListener("blur", () => flushNote(el));
  el.addEventListener("click", (e) => {
    const link = e.target.closest("a.passage-link");
    if (!link) return;
    e.preventDefault();
    host.focus = "tab";
    run({ command_id: "nav.navigate", args: { reference: link.dataset.reference, column: host.readingColumn } });
  });
  return el;
}

/** The editor changed: the other editors of the note follow at once, the core gets the note
 *  when the edit settles (300 ms after the last input, or on blur). */
function noteChanged(editor) {
  for (const other of document.querySelectorAll(".note-editor")) {
    if (other !== editor && other.dataset.note === editor.dataset.note) other.innerHTML = editor.innerHTML;
  }
  clearTimeout(editor.noteTimer);
  editor.noteDirty = true;
  editor.noteTimer = setTimeout(() => flushNote(editor), NOTE_SETTLE_MS);
}
const NOTE_SETTLE_MS = 300;

/** `putNote` with the editor's blocks, when it has an edit the core has not seen. */
function flushNote(editor) {
  clearTimeout(editor.noteTimer);
  if (!editor.noteDirty) return;
  editor.noteDirty = false;
  try {
    host.handle.putNote(noteFromEditor(editor, editor.dataset.note));
  } catch (error) {
    toast(error.message || String(error));
    return;
  }
  changed();
}

/** Every editor's pending edit into the core (before a save on hide). */
function flushNotes() {
  for (const editor of document.querySelectorAll(".note-editor")) flushNote(editor);
}

/** A passage link: the human reference, not editable as text, carrying the OSIS reference. */
function passageLink(reference, text = humanPassage(core, reference)) {
  const a = document.createElement("a");
  a.className = "passage-link";
  a.href = "#";
  a.contentEditable = "false";
  a.dataset.reference = reference;
  a.textContent = text;
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
    const block = node && (node.nodeType === 1 ? node : node.parentElement).closest("h2, h3, p, li, blockquote, div:not(.note-editor)");
    document.execCommand("formatBlock", false, block && /^H[23]$/.test(block.tagName) ? "p" : "h2");
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
//
// B074 (specs/24 §2.3): the core holds the strokes, each pinned to the verse under its first
// point, with `y` from that verse's top and `x` from the text's leading edge (the right edge
// in a right-to-left text, measured leftward). A block's strokes come from `inkIn` and are drawn
// at the verse's top, so they move with it and are hidden while it is not rendered.

/** The core's strokes of an edition's chapter (`inkIn`), fetched once and again only after a
 *  stroke of that chapter changed; every column showing the chapter shares them. */
function chapterInk(resource, chapterKey) {
  const key = `${resource}|${chapterKey}`;
  let marks = host.inkCache.get(key);
  if (!marks) { marks = host.handle.inkIn(resource, chapterKey); host.inkCache.set(key, marks); }
  return marks;
}
const blockInk = (view, b) => chapterInk(view.tab.content_id, b.key);

/** Whether any rendered block of the view holds a stroke. */
function viewHasInk(view) {
  return !!view.stream && view.stream.blocks.some((b) => blockInk(view, b).length > 0);
}

/** The text's leading edge in page x, and the direction x grows from it. */
function inkFrame(view) {
  const page = view.page.getBoundingClientRect(), text = view.text.getBoundingClientRect();
  return view.resource.dir === "rtl" ? { edge: text.right - page.left, sign: -1 } : { edge: text.left - page.left, sign: 1 };
}

/** The margin layer is a band three viewports tall around the visible text, moved (and
 *  repainted) only when the reader scrolls out of it or blocks change, so strokes scroll with
 *  the text and a long stream never holds a page-tall canvas (B078). */
function sizeCanvas(view) {
  if (!view.canvas) return;
  const dpr = window.devicePixelRatio || 1;
  // No bitmap until there is ink to show or an ink tool to draw with (a tab switch then
  // allocates nothing).
  const needed = ["pen", "highlighter_pen", "eraser"].includes(host.toolWell && host.toolWell.effective_tool) || viewHasInk(view);
  const w = needed ? view.page.clientWidth : 0, h = needed ? view.scroller.clientHeight * 3 : 0;
  if (view.canvas.width === Math.round(w * dpr) && view.canvas.height === Math.round(h * dpr)) return;
  view.canvas.width = Math.round(w * dpr);
  view.canvas.height = Math.round(h * dpr);
  view.canvas.style.width = `${w}px`;
  view.canvas.style.height = `${h}px`;
  placeBand(view, true);
}

/** Puts the band around the viewport (page coordinates) and repaints when it moved. */
function placeBand(view, force = false) {
  if (!view.canvas || !view.section.isConnected) return;
  const visible = view.scroller.scrollTop - view.page.offsetTop;
  const h = view.scroller.clientHeight;
  const top = view.bandTop;
  if (!force && top != null && visible >= top && visible + h <= top + h * 3) return;
  view.bandTop = Math.max(0, visible - h);
  view.canvas.style.top = `${view.bandTop}px`;
  scheduleInk(view);
}

/** Repaints a view's ink before the next frame is painted, once however often it was asked
 *  for in this frame (a workspace switch moves a band, sizes a canvas and loads blocks). */
const inkDue = new Set();
function scheduleInk(view) {
  if (!inkDue.size) requestAnimationFrame(flushInk);
  inkDue.add(view);
}
function flushInk() {
  const views = [...inkDue];
  inkDue.clear();
  for (const v of views) if (v.canvas && v.section.isConnected) repaintInk(v);
}

/** Page y of a verse's top. */
function verseTop(view, segmentId) {
  const el = view.stream && view.stream.verseEl(segmentId);
  return el ? el.getBoundingClientRect().top - view.page.getBoundingClientRect().top : null;
}

/** The verse under a page point (a line box of it holds the point; else the nearest line box),
 *  and that verse's top, for a stroke's pin. */
function verseAnchorAt(view, x, y) {
  if (!view.stream) return null;
  const page = view.page.getBoundingClientRect();
  const cx = x + page.left, cy = y + page.top;
  let best = null;
  for (const b of view.stream.blocks) {
    for (const v of b.verses.querySelectorAll("[data-segment]")) {
      for (const r of v.getClientRects()) {
        const dy = cy < r.top ? r.top - cy : cy > r.bottom ? cy - r.bottom : 0;
        const dx = cx < r.left ? r.left - cx : cx > r.right ? cx - r.right : 0;
        const d = dy * 1000 + dx; // the line first, then along it
        if (!best || d < best.d) best = { d, el: v };
        if (d === 0) break;
      }
      if (best && best.d === 0) break;
    }
    if (best && best.d === 0) break;
  }
  return best && { segment_id: best.el.dataset.segment, top: best.el.getBoundingClientRect().top - page.top };
}

/** The view's stored strokes in page coordinates, for drawing and erasing. */
function pageStrokes(view) {
  if (!view.stream) return [];
  const frame = inkFrame(view);
  const out = [];
  for (const b of view.stream.blocks) {
    for (const mark of blockInk(view, b)) {
      const top = verseTop(view, mark.segment_id);
      if (top == null) continue;
      const stroke = mark.stroke;
      out.push({ ...stroke, id: mark.id, segment_id: mark.segment_id, points: stroke.points.map((pt) => ({ ...pt, x: frame.edge + frame.sign * pt.x, y: top + pt.y })) });
    }
  }
  return out;
}

/** A stored stroke's outline in its own coordinates (from its verse's top and the text's
 *  edge), worked out once per stroke: a stroke never changes, only where it is drawn. */
const outlines = new Map();
function outlineOf(mark) {
  let o = outlines.get(mark.id);
  if (!o) { o = strokeOutline(core, mark.stroke); outlines.set(mark.id, o); }
  return o;
}

function repaintInk(view, live = null) {
  const ctx = view.canvas.getContext("2d");
  const dpr = window.devicePixelRatio || 1;
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.clearRect(0, 0, view.canvas.width, view.canvas.height);
  if (!view.canvas.width) return;
  const band = view.bandTop || 0, bottom = band + view.canvas.height / dpr;
  if (view.stream) {
    const frame = inkFrame(view);
    for (const b of view.stream.blocks) {
      for (const mark of blockInk(view, b)) {
        const top = verseTop(view, mark.segment_id);
        if (top == null) continue;
        // Only what reaches into the band.
        let lo = Infinity, hi = -Infinity;
        for (const pt of mark.stroke.points) { if (pt.y < lo) lo = pt.y; if (pt.y > hi) hi = pt.y; }
        if (top + hi + mark.stroke.width < band || top + lo - mark.stroke.width > bottom) continue;
        // x from the leading edge (mirrored in a right-to-left text), y from the verse's top.
        ctx.setTransform(frame.sign, 0, 0, 1, frame.edge * dpr, (top - band) * dpr);
        compositeAt(ctx, mark.stroke, outlineOf(mark), dpr);
      }
    }
  }
  if (live && live.points && live.points.length >= 2) {
    ctx.setTransform(1, 0, 0, 1, 0, -band * dpr);
    compositeAt(ctx, live, strokeOutline(core, live), dpr);
  }
  ctx.setTransform(1, 0, 0, 1, 0, 0);
}

/** A colour as `#RRGGBB` (the store's form), whatever CSS form it was given in. */
function hexColour(colour) {
  const c = String(colour).trim();
  if (/^#[0-9a-f]{6}$/i.test(c)) return c.toUpperCase();
  const probe = document.createElement("canvas").getContext("2d");
  probe.fillStyle = c;
  const v = probe.fillStyle;
  return /^#[0-9a-f]{6}$/i.test(v) ? v.toUpperCase() : "#000000";
}

/** Every stream showing an edition repaints its ink (strokes belong to the edition); the
 *  chapter that changed is fetched again (all of them when none is named). */
function inkChanged(resource = null, segmentId = null) {
  if (resource && segmentId) host.inkCache.delete(`${resource}|${chapterOfSegment(segmentId)}`);
  else host.inkCache.clear();
  for (const v of host.views.values()) if (v.stream && v.section.isConnected) { sizeCanvas(v); scheduleInk(v); }
  changed();
}

function attachInk(view) {
  const canvas = view.canvas;
  const arbiter = new TouchArbiter("stylus");
  let live = null, scheduled = false;
  // Page coordinates: the band moves, the page does not.
  const point = (e) => { const r = view.page.getBoundingClientRect(); return { x: e.clientX - r.left, y: e.clientY - r.top }; };
  const contact = (e) => (e.pointerType === "pen" ? "stylus" : e.pointerType === "touch" ? "finger" : "pointer");
  const frame = () => { if (scheduled) return; scheduled = true; requestAnimationFrame(() => { scheduled = false; repaintInk(view, live); }); };
  const erase = (p) => {
    const strokes = pageStrokes(view);
    for (let i = strokes.length - 1; i >= 0; i--) {
      const s = strokes[i];
      for (let j = 1; j < s.points.length; j++) {
        if (distanceToSegment(p, s.points[j - 1], s.points[j]) <= s.width / 2 + 4) {
          try { host.handle.removeInk(s.id); } catch { return; }
          host.erased.set(view.tab.content_id, (host.erased.get(view.tab.content_id) || 0) + 1);
          inkChanged(view.tab.content_id, s.segment_id);
          return;
        }
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
    const color = hexColour(tool === "highlighter_pen" ? host.session.ink.swatch : getComputedStyle(root).getPropertyValue("--accent"));
    const p = point(e);
    live = { id: "live", tool: brush.tool, brush_version: brush.version, width, color, opacity: brush.opacity, points: [{ ...p, pressure: e.pointerType === "pen" ? e.pressure : null, elapsed_ms: 0 }], started: e.timeStamp };
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
    const done = live;
    live = null;
    if (!done.eraser && done.points.length >= 2 && e.type !== "pointercancel") {
      // Pinned to the verse under the first point: x from the text's leading edge, y from the
      // verse's top (the core rounds to a tenth of a point).
      const pin = verseAnchorAt(view, done.points[0].x, done.points[0].y);
      if (pin) {
        const f = inkFrame(view);
        const { started, ...stroke } = done;
        stroke.points = stroke.points.map((pt) => ({ ...pt, x: (pt.x - f.edge) * f.sign, y: pt.y - pin.top }));
        try {
          host.handle.addInk({ resource_id: view.tab.content_id, segment_id: pin.segment_id, stroke });
          inkChanged(view.tab.content_id, pin.segment_id);
          return;
        } catch (error) {
          toast(error.message || String(error));
        }
      }
    }
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

/** B071: "About this edition": the edition's name, revision and the attribution its licence
 *  asks a reader to be shown (`Edition.attribution`), one menu row away from the text. */
function aboutItem(menu, button) {
  const b = document.createElement("button");
  b.type = "button";
  b.setAttribute("role", "menuitem");
  b.textContent = "About this edition";
  b.addEventListener("click", () => {
    const tab = activeTab();
    closeMenu(false);
    openMenu(button, (panel) => {
      panel.classList.add("about");
      panel.setAttribute("aria-label", "About this edition");
      fillAbout(panel, tab.content_id);
    }, "dialog");
  });
  menu.appendChild(b);
}

function fillAbout(panel, contentId) {
  const resource = host.text.resource(contentId);
  const edition = host.text.edition(contentId);
  const title = document.createElement("h2");
  title.textContent = resource.name;
  panel.appendChild(title);
  if (!edition) {
    const p = document.createElement("p");
    p.textContent = PLACEHOLDER_NOTICE;
    panel.appendChild(p);
    return;
  }
  const revision = document.createElement("p");
  revision.className = "revision";
  revision.textContent = edition.content_revision;
  panel.appendChild(revision);
  const p = document.createElement("p");
  p.className = "credit";
  // The attribution as the pack has it; its addresses can be followed.
  for (const part of (edition.attribution || "").split(/(https?:\/\/[^\s,;)]*[^\s,;.)])/)) {
    if (/^https?:\/\//.test(part)) {
      const a = document.createElement("a");
      a.href = part;
      a.target = "_blank";
      a.rel = "noopener noreferrer";
      a.textContent = part;
      p.appendChild(a);
    } else {
      p.appendChild(document.createTextNode(part));
    }
  }
  panel.appendChild(p);
}

function openPaneMenu(button) {
  openMenu(button, (menu) => {
    const states = commandStates();
    const ws = activeWorkspace();
    if (isStream(activeTab())) {
      aboutItem(menu, button);
      menu.appendChild(document.createElement("hr"));
    }
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
  if (onPlaceholder(hit)) menu = withPlaceholderRows(menu);
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

/** The text of a verse or verse range of a resource (one chapter), from the reading handle;
 *  null when a verse of it is a placeholder (specs/23 §5). */
function verseText(resourceId, reference) {
  if (resourceId && reference && host.editions.has(resourceId)) {
    const [first, last = first] = reference.split("-");
    let verses;
    try { verses = host.reading.chapterContent(resourceId, chapterOfSegment(first)).verses; } catch { return ""; }
    const i = verses.findIndex((v) => v.segment_id === first);
    const j = verses.findIndex((v) => v.segment_id === last);
    const picked = i < 0 ? [] : verses.slice(i, Math.max(i, j) + 1);
    return picked.some((v) => v.placeholder) ? null : picked.map((v) => v.text).join(" ");
  }
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
  if (words === null) return toast(PLACEHOLDER_REASON);
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
  if (kind === "copy") toast("Copied");
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
    if (!reference) return toast("No passage on the clipboard");
    if (run({ command_id: "nav.navigate", args: { reference } })) toast(`Opened ${humanPassage(core, reference)}`);
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
  toast(`Removed link to ${humanPassage(core, reference)}`);
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
  // A stream finds in the named chapter's block (B078).
  const scope = (view.stream && view.text.querySelector(`section.chapter[data-chapter="${CSS.escape(reference)}"]`)) || view.text;
  for (const t of scope.querySelectorAll(".t")) {
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
  // B071: an answer still on its way is for a palette that has closed.
  host.palette.textGroup = null;
  host.palette.textWanted = false;
  host.palette.textPending = false;
  if (host.search) host.search.seq += 1;
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
  host.palette.textPending = false;
  if (host.search) {
    // B071: passages, commands and tabs now, the Text section when the worker answers. The rows
    // of the last answer stay meanwhile, so the list does not jump on every keystroke.
    const wanted = wantsText(result.query);
    host.palette.textWanted = wanted;
    if (!wanted) host.palette.textGroup = null;
    if (wanted && host.search.state === "ready") {
      host.palette.textPending = true;
      host.palette.textAsked = performance.now();
      host.search.ask(text, expanded);
    }
    if (host.palette.textGroup) result.groups.push(host.palette.textGroup);
  }
  scopeText(result);
  host.palette.result = result;
  renderScopeChip();
  renderPalette(result);
  return result;
}

/** A scoped search keeps the rows of its editions. */
function scopeText(result) {
  const keep = scopeResources();
  if (!keep) return;
  const shorts = [...keep].map((id) => `${host.text.resource(id).short} · `);
  for (const group of result.groups) {
    if (group.section !== "text") continue;
    const items = group.items.filter((item) => shorts.some((p) => item.title.startsWith(p)));
    const scoped = { ...group, items, total: items.length };
    result.groups[result.groups.indexOf(group)] = scoped;
  }
}

/** B071: the worker's Text section for the field's current text goes into the result on screen.
 *  A row chosen above the section stays chosen. */
function textAnswered(group) {
  host.palette.textPending = false;
  if (results.hidden || !host.palette.result) return;
  host.palette.textGroup = group;
  const result = host.palette.result;
  result.groups = result.groups.filter((g) => g.section !== "text");
  if (group) result.groups.push(group);
  scopeText(result);
  const row = host.palette.rows[host.palette.index];
  const keep = row && row.section !== "text" ? host.palette.index : null;
  const top = results.scrollTop;
  renderPalette(result, keep);
  results.scrollTop = top;
  if (host.textLatency) host.textLatency(group);
}

/** B071: the search's state changed; if the palette is waiting on it, it asks or says so. */
function textStateChanged() {
  if (results.hidden || !host.palette.textWanted) return;
  if (host.search.state === "ready") { queryPalette(); return; }
  const hint = results.querySelector(".text-status");
  if (hint) hint.textContent = host.search.statusLine();
  else renderPalette(host.palette.result, host.palette.index);
}

const SECTION_TITLES = { passage: "Passage", commands: "Commands", tabs: "Tabs and workspaces", text: "Text" };

function renderPalette(result, keepIndex = null) {
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
      parts.push(`<div class="row${item.disabled ? " disabled" : ""}" role="option" id="pr-${i}" data-i="${i}" aria-selected="false"${item.disabled ? ' aria-disabled="true"' : ""}><span class="title">${escapeHtml(title)}</span>${item.shortcut ? `<span class="key">${escapeHtml(shortcutGlyphs(item.shortcut))}</span>` : ""}${sub ? `<span class="sub" dir="auto">${escapeHtml(sub)}</span>` : ""}</div>`);
    }
    if (group.total > group.items.length) {
      const i = rows.length;
      rows.push({ kind: "more", section: group.section });
      parts.push(`<div class="row more" role="option" id="pr-${i}" data-i="${i}" aria-selected="false"><span class="title">Show all ${group.total}</span><span class="key">⌥↩</span></div>`);
    }
  }
  if (result.notice) parts.push(`<div class="hint">${escapeHtml(result.notice)}</div>`);
  // B071: a search not ready is said to be, and an answer on its way is not "nothing".
  const status = host.search && host.palette.textWanted ? host.search.statusLine() : null;
  if (status) parts.push(`<h3 id="ph-text">${SECTION_TITLES.text}</h3><div class="hint text-status">${escapeHtml(status)}</div>`);
  if (rows.length === 0 && !status && !host.palette.textPending) {
    parts.push(field.value.trim()
      ? `<div class="hint">Nothing matches “${escapeHtml(field.value.trim())}”.</div>`
      : `<div class="hint">Type a passage (<kbd>Jn 3:16</kbd>), a word, a command or a tab. <kbd>&gt;</kbd> commands · <kbd>@</kbd> tabs and workspaces · <kbd>#</kbd> text · <kbd>:</kbd> passage · <kbd>@2</kbd> at the end opens in column 2.</div>`);
  }
  results.innerHTML = parts.join("");
  host.palette.rows = rows;
  results.hidden = false;
  field.setAttribute("aria-expanded", "true");
  const firstEnabled = rows.findIndex((r) => r.kind === "item" && !r.item.disabled);
  select(keepIndex != null && keepIndex < rows.length ? keepIndex : firstEnabled >= 0 ? firstEnabled : rows.length ? 0 : -1);
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
window.addEventListener("resize", () => { host.well.autoTop = null; placeWell(); for (const v of host.views.values()) if (v.section.isConnected) sizeCanvas(v); restoreStreams(); });

// MARK: - Toast

let toastTimer = 0;
function toast(message, ms = 2400) {
  const el = $("toast");
  el.textContent = message;
  el.classList.add("show");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.remove("show"), ms);
  host.lastToast = message;
}

/** B074: a quiet notice about the saved store, said once and left up a little longer. */
function notice(message) {
  host.notices = [...(host.notices || []), message];
  toast(message, 6000);
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

/** A scripted continuous scroll: `dy` px each frame for `frames` frames, as a steady wheel
 *  would. Rows: the frame intervals (a frame beyond one interval is a dropped frame), each
 *  scroll frame's own work (the window, the header, the reading line), and each linked follow
 *  (`tab.record_scroll` through every follower scrolled and laid out). */
async function scrollRun(view, frames, dy) {
  host.frames = [];
  host.followTimes = [];
  const intervals = [], chapters = [];
  let maxBlocks = 0, maxNodes = 0, last = null, i = 0;
  const heap0 = performance.memory ? performance.memory.usedJSHeapSize : null;
  await new Promise((ok) => {
    const step = (t) => {
      if (last != null) intervals.push(t - last);
      last = t;
      const b = view.stream.blockAtTop();
      if (b && chapters[chapters.length - 1] !== b.key) chapters.push(b.key);
      maxBlocks = Math.max(maxBlocks, view.stream.blocks.length);
      maxNodes = Math.max(maxNodes, view.page.getElementsByTagName("*").length);
      if (i++ >= frames) { ok(); return; }
      view.scroller.scrollTop += dy;
      requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  });
  const heap = performance.memory ? performance.memory.usedJSHeapSize : null;
  const extra = { chapters: chapters.length, from: chapters[0], to: chapters[chapters.length - 1], max_blocks: maxBlocks, max_dom_nodes: maxNodes, frames_over_25ms: intervals.filter((x) => x > 25).length, js_heap_mb: heap && +(heap / 1048576).toFixed(1), js_heap_delta_mb: heap && +((heap - heap0) / 1048576).toFixed(1), stats: { ...view.stream.stats } };
  const rows = [
    { name: `B078 continuous scroll, ${dy} px a frame through ${chapters.length} chapters (frame interval; work = one scroll frame)`, work_ms: stats(host.frames), frame_ms: stats(intervals), ...extra },
    { name: "B078 linked follow (tab.record_scroll → followers scrolled, during the scroll)", work_ms: stats(host.followTimes), frame_ms: stats(host.followTimes) },
  ];
  host.frames = null;
  host.followTimes = null;
  return rows;
}

/** B071: until the shown streams have their text, then painted: the frame after it and the
 *  task after that frame. */
async function textPainted() {
  const busy = () => [...host.views.values()].some((v) => v.section.isConnected && v.stream && v.stream.waiting);
  while (busy()) await new Promise((ok) => { host.streamSettled = ok; setTimeout(ok, 50); });
  host.streamSettled = null;
  await nextFrame();
  await new Promise((ok) => { const c = new MessageChannel(); c.port1.onmessage = () => ok(); c.port2.postMessage(0); });
  return performance.now();
}

/** B071: the end-to-end rows of specs/22 §7 over the real pack, each with a repaint. `queries`
 *  (bench-queries.json) are replayed keystroke by keystroke, a sample ending when that
 *  keystroke's text results have been painted. */
export async function benchPack({ samples = 200, queries = [] } = {}) {
  const out = [];
  const row = (name, values, extra = {}) => out.push({ name, ...stats(values), ...extra });
  while (host.search.state !== "ready") await new Promise((ok) => setTimeout(ok, 20));
  // Keystroke → painted text results.
  const strokes = queries.flatMap((q) => [...q].slice(1).map((_, i) => [...q].slice(0, i + 2).join("")));
  const field = $("palette-field");
  focusPalette("");
  const painted = [], handler = [], answered = [];
  let longTasks = 0;
  const observer = new PerformanceObserver((list) => { longTasks += list.getEntries().length; });
  observer.observe({ entryTypes: ["longtask"] });
  for (let i = 0; i < Math.max(samples, strokes.length); i++) {
    await new Promise((ok) => setTimeout(ok, Math.random() * 16));
    const t0 = performance.now();
    const answer = new Promise((ok) => { host.textLatency = ok; });
    field.value = strokes[i % strokes.length];
    field.dispatchEvent(new Event("input"));
    handler.push(performance.now() - t0);
    await answer;
    answered.push(performance.now() - t0);
    painted.push((await textPainted()) - t0);
  }
  observer.disconnect();
  host.textLatency = null;
  row("keystroke → text results painted (script-driven; budget 50)", painted, { long_tasks_over_50ms: longTasks });
  row("  of which the page's own handler per keystroke (main thread)", handler);
  row("  of which keystroke → worker's answer rendered", answered);
  closePalette(false);
  field.value = "";
  // Open a chapter.
  run({ command_id: "column.focus", args: { column: 1 } });
  const openOnce = async (reference) => {
    const t0 = performance.now();
    run({ command_id: "nav.navigate", args: { reference } });
    return (await textPainted()) - t0;
  };
  const warmRefs = ["John.3", "Gen.1", "John.3.16", "Gen.1.2", "Ps.119.105", "Rev.22.21", "Matt.5.3", "Isa.53.5", "Acts.1.8", "Ps.23", "Rom.8.28", "Jude.1.3"];
  for (const r of warmRefs) await openOnce(r);
  await new Promise((ok) => setTimeout(ok, 200)); // what was fetched ahead
  const warm = [];
  for (let i = 0; i < samples; i++) warm.push(await openOnce(warmRefs[i % warmRefs.length]));
  row("open a chapter, warm: chunks already in the core, across the canon (budget 100)", warm);
  // A chapter not yet visited: its chunks come from the server (localhost here).
  const chapters = [];
  const books = host.reading.editions().length && JSON.parse(await (await fetch(`${packBase(params)}packs/bsb/manifest.json`)).text()).books;
  for (const b of books) for (const c of b.chapters) if (!host.chunks.supplied.has(`bsb|${b.book}.${c.chapter}`)) chapters.push(`${b.book}.${c.chapter}`);
  const first = [];
  for (let i = 0; i < samples && i * 4 < chapters.length; i++) first.push(await openOnce(chapters[i * 4]));
  row("open a chapter on first visit: chunk fetched from localhost (budget 100 is for warm)", first);
  // Workspace switch.
  const switches = [];
  for (let i = 0; i < samples; i++) {
    const t0 = performance.now();
    run({ command_id: "workspace.next" });
    switches.push((await textPainted()) - t0);
  }
  row(`workspace switch to first painted text, ${host.session.workspaces.length} workspaces (budget 100)`, switches);
  // Scrolling through chapters that have to be fetched.
  run({ command_id: "column.focus", args: { column: 1 } });
  run({ command_id: "nav.navigate", args: { reference: "Isa.1.1" } });
  await textPainted();
  const view = activeView();
  const scroll = view && view.stream ? await scrollRun(view, Math.max(samples * 3, 600), 90) : [];
  return { user_agent: navigator.userAgent, viewport: [innerWidth, innerHeight], rows: out, scroll, chunks: { ...host.chunks.stats }, search: { timing: host.search.timing, bytes: host.search.bytes } };
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
  // B078 (specs/23 §7): a typed jump anywhere in the canon, the stream rebuilt around it (and
  // its linked followers); then a scripted continuous scroll through dozens of chapters.
  const jumps = ["Ps.119.105", "Gen.1.1", "Rev.22.21", "Matt.5.3", "Isa.53.5", "John.3.16-John.3.18", "Acts.1.8", "Ps.23", "Rom.8.28", "Jude.1.3", "Obad.1.1", "Mal.4.5"];
  run({ command_id: "column.focus", args: { column: 1 } });
  out.push(await measure("B078 typed jump → stream painted (nav.navigate across the canon, followers too)", samples, (i) => run({ command_id: "nav.navigate", args: { reference: jumps[i % jumps.length] } })));
  run({ command_id: "nav.navigate", args: { reference: "Ps.1.1" } });
  await nextFrame();
  const view = activeView();
  if (view && view.stream) out.push(...(await scrollRun(view, Math.max(samples * 3, 600), 90)));
  return { user_agent: navigator.userAgent, dpr: window.devicePixelRatio, viewport: [innerWidth, innerHeight], workspaces: host.session.workspaces.length, tabs: host.session.workspaces.reduce((a, w) => a + w.columns.reduce((b, c) => b + c.tabs.length, 0), 0), rows: out };
}

/** The words a stored span covers, from the reading handle (for the tests). */
function spanText(resourceId, span) {
  try {
    const verse = host.reading.chapterContent(resourceId, chapterOfSegment(span.segment_id)).verses.find((v) => v.segment_id === span.segment_id);
    return verse ? [...verse.text].slice(span.start, span.end).join("") : "";
  } catch { return ""; }
}

// MARK: - B074: the acceptance store and its speed runs

/** Fills the shell (the bench session: ten workspaces of twelve tabs) to the acceptance size
 *  through the API: 200 notes, 1,000 highlights on the fixture verses and 200 strokes of 120
 *  points with pressure across John 3 and Genesis 1. Returns the store's size. */
export function buildAcceptance() {
  let seed = 7;
  const rnd = () => { seed = (seed * 1103515245 + 12345) % 2147483648; return seed / 2147483648; };
  for (let n = 1; n <= 200; n++) {
    host.handle.putNote({ id: `bench:note:${n}`, blocks: [
      { kind: "heading", spans: [{ text: `Study ${n} ` }, { text: "Jn 3:16", link: "John.3.16" }] },
      { kind: "paragraph", spans: [{ text: "A bench note with " }, { text: "bold", bold: true }, { text: " and " }, { text: "italic", italic: true }, { text: " words." }] },
      { kind: "list_item", spans: [{ text: "Compare " }, { text: "Gen 1:1", link: "Gen.1.1" }] },
      { kind: "quote", spans: [{ text: "A bench quote, not Scripture." }] },
    ] });
  }
  const verses = [["bsb", "John.3.16"], ["bsb", "John.3.17"], ["sblgnt", "John.3.16"], ["sblgnt", "John.3.17"], ["wlc", "Gen.1.1"], ["wlc", "Gen.1.2"]].map(([r, id]) => {
    const block = host.reading.chapterContent(r, chapterOfSegment(id));
    const verse = block.verses.find((v) => v.segment_id === id);
    return { r, id, revision: block.content_revision, length: [...verse.text].length, placeholder: verse.placeholder };
  }).filter((v) => !v.placeholder);
  const colours = SWATCH_ORDER.map((k) => TOKENS.swatches[k][0]);
  for (let i = 0; i < 1000; i++) {
    const v = verses[i % verses.length];
    const start = Math.floor(rnd() * (v.length - 8));
    const end = Math.min(v.length, start + 3 + Math.floor(rnd() * 20));
    host.handle.addHighlight({ resource_id: v.r, content_revision: v.revision, spans: [{ segment_id: v.id, start, end }], color: colours[i % 4] }, host.reading);
  }
  for (let i = 0; i < 200; i++) {
    const [r, id] = i % 4 === 3 ? ["wlc", `Gen.1.${1 + (i % 31)}`] : ["bsb", `John.3.${1 + (i % 36)}`];
    const x0 = 20 + rnd() * 300, y0 = rnd() * 20;
    const points = Array.from({ length: 120 }, (_, k) => ({ x: x0 + k * 2.5, y: y0 + Math.sin(k / 8) * 6, pressure: 0.3 + 0.5 * rnd(), elapsed_ms: k * 8 }));
    host.handle.addInk({ resource_id: r, segment_id: id, stroke: { id: "bench", tool: i % 2 ? "highlighter" : "pen", brush_version: i % 2 ? BRUSHES.highlighter.version : BRUSHES.pen.version, width: 4, color: i % 2 ? "#F5D90A" : "#2F5BD3", opacity: i % 2 ? 0.35 : 1, points } });
  }
  highlightsChanged();
  inkChanged();
  render();
  return host.handle.exportStore().length;
}

/** Save and workspace commands at the acceptance size (specs/24 §4, B074 brief). `export` is
 *  the main thread's part of a save (`exportStore` after one change); `save → committed` is
 *  saveNow to IndexedDB's commit, off the main thread but for the call. */
export async function storeBench({ samples = 200 } = {}) {
  closePalette(false);
  const out = [];
  const exportAfter = async (name, change) => {
    const work = [];
    for (let i = 0; i < samples; i++) {
      change(i);
      const t0 = performance.now();
      host.handle.exportStore();
      work.push(performance.now() - t0);
      if (i % 20 === 0) await nextFrame();
    }
    out.push({ name, work_ms: stats(work), frame_ms: stats(work) });
  };
  const ws = activeWorkspace();
  const tab = shownTab(ws.columns[0]);
  await exportAfter("B074 exportStore after a scroll (tab.record_scroll)", (i) => run({ command_id: "tab.record_scroll", args: { workspace_id: ws.id, tab_id: tab.id, segment_id: `John.3.${1 + (i % 30)}`, scalar: 0 } }));
  const added = [];
  await exportAfter("B074 exportStore after a stroke (addInk)", (i) => {
    added.push(host.handle.addInk({ resource_id: "bsb", segment_id: "John.3.20", stroke: { id: "b", tool: "pen", brush_version: BRUSHES.pen.version, width: 4, color: "#2F5BD3", opacity: 1, points: Array.from({ length: 120 }, (_, k) => ({ x: k * 2 + i, y: k % 7, pressure: 0.5, elapsed_ms: k * 8 })) } }));
  });
  for (const id of added) host.handle.removeInk(id);
  inkChanged("bsb", "John.3.20");
  await exportAfter("B074 exportStore after a note edit (putNote)", (i) => host.handle.putNote({ id: "bench:note:1", blocks: [{ kind: "paragraph", spans: [{ text: `Edit ${i}` }] }] }));
  // The whole save as the page does it: export on the main thread, then the IndexedDB put.
  const sync = [], committed = [];
  for (let i = 0; i < Math.min(samples, 100); i++) {
    run({ command_id: "tab.record_scroll", args: { workspace_id: ws.id, tab_id: tab.id, segment_id: `John.3.${1 + (i % 30)}`, scalar: 0 } });
    await nextFrame();
    const t0 = performance.now();
    const done = saveNow();
    sync.push(performance.now() - t0);
    await done;
    committed.push(performance.now() - t0);
  }
  out.push({ name: "B074 save on the main thread (saveNow: export + put queued)", work_ms: stats(sync), frame_ms: stats(sync) });
  out.push({ name: "B074 save → IndexedDB committed (off the main thread)", work_ms: stats(committed), frame_ms: stats(committed) });
  const count = () => host.session.workspaces.length;
  const base = count();
  out.push(await measure(`B074 workspace.new (${base} workspaces at the acceptance size)`, samples, () => run({ command_id: "workspace.new" }), () => { while (count() > base) run({ command_id: "workspace.close" }); }));
  while (count() > base) run({ command_id: "workspace.close" });
  out.push(await measure("B074 workspace.rename", samples, (i) => run({ command_id: "workspace.rename", args: { text: `Bench ${1 + (i % 9)}` } })));
  out.push(await measure("B074 workspace.duplicate", samples, () => run({ command_id: "workspace.duplicate" }), () => { while (count() > base) run({ command_id: "workspace.close" }); }));
  while (count() > base) run({ command_id: "workspace.close" });
  out.push(await measure("B074 workspace.close", samples, () => run({ command_id: "workspace.close" }), () => { run({ command_id: "workspace.duplicate" }); }));
  out.push(await measure("B074 workspace.switch (workspace.next)", samples, () => run({ command_id: "workspace.next" })));
  return { workspaces: count(), bytes: host.handle.exportStore().length, rows: out };
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
  const stored = JSON.parse(host.handle.exportStore());
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
    // B074: the core's highlights (with the words they cover) and strokes per edition.
    highlights: stored.highlights.map((h) => ({ id: h.id, swatch: swatchName(h.color) || h.color, anchors: h.spans.map((x) => ({ segment_id: x.segment_id, exact: spanText(h.resource_id, x) })) })),
    ink: [...new Set([...stored.ink.map((k) => k.resource_id), ...host.erased.keys()])].map((r) => ({ tab: r, strokes: stored.ink.filter((k) => k.resource_id === r).length, erased: host.erased.get(r) || 0, segments: stored.ink.filter((k) => k.resource_id === r).map((k) => k.segment_id) })),
    stored: { notes: stored.notes, changes: host.handle.changeCount(), lastSaved: host.lastSaved, restore: host.restore },
    toast: host.lastToast || null, focus: document.activeElement ? (document.activeElement.id || document.activeElement.className) : null,
    paletteOpen: !results.hidden, log: host.log.slice(-20),
    // B076: the well as drawn, what has focus, the notes as edited, the last copied reference.
    well: {
      context: $("well").dataset.context, effective: host.toolWell.effective_tool, focus: host.focus,
      groups: [...$("well").querySelectorAll(".group")].map((g) => ({ kind: g.dataset.kind, items: [...g.querySelectorAll("button[data-command]")].map((b) => b.dataset.command) })),
      pressed: [...$("well").querySelectorAll('button[aria-pressed="true"]')].map((b) => b.dataset.command),
      top: $("well").getBoundingClientRect().top,
    },
    notes: Object.fromEntries([...document.querySelectorAll(".note-editor")].reverse().map((e) => [e.dataset.note, e.innerHTML])), copied: host.copied || null,
    // B077: the context menu as drawn (every open level), the last target, find and scope.
    contextMenu: contextMenuSnapshot(), target: host.lastTarget || null, selection: document.getSelection().toString(),
    find: host.find ? { count: host.find.ranges.length, index: host.find.index, where: host.find.where, current: host.find.ranges[host.find.index].toString() } : null,
    scope: host.palette.scope ? { ...host.palette.scope, chip: $("scope-chip").firstChild.textContent } : null,
    paletteValue: field.value,
    pack: { editions: [...host.editions].filter((id) => host.text.edition(id)), chunks: { ...host.chunks.stats }, search: host.search && { state: host.search.state, status: host.search.statusLine(), timing: host.search.timing, bytes: host.search.bytes }, reanchors: { ...host.reanchors } },
    // B078: each column's reading stream as rendered.
    reading: ws.columns.map((c) => {
      const view = host.views.get(`${ws.id}/${c.id}`);
      if (!view || !isStream(view.tab)) return null;
      const missing = view.section.querySelector(".not-in-edition");
      if (!view.stream) return { error: missing ? missing.textContent.replace(/\s+/g, " ").trim() : null };
      const st = view.stream;
      return {
        blocks: st.blocks.map((b) => b.key), header: st.header.textContent, top: st.top, scrollTop: view.scroller.scrollTop,
        notice: (view.text.querySelector(".notice") || {}).textContent || null,
        marked: [...view.text.querySelectorAll(".verse.target")].map((v) => v.dataset.segment),
        flashing: [...view.text.querySelectorAll(".verse.flash")].map((v) => v.dataset.segment),
        nodes: view.page.getElementsByTagName("*").length, stats: { ...st.stats },
      };
    }),
  };
}

/** `?store=off` keeps nothing (the tests' default, `autotest=1`); `?store=<name>` uses the
 *  database `openlogos-<name>`; otherwise the page's own `openlogos`. */
function openStore() {
  const name = params.get("store") ?? (params.get("autotest") ? "off" : "");
  host.saveDebounce = core.storeConstants().save_debounce_ms;
  if (name === "off" || typeof indexedDB === "undefined") return null;
  if (!name && navigator.storage && navigator.storage.persist) navigator.storage.persist().catch(() => {});
  return new UserStore(name ? `openlogos-${name}` : "openlogos");
}

/** Reads the saved store into the shell. A store refused whole keeps the seed session, is
 *  moved aside as a backup and said once; one that restored with something set aside is kept
 *  as a backup too, and what was set aside is said. Nothing is due to save afterwards (a
 *  restore that set something aside moved the count, so the repaired store is written). */
async function restoreStore() {
  host.store = openStore();
  if (!host.store) return;
  const t0 = performance.now();
  let json = null, pending = false;
  try {
    json = host.store.pending();
    pending = json != null;
    if (!pending) json = await host.store.read();
  } catch (error) {
    console.warn("the store could not be opened", error);
    host.restore = { t0: null, error: String(error) };
    host.store = null;
    return;
  }
  const read = performance.now() - t0;
  if (json == null) { host.restore = { t0, found: false, read_ms: read }; host.lastSaved = host.handle.changeCount(); return; }
  const before = host.handle.changeCount();
  try {
    const report = host.handle.importStore(json);
    host.restore = { t0, found: true, read_ms: read, import_ms: performance.now() - t0 - read, bytes: json.length, report };
    const text = issuesNotice(report);
    if (text) {
      host.store.backup(json).catch((e) => console.warn("backup failed", e));
      notice(text);
    }
  } catch (error) {
    host.restore = { t0, found: true, read_ms: read, refused: error.kind || String(error), bytes: json.length };
    host.store.setAside(json).catch((e) => console.warn("backup failed", e));
    notice(refusedNotice(error));
  }
  // A clean restore leaves the count where it was saved; one that set something aside moved it.
  host.lastSaved = host.restore.report && host.restore.report.issues.length ? before : host.handle.changeCount();
  host.restore.pending = pending;
  // A copy kept on hide goes into IndexedDB as it is (or, refused, it was set aside above).
  if (pending && !host.restore.refused) host.store.write(json).then(() => host.store.clearPending(), (e) => console.warn("save failed", e));
  else if (pending) host.store.clearPending();
  changed();
}

/** B071: the chunks the active workspace's shown tabs open on, so the first paint has text. */
async function preloadShown() {
  const wanted = [];
  for (const column of activeWorkspace().columns) {
    const tab = shownTab(column);
    if (!isStream(tab) || !tab.reference) continue;
    let key = null, around = null;
    try {
      const anchor = tab.scroll_anchor && chapterOfSegment(tab.scroll_anchor.segment_id);
      key = anchor || host.reading.locate(tab.content_id, tab.reference).chapter_key;
      around = host.reading.chapterNeighbors(tab.content_id, key);
    } catch { continue; }
    for (const k of [key, around.prev, around.next]) if (k) wanted.push(host.chunks.request(tab.content_id, k));
  }
  await Promise.allSettled(wanted);
}

async function main() {
  await init();
  host.text = fixtureTextSource(core);
  // B071: the preview pack's manifests, over the placeholder editions of the same ids. Where a
  // pack is not found the placeholder stays, and says so on every block.
  host.reading = core.ReadingHandle.placeholder();
  const base = packBase(params);
  const packs = await installPacks(host.reading, base);
  for (const r of packs.refused) console.warn(`pack ${r.id} refused: ${r.reason}`);
  host.text.useEditions(packs.installed);
  host.chunks = new Chunks(host.reading, base, packs.installed.map((e) => e.resource_id));
  host.registry = core.shellRegistry();
  host.specs = new Map(host.registry.map((s) => [s.id, s]));
  host.bindings = bindingsFrom(host.registry);
  host.layout = core.layoutConstants();
  // `?theme=` seeds the session's reading theme (for captures); otherwise it follows the system.
  const theme = params.get("theme");
  const session = params.get("session") === "bench" ? benchSession() : seedSession();
  if (TOKENS.themes.includes(theme)) session.reading.theme = theme;
  host.handle = new core.ShellHandle(session);
  // B071: with packs the Text section comes from the search worker; without, from the fixtures.
  if (!packs.installed.length) host.handle.useFixtureText();
  host.handle.setResourceTitles(host.text.titles());
  host.handle.setResourceLanguages(host.text.languages());
  // B078: reading tabs take their text from the reading handle (installed above), and linked
  // tabs follow through each edition's versification.
  host.editions = new Set(host.reading.editions().map((e) => e.resource_id));
  host.handle.setResourceSchemes(host.reading.schemes());
  // B074 (specs/24 §5): the saved store over the seed session, then the active workspace drawn.
  await restoreStore();
  sync();
  await preloadShown();
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => { if (host.session.reading.theme === "system") render(); });
  buildWell();
  render();
  // B071: from the navigation to the restored workspace's text painted (specs/22 §7).
  host.launch = { rendered_ms: performance.now() };
  textPainted().then((t) => { host.launch.painted_ms = t; host.launch.resources = performance.getEntriesByType("resource").map((e) => ({ name: e.name.split("/").slice(-2).join("/"), ms: +e.duration.toFixed(1), bytes: e.decodedBodySize })); });
  if (host.restore && host.restore.t0 != null) {
    const r = host.restore;
    r.rendered_ms = performance.now() - r.t0;
    requestAnimationFrame(() => { r.first_paint_ms = performance.now() - r.t0; });
  }
  // B071: the search packs arrive after the text is on screen, in their worker.
  if (packs.installed.length) {
    host.search = new TextSearch({
      base, editions: packs.installed.map((e) => e.resource_id), titles: host.text.titles(),
      onChange: textStateChanged, onResult: (_, group) => textAnswered(group),
    });
    if (params.get("search") !== "manual") setTimeout(() => host.search.start(), 0);
  }
  $("scope-chip").lastChild.addEventListener("click", () => { host.palette.scope = null; field.focus(); queryPalette(); });
  window.openlogosShell = { core, host, benchPack, run, saveNow, storeBench, buildAcceptance, render, selectionChanged, snapshot, bench, keyReport, menuReport, queryPalette, focusPalette, closePalette, tokens: TOKENS, activeWorkspace, activeTab, showContextMenu, closeContextMenu };
  window.openlogosReady = true;
}

main().catch((error) => {
  console.error(error);
  window.openlogosError = String(error && error.stack || error);
  document.body.insertAdjacentHTML("beforeend", `<p style="position:fixed;bottom:0;left:0;right:0;padding:12px;background:#fee;color:#900;font:13px system-ui">failed: ${escapeHtml(error.message || error)}</p>`);
});
