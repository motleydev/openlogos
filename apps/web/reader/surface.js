// The DOM reader's text surface (B005), shared by the reader spike (reader.js) and the workspace
// shell (B072, ../shell): a paragraph of core segments as semantic HTML, and the mapping between
// DOM points and UTF-16 offsets over the core text (verse markers are not core text; an end
// inside one collapses to its edge). No page state.

export const JOINER = " ";
export const segmenter = typeof Intl !== "undefined" && Intl.Segmenter ? new Intl.Segmenter("und", { granularity: "grapheme" }) : null;

/** Segments joined by the joiner: what the core measures on; and where each segment starts. */
export function coreTextOf(paragraph) {
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

/** The paragraph as semantic HTML: one span per segment, its verse number as a marker that is
 *  not core text (`isCore`), the segment text in its own span. */
export function renderParagraph(paragraph, dir, lang) {
  const text = document.createElement("p");
  text.className = "text";
  text.dir = dir;
  text.lang = lang;
  paragraph.segments.forEach((s, i) => {
    if (i > 0) text.appendChild(document.createTextNode(paragraph.joiner));
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
    t.lang = lang;
    t.textContent = s.text;
    verse.appendChild(t);
    text.appendChild(verse);
  });
  return text;
}

// MARK: - Offsets between the DOM and the core text

export function isCore(node) {
  return !(node.parentElement && node.parentElement.closest(".marker"));
}

export function* textNodes(root) {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  let node;
  while ((node = walker.nextNode())) yield node;
}

/** Core UTF-16 offset of a DOM point: the core text length before it, markers excluded. */
export function coreOffset(root, container, offset) {
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
export function domPoint(root, offset, isStart) {
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

export function snapToClusters(text, start, end) {
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

// MARK: - Selection to anchors and back

/** The round trip of a DOM range inside `root` (the rendered paragraph): core UTF-16 offsets,
 *  widened to grapheme clusters → `selectionToAnchor` → one anchor per segment →
 *  `anchorToUtf16` → a DOM Range per anchor over the same characters, for painting.
 *  Null when the range holds no core text. */
export function selectionAnchors(core, paragraph, root, range) {
  const { text: coreText, offsets } = coreTextOf(paragraph);
  let start = coreOffset(root, range.startContainer, range.startOffset);
  let end = coreOffset(root, range.endContainer, range.endOffset);
  const [s, e] = snapToClusters(coreText, start, end);
  const snapped = s !== start || e !== end;
  start = s; end = e;
  if (end <= start) return null;
  const anchors = core.selectionToAnchor(paragraph, start, end);
  return { start, end, snapped, anchors, ranges: anchors.map((anchor) => anchorRange(core, paragraph, root, anchor, offsets)) };
}

/** A DOM Range over an anchor's characters in the rendered paragraph. */
export function anchorRange(core, paragraph, root, anchor, offsets = coreTextOf(paragraph).offsets) {
  const segment = paragraph.segments.find((x) => x.segment_id === anchor.segment_id);
  const back = core.anchorToUtf16(anchor, segment.text);
  const from = offsets[anchor.segment_id] + back.start;
  const to = offsets[anchor.segment_id] + back.end;
  const r = document.createRange();
  const a = domPoint(root, from, true);
  const b = domPoint(root, to, false);
  r.setStart(a.node, a.offset);
  r.setEnd(b.node, b.offset);
  return r;
}
