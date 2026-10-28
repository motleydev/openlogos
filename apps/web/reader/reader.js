// The DOM reader (B005): semantic HTML text, the browser's Selection, and the Rust core as
// WebAssembly for anchors. The round trip is the same as the TextKit 2 surface's:
//
//   Selection/Range → UTF-16 offsets over the core text of the paragraph (verse markers are
//   not core text; an end inside one collapses to its edge) → selectionToAnchor() → one anchor
//   per segment → anchorToUtf16() → a DOM Range over the same characters → painted with the
//   CSS Custom Highlight API (Chromium 105+, Safari 17.2+, Firefox 140+), or with positioned
//   overlays where that API is missing.
//
// A range that ends inside a grapheme cluster (programmatic; the browser's own caret never
// stops there) is widened to cluster boundaries with Intl.Segmenter, the platform's notion of
// clusters, and the readout says so. `?case=<name>` selects one case of CASES at load and
// `?autorun=1` runs them all and leaves the results on window.openlogosResults for the
// headless test (test/run-chrome.mjs).
import init, * as core from "../core-host/pkg/openlogos_ffi.js";

const JOINER = " ";
const segmenter = typeof Intl !== "undefined" && Intl.Segmenter ? new Intl.Segmenter("und", { granularity: "grapheme" }) : null;

/** The selections the evidence drives, matching the Apple AnchorCase table where the texts agree. */
export const CASES = [
  { name: "cross-verse", pane: "p1", core: [113, 164], expect: [
    { segment_id: "John.3.16", exact: "but have eternal life.", tokens: ["bsb:44003016024", "bsb:44003016025", "bsb:44003016026", "bsb:44003016027"] },
    { segment_id: "John.3.17", exact: "For God did not send His Son", tokens: ["bsb:44003017001", "bsb:44003017002", "bsb:44003017003", "bsb:44003017004", "bsb:44003017005", "bsb:44003017006", "bsb:44003017007"] },
  ] },
  { name: "bsb-eternal-life", pane: "p1", core: [122, 135], expect: [{ segment_id: "John.3.16", exact: "eternal life.", tokens: ["bsb:44003016026", "bsb:44003016027"] }] },
  { name: "hebrew-elohim", pane: "p2", core: [21, 30], expect: [{ segment_id: "Gen.1.1", exact: "אֱלֹהִ֑ים", tokens: ["wlc:01001001003"] }] },
  { name: "hebrew-cross-verse", pane: "p2", core: [56, 70], expect: [
    { segment_id: "Gen.1.1", exact: "הָאָֽרֶץ׃", tokens: ["wlc:01001001007"] },
    { segment_id: "Gen.1.2", exact: "וְהָ", tokens: ["wlc:01001002001"] },
  ] },
  // Inside the first cluster (bet + sheva + dagesh): a DOM Range can be put there; the reader widens it.
  // The expected text is sliced from the fixture (non-NFC, byte-exact), not written here.
  { name: "hebrew-inside-cluster", pane: "p2", core: [1, 3], snaps: true, expect: [{ segment_id: "Gen.1.1", slice: [0, 3], tokens: ["wlc:01001001001"] }] },
  { name: "hebrew-maqqef", pane: "p2", core: [117, 118], expect: [{ segment_id: "Gen.1.2", exact: "־", tokens: [] }] },
  { name: "greek-monogene", pane: "p3", phrase: "μονογενῆ", expect: [{ segment_id: "John.3.16", exact: "μονογενῆ", tokens: ["sblgnt:44003016012"] }] },
  { name: "greek-diacritics-inside", pane: "p3", phrase: "ἠγάπησεν", expect: [{ segment_id: "John.3.16", exact: "ἠγάπησεν", tokens: ["sblgnt:44003016003"] }] },
  { name: "greek-two-words", pane: "p3", phrase: "ζωὴν αἰώνιον.", expect: [{ segment_id: "John.3.16", exact: "ζωὴν αἰώνιον.", tokens: ["sblgnt:44003016024", "sblgnt:44003016025"] }] },
  { name: "note-emoji", pane: "note", core: [126, 130], expect: [{ segment_id: "note-1", exact: "👍🏽", tokens: [] }] },
  { name: "note-emoji-split", pane: "note", core: [128, 130], snaps: true, expect: [{ segment_id: "note-1", exact: "👍🏽", tokens: [] }] },
  { name: "note-isolated-word", pane: "note", core: [19, 33], expect: [{ segment_id: "note-1", slice: [19, 33], tokens: [] }] },
  { name: "note-compare-to-end", pane: "note", core: [88, 131], expect: [{ segment_id: "note-1", exact: "Compare ⁨אֱלֹהִ֑ים⁩ in the same verse 👍🏽.", tokens: [] }] },
];

// MARK: - Panes from the core's fixtures

function textFixture(fixtures, id) {
  return fixtures.find((f) => f.id === id && f.kind === "text");
}

function panesFrom(fixtures) {
  const bsb = textFixture(fixtures, "cross-verse-selection-john-3");
  const wlc = textFixture(fixtures, "hebrew-cantillation-genesis-1");
  const note = textFixture(fixtures, "bidi-isolation-note");
  const alignment = fixtures.find((f) => f.id === "alignment-john-3-16-sblgnt-bsb");
  const greek = alignment && alignment.source_segment;
  const panes = [];
  if (bsb) panes.push(pane("p1", "BSB", "John 3:16–17", bsb, "ltr", "en", "Group 1"));
  if (wlc) panes.push(pane("p2", "WLC", "בראשית א׳ 1–2", wlc, "rtl", "he", "Group 1"));
  if (greek) panes.push(pane("p3", "SBLGNT", "Κατὰ Ἰωάννην 3:16", {
    edition_id: greek.edition_id, content_revision: greek.content_revision, license: alignment.license,
    segments: [{ segment_id: greek.segment_id, text: greek.text, tokens: greek.tokens }],
  }, "ltr", "grc", "Pinned"));
  const dock = note ? pane("note", "Note", "Note on Genesis 1:1", note, "ltr", "en", "") : null;
  return { panes, dock };
}

function pane(id, shortName, reference, fixture, dir, lang, link) {
  const license = (fixture.license || "").split(" ").filter((w) => !w.startsWith("http")).join(" ").replace(/[,;: ]+$/, "");
  return {
    id, shortName, reference, dir, lang, link, attribution: `Fixture: ${license}`,
    paragraph: {
      edition_id: fixture.edition_id,
      content_revision: fixture.content_revision,
      joiner: JOINER,
      segments: fixture.segments.map((s) => ({
        segment_id: s.segment_id,
        text: s.text,
        tokens: (s.tokens || []).map((t) => ({ id: core.parseTokenId(t.id), range: { start: t.start, end: t.end } })),
      })),
    },
  };
}

/** Segments joined by the joiner: what the core measures on; and where each segment starts. */
function coreTextOf(paragraph) {
  const offsets = {};
  let text = "";
  paragraph.segments.forEach((s, i) => {
    if (i > 0) text += paragraph.joiner;
    offsets[s.segment_id] = text.length;
    text += s.text;
  });
  return { text, offsets };
}

// MARK: - Rendering

function render(p, isDock) {
  const section = document.createElement(isDock ? "aside" : "section");
  section.className = isDock ? "dock" : "pane";
  section.dataset.pane = p.id;
  section.setAttribute("aria-label", isDock ? "Notes dock" : `Pane ${p.shortName}`);
  if (isDock) {
    const h = document.createElement("h2");
    h.textContent = "Notes";
    section.appendChild(h);
  } else {
    const identity = document.createElement("div");
    identity.className = "identity";
    identity.innerHTML = `<b>${p.shortName}</b> <span class="ref" dir="auto">${p.reference}</span> <span class="link">${p.link}</span>`;
    section.appendChild(identity);
  }
  const text = document.createElement("p");
  text.className = "text";
  text.dir = p.dir;
  text.lang = p.lang;
  p.paragraph.segments.forEach((s, i) => {
    if (i > 0) text.appendChild(document.createTextNode(p.paragraph.joiner));
    const verse = document.createElement("span");
    verse.className = "verse";
    verse.dataset.segment = s.segment_id;
    const number = s.segment_id.split(".").pop();
    if (/^\d+$/.test(number)) {
      const marker = document.createElement("sup");
      marker.className = "marker";
      marker.textContent = number;
      marker.setAttribute("aria-label", `verse ${number}`);
      verse.appendChild(marker);
    }
    const t = document.createElement("span");
    t.className = "t";
    t.lang = p.lang;
    t.textContent = s.text;
    verse.appendChild(t);
    text.appendChild(verse);
  });
  section.appendChild(text);
  const attribution = document.createElement("p");
  attribution.className = "attribution";
  attribution.textContent = p.attribution;
  section.appendChild(attribution);
  if (!isDock) {
    const actions = document.createElement("div");
    actions.className = "actions";
    actions.hidden = true;
    actions.setAttribute("role", "group");
    actions.setAttribute("aria-label", "Selection actions");
    for (const label of ["Highlight", "Collect", "Study", "More"]) {
      const b = document.createElement("button");
      b.type = "button";
      b.textContent = label;
      actions.appendChild(b);
    }
    section.appendChild(actions);
  }
  const readout = document.createElement("output");
  readout.className = "readout";
  readout.setAttribute("aria-live", "polite");
  section.appendChild(readout);
  return { section, text, readout, actions: section.querySelector(".actions") };
}

// MARK: - Offsets between the DOM and the core text

function isCore(node) {
  return !(node.parentElement && node.parentElement.closest(".marker"));
}

function* textNodes(root) {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  let node;
  while ((node = walker.nextNode())) yield node;
}

/** Core UTF-16 offset of a DOM point: the core text length before it, markers excluded. */
function coreOffset(root, container, offset) {
  const point = document.createRange();
  point.setStart(container, offset);
  point.collapse(true);
  let total = 0;
  for (const node of textNodes(root)) {
    if (!isCore(node)) continue;
    const probe = document.createRange();
    probe.selectNodeContents(node);
    // compareBoundaryPoints(how, source) compares this range's point named second in `how`
    // with the source's point named first: START_TO_END is this.end against point.start.
    if (probe.compareBoundaryPoints(Range.START_TO_END, point) <= 0) { total += node.length; continue; } // node ends at or before the point
    if (probe.compareBoundaryPoints(Range.START_TO_START, point) >= 0) break; // node starts at or after the point
    total += container === node ? offset : 0;
    break;
  }
  return total;
}

/** The DOM point of a core offset; a start lands after a marker, an end before it. */
function domPoint(root, offset, isStart) {
  let total = 0;
  let last = null;
  for (const node of textNodes(root)) {
    if (!isCore(node)) continue;
    const end = total + node.length;
    if (isStart ? offset >= total && offset < end : offset > total && offset <= end) return { node, offset: offset - total };
    if (isStart && offset === total && node.length === 0) return { node, offset: 0 };
    total = end;
    last = node;
  }
  if (last && offset === total) return { node: last, offset: last.length };
  if (!last) return { node: root, offset: 0 };
  return isStart ? { node: last, offset: last.length } : { node: last, offset: last.length };
}

function snapToClusters(text, start, end) {
  if (!segmenter) return [start, end];
  let s = start, e = end;
  for (const { index, segment } of segmenter.segment(text)) {
    const stop = index + segment.length;
    if (index < start && start < stop) s = index;
    if (index < end && end < stop) e = stop;
    if (index > end) break;
  }
  return [s, e];
}

// MARK: - Painting

const highlightName = "openlogos-anchor";
const highlightApi = typeof CSS !== "undefined" && "highlights" in CSS;
let overlays = [];

function paint(ranges) {
  if (highlightApi) {
    CSS.highlights.set(highlightName, new Highlight(...ranges));
    return "css-highlight";
  }
  for (const o of overlays) o.remove();
  overlays = [];
  for (const range of ranges) {
    for (const rect of range.getClientRects()) {
      const o = document.createElement("div");
      o.className = "overlay";
      o.style.left = `${rect.left + window.scrollX}px`;
      o.style.top = `${rect.top + window.scrollY}px`;
      o.style.width = `${rect.width}px`;
      o.style.height = `${rect.height}px`;
      document.body.appendChild(o);
      overlays.push(o);
    }
  }
  return "overlay";
}

// MARK: - The round trip

const state = { panes: new Map(), current: null };

/** Runs the round trip for a DOM range inside a pane's text; returns the readout record. */
function roundTrip(paneState, range) {
  const { pane, text } = paneState;
  const started = performance.now();
  const { text: coreText, offsets } = coreTextOf(pane.paragraph);
  let start = coreOffset(text, range.startContainer, range.startOffset);
  let end = coreOffset(text, range.endContainer, range.endOffset);
  const [s, e] = snapToClusters(coreText, start, end);
  const snapped = s !== start || e !== end;
  start = s; end = e;
  if (end <= start) return null;
  const anchors = core.selectionToAnchor(pane.paragraph, start, end);
  const painted = [];
  const ranges = [];
  for (const anchor of anchors) {
    const segment = pane.paragraph.segments.find((x) => x.segment_id === anchor.segment_id);
    const back = core.anchorToUtf16(anchor, segment.text);
    const from = offsets[anchor.segment_id] + back.start;
    const to = offsets[anchor.segment_id] + back.end;
    const r = document.createRange();
    const a = domPoint(text, from, true);
    const b = domPoint(text, to, false);
    r.setStart(a.node, a.offset);
    r.setEnd(b.node, b.offset);
    ranges.push(r);
    painted.push(r.toString());
  }
  const elapsed = performance.now() - started;
  const method = paint(ranges);
  const quoted = anchors.map((a) => (a.quote ? a.quote.exact : ""));
  const selected = pane.paragraph.segments.map((seg) => {
    const segStart = offsets[seg.segment_id];
    const lo = Math.max(segStart, start), hi = Math.min(segStart + seg.text.length, end);
    return hi > lo ? coreText.slice(lo, hi) : null;
  }).filter((x) => x !== null);
  const identical = JSON.stringify(quoted) === JSON.stringify(painted) && JSON.stringify(quoted) === JSON.stringify(selected);
  return {
    pane: pane.id, core: [start, end], snapped, identical, elapsed_ms: elapsed, paint: method,
    anchors: anchors.map((a) => ({ segment_id: a.segment_id, range: [a.range.start, a.range.end], exact: a.quote ? a.quote.exact : "", tokens: a.tokens.map(core.formatTokenId) })),
    painted,
  };
}

function describe(result) {
  const lines = result.anchors.map((a) => `${a.segment_id} scalars [${a.range[0]}, ${a.range[1]}) → “${a.exact}” · ${a.tokens.length === 0 ? "no tokens" : a.tokens.length === 1 ? a.tokens[0] : `${a.tokens[0]} … ${a.tokens[a.tokens.length - 1]} (${a.tokens.length})`}`);
  lines.push(`${result.identical ? "round trip identical" : "ROUND TRIP DIFFERS"} · core [${result.core[0]}, ${result.core[1]}) → ${result.anchors.length} highlight${result.anchors.length === 1 ? "" : "s"} (${result.paint})${result.snapped ? " · widened to grapheme clusters" : ""} · ${result.elapsed_ms.toFixed(2)} ms`);
  return lines.join("\n");
}

function show(paneState, result) {
  state.current = result;
  paneState.readout.textContent = result ? describe(result) : "";
  paneState.readout.classList.toggle("differs", !!result && !result.identical);
  if (paneState.actions) paneState.actions.hidden = !result;
}

function onSelectionChange() {
  const selection = document.getSelection();
  if (!selection || selection.rangeCount === 0 || selection.isCollapsed) return;
  const range = selection.getRangeAt(0);
  for (const paneState of state.panes.values()) {
    if (!paneState.text.contains(range.commonAncestorContainer) && range.commonAncestorContainer !== paneState.text) continue;
    try {
      const result = roundTrip(paneState, range);
      for (const other of state.panes.values()) if (other !== paneState) show(other, null);
      show(paneState, result);
    } catch (error) {
      paneState.readout.textContent = `core error: ${error.message} (${error.kind || ""})`;
      paneState.readout.classList.add("differs");
    }
    return;
  }
}

/** Selects a case through the real Selection (so selectionchange runs the same path). */
export function runCase(c) {
  const paneState = state.panes.get(c.pane);
  if (!paneState) return { name: c.name, error: `no pane ${c.pane}` };
  const { text: coreText } = coreTextOf(paneState.pane.paragraph);
  let [start, end] = c.core || [0, 0];
  if (c.phrase) { start = coreText.indexOf(c.phrase); end = start + c.phrase.length; }
  if (start < 0) return { name: c.name, error: `phrase not found: ${c.phrase}` };
  const a = domPoint(paneState.text, start, true);
  const b = domPoint(paneState.text, end, false);
  const range = document.createRange();
  range.setStart(a.node, a.offset);
  range.setEnd(b.node, b.offset);
  const selection = document.getSelection();
  selection.removeAllRanges();
  selection.addRange(range);
  // What the browser kept of the range it was given (it may refuse a point inside a cluster).
  const kept = selection.rangeCount ? [coreOffset(paneState.text, selection.getRangeAt(0).startContainer, selection.getRangeAt(0).startOffset), coreOffset(paneState.text, selection.getRangeAt(0).endContainer, selection.getRangeAt(0).endOffset)] : null;
  let result;
  try {
    result = roundTrip(paneState, selection.rangeCount ? selection.getRangeAt(0) : range);
  } catch (error) {
    return { name: c.name, asked: [start, end], kept, error: `${error.message} (${error.kind || ""})` };
  }
  for (const other of state.panes.values()) if (other !== paneState) show(other, null);
  show(paneState, result);
  const problems = [];
  if (!result) problems.push("no selection");
  else {
    if (result.anchors.length !== c.expect.length) problems.push(`expected ${c.expect.length} anchors, got ${result.anchors.length}`);
    c.expect.forEach((want, i) => {
      const got = result.anchors[i];
      if (!got) return;
      if (got.segment_id !== want.segment_id) problems.push(`segment ${got.segment_id} ≠ ${want.segment_id}`);
      const segment = paneState.pane.paragraph.segments.find((x) => x.segment_id === want.segment_id);
      const exact = want.slice && segment ? segment.text.slice(want.slice[0], want.slice[1]) : want.exact;
      if (got.exact !== exact) problems.push(`exact “${got.exact}” ≠ “${exact}” (code points ${[...got.exact].map((c) => c.codePointAt(0).toString(16)).join(" ")} vs ${[...exact].map((c) => c.codePointAt(0).toString(16)).join(" ")})`);
      if (want.tokens && JSON.stringify(got.tokens) !== JSON.stringify(want.tokens)) problems.push(`tokens ${JSON.stringify(got.tokens)} ≠ ${JSON.stringify(want.tokens)}`);
    });
    if (!result.identical) problems.push("round trip not identical");
    const widened = result.snapped || (kept && (kept[0] !== start || kept[1] !== end));
    if (!!widened !== !!c.snaps) problems.push(`widened ${!!widened} (browser kept ${JSON.stringify(kept)}, reader snapped ${result.snapped}), expected ${!!c.snaps}`);
  }
  return { name: c.name, asked: [start, end], kept, passed: problems.length === 0, problems, result };
}

export function runAllCases() {
  const results = CASES.map(runCase);
  document.getSelection().removeAllRanges();
  paint([]);
  for (const p of state.panes.values()) show(p, null);
  return results;
}

// MARK: - Boot

async function main() {
  const status = document.getElementById("status");
  await init();
  document.getElementById("core").textContent = `core API ${core.apiVersion()} · ${highlightApi ? "CSS Custom Highlight API" : "overlay highlights"} · ${segmenter ? "Intl.Segmenter" : "no Intl.Segmenter"}`;
  const fixtures = JSON.parse(core.conformanceFixturesJson());
  const { panes, dock } = panesFrom(fixtures);
  const main = document.getElementById("main");
  for (const p of panes) {
    const view = render(p, false);
    main.appendChild(view.section);
    state.panes.set(p.id, { pane: p, ...view });
  }
  if (dock) {
    const view = render(dock, true);
    main.appendChild(view.section);
    state.panes.set(dock.id, { pane: dock, ...view });
  }
  document.addEventListener("selectionchange", onSelectionChange);
  status.textContent = `${panes.length} panes + notes dock · ${navigator.userAgent}`;
  const params = new URLSearchParams(location.search);
  window.openlogos = { core, CASES, runCase, runAllCases, state };
  if (params.get("autorun")) {
    window.openlogosResults = runAllCases();
    status.textContent += ` · ${window.openlogosResults.filter((r) => r.passed).length}/${CASES.length} cases passed`;
  }
  const named = params.get("case");
  if (named) {
    const c = CASES.find((x) => x.name === named);
    if (c) window.openlogosResults = [runCase(c)];
  }
  window.openlogosReady = true;
}

main().catch((error) => {
  document.getElementById("status").textContent = `failed: ${error.message}`;
  window.openlogosError = String(error);
});
