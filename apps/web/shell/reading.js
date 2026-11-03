// Continuous reading on the web (B078, specs/23 §7): a reading tab is one scroll through its
// edition, rendered as a window of chapter blocks from the core's ReadingHandle
// (`chapterContent`, each block carrying its `prev` and `next`). The block in view and two each
// side are kept (more while they are shorter than one and a half viewports, never more than
// nine); a block inserted or released above the reader shifts `scrollTop` by its height, so
// the verse under the eye stays where it was (anchored to a segment, not to pixels; the
// browser's own scroll anchoring is off for the scroller). A sticky header names the block at
// the top. The stream reports the verse at the top as it changes (live following) and again
// with its scalar when scrolling settles; scrolls it was told to make are not reported.
//
// Block markup: `section.chapter[data-chapter]` with the book title on a book's first chapter,
// the chapter heading, a one-line notice when the block holds placeholder verses, then
// `div.verses`, the block's core text: paragraphs (`p.para`, `p.line` for poetry,
// `p.superscription` for a Psalm title) of `span.verse[data-segment]` with the B005 marker and
// `.t` text span, the joiner between every two verses, so selection anchors (one core
// paragraph per block) and the context menu work as they did on one passage.
import { JOINER } from "../reader/surface.js";

export const PLACEHOLDER_NOTICE = "Placeholder text — this edition is not installed";
export const PLACEHOLDER_REASON = "Placeholder text can’t be quoted or collected";
/** Chapters kept each side of the one in view; the cap on blocks; how far (in viewports) the
 *  rendered text reaches past either edge before more is loaded. */
export const WINDOW = { eachSide: 2, cap: 9, reach: 1.5 };
/** The reading line, this far below the sticky header: a typed passage lands on it (a sliver
 *  of context shows above), the verse on it is the tab's position, a follower's verse is put
 *  on it. */
export const READING_LINE = 24;
const SETTLE_MS = 150;

/** `Book.C.V` → [book, chapter, verse]; a Psalm title is verse 0, a bridge its first verse. */
export function versePoint(segmentId) {
  const m = /^([A-Za-z0-9]+)\.(\d+)\.(\d+)/.exec(segmentId || "");
  return m ? [m[1], Number(m[2]), Number(m[3])] : null;
}

/** The chapter key (`John.3`) of a segment or reference. */
export function chapterOfSegment(segmentId) {
  const m = /^([A-Za-z0-9]+)\.(\d+)/.exec(segmentId || "");
  return m ? `${m[1]}.${m[2]}` : null;
}

/** Whether a verse lies in an OSIS range (`John.3.16-John.3.18`, across chapters too). */
function inRange(range, segmentId) {
  if (!range) return false;
  const [a, b = a] = range.split("-");
  const s = versePoint(a), e = versePoint(b), v = versePoint(segmentId);
  if (!s || !e || !v || v[0] !== s[0] || v[0] !== e[0]) return false;
  const key = (p) => p[1] * 1000 + p[2];
  return key(v) >= key(s) && key(v) <= key(e);
}

/** The core paragraph of a block, the shape `selectionToAnchor` takes. */
function paragraphOf(core, block) {
  return {
    edition_id: block.resource_id, content_revision: block.content_revision, joiner: JOINER,
    segments: block.verses.map((v) => ({
      segment_id: v.segment_id, text: v.text,
      tokens: v.tokens.map((t) => ({ id: core.parseTokenId(t.id), range: { start: t.start, end: t.end } })),
    })),
  };
}

/** The chapter block as DOM. */
function renderBlock(block, resource, mark) {
  const section = document.createElement("section");
  section.className = "chapter";
  section.dataset.chapter = block.chapter_key;
  if (block.heading.book_title) {
    const h = document.createElement("h2");
    h.className = "book-title";
    h.textContent = block.heading.book_title;
    section.appendChild(h);
  }
  const h = document.createElement("h3");
  h.className = "chapter-heading";
  h.textContent = block.heading.chapter_label;
  section.appendChild(h);
  if (block.placeholder) {
    const notice = document.createElement("p");
    notice.className = "notice";
    notice.textContent = PLACEHOLDER_NOTICE;
    section.appendChild(notice);
  }
  const verses = document.createElement("div");
  verses.className = "verses";
  verses.dir = resource.dir;
  verses.lang = resource.lang;
  let p = null;
  let kind = null;
  block.verses.forEach((v, i) => {
    const want = v.superscription ? "superscription" : v.poetry ? "line" : "para";
    // A poetry verse is a line of its own; prose runs on until the next paragraph start.
    if (!p || want !== kind || v.paragraph_start || want !== "para") {
      if (p && i > 0) p.appendChild(document.createTextNode(JOINER));
      p = document.createElement("p");
      p.className = want;
      verses.appendChild(p);
      kind = want;
    } else {
      p.appendChild(document.createTextNode(JOINER));
    }
    const verse = document.createElement("span");
    verse.className = v.placeholder ? "verse placeholder-verse" : "verse";
    if (inRange(mark, v.segment_id)) verse.classList.add("target");
    verse.dataset.segment = v.segment_id;
    if (v.label) {
      const marker = document.createElement("sup");
      marker.className = "marker";
      marker.textContent = v.label;
      marker.setAttribute("aria-label", `verse ${v.label}`);
      verse.appendChild(marker);
    }
    const t = document.createElement("span");
    t.className = "t";
    t.textContent = v.text;
    verse.appendChild(t);
    p.appendChild(verse);
  });
  section.appendChild(verses);
  return { section, verses };
}

export class ReadingStream {
  /** `text`: the stream root (`.text`, inside the column's `.measure`); `header`: the sticky
   *  bar; `onTop(anchor, settled)`: the reader moved the verse at the top; `onBlocks()`:
   *  blocks were loaded or released (highlights and ink repaint); `onFrame(ms)`: one scroll
   *  frame's work, for the speed runs. */
  constructor({ core, reading, resource, scroller, text, header, onTop, onBlocks, onFrame }) {
    Object.assign(this, { core, reading, resource, scroller, text, header, onTop, onBlocks, onFrame });
    this.blocks = []; // { key, prev, next, data, section, verses, paragraph }
    this.top = null; // { segment_id, scalar }: the verse at the top as last measured
    this.reported = null; // the segment last reported (live) and the anchor on settle
    this.expected = null; // scrollTop after a scroll we made; equal means not the reader's
    this.mark = null;
    this.headerLabel = "";
    this.stats = { prepends: 0, appends: 0, releases: 0 };
    this.scheduled = false;
    scroller.style.overflowAnchor = "none";
    scroller.addEventListener("scroll", () => this.scrolled(), { passive: true });
  }

  // MARK: - Blocks

  load(key) {
    const data = this.reading.chapterContent(this.resource.id, key);
    const { section, verses } = renderBlock(data, this.resource, this.mark);
    return { key, prev: data.prev, next: data.next, data, section, verses, paragraph: paragraphOf(this.core, data) };
  }

  blockOf(node) {
    return this.blocks.find((b) => b.section.contains(node)) || null;
  }

  verseEl(segmentId) {
    return this.text.querySelector(`[data-segment="${CSS.escape(segmentId)}"]`);
  }

  /** The reading line in viewport y: below the sticky header by READING_LINE. */
  edge() {
    return this.scroller.getBoundingClientRect().top + this.header.offsetHeight + READING_LINE;
  }

  /** The block at the reading line. */
  blockAtTop() {
    const y = this.edge() + 2;
    for (const b of this.blocks) if (b.section.getBoundingClientRect().bottom > y) return b;
    return this.blocks[this.blocks.length - 1] || null;
  }

  /** Opens the window at `chapterKey` and puts `segmentId` (the line holding its `scalar`) on
   *  the reading line. */
  open(chapterKey, { segmentId, scalar = 0, mark = this.mark, flash = false } = {}) {
    this.mark = mark;
    for (const b of this.blocks) b.section.remove();
    const first = this.load(chapterKey);
    this.blocks = [first];
    this.text.appendChild(first.section);
    // One each side straight away so the jump has text above and below; fill() does the rest.
    if (first.prev) { const b = this.load(first.prev); this.blocks.unshift(b); this.text.insertBefore(b.section, first.section); }
    if (first.next) { const b = this.load(first.next); this.blocks.push(b); this.text.appendChild(b.section); }
    this.scrollToSegment(segmentId || first.data.verses[0].segment_id, scalar);
    this.fill();
    this.top = this.measureTop();
    this.reported = this.top && { live: this.top.segment_id, settled: this.top };
    this.updateHeader();
    if (this.onBlocks) this.onBlocks();
    if (flash) this.flash();
  }

  /** Scrolls a rendered segment to the reading line (a scroll we make, never reported). A
   *  bridge or a verse the edition numbers differently lands on the verse that holds it. */
  scrollToSegment(segmentId, scalar = 0) {
    const verse = this.verseEl(segmentId) || this.holding(segmentId);
    if (!verse) return false;
    let top = verse.getBoundingClientRect().top;
    const node = verse.querySelector(".t").firstChild;
    if (node && scalar > 0) {
      const r = document.createRange();
      const at = [...node.data].slice(0, scalar).join("").length;
      r.setStart(node, Math.min(at, node.length));
      r.setEnd(node, Math.min(at + 1, node.length));
      top = r.getBoundingClientRect().top;
    }
    this.scroller.scrollTop += top - this.edge();
    this.expected = this.scroller.scrollTop;
    return true;
  }

  /** The rendered verse whose `number…last_number` holds a verse (a bridge), else null. */
  holding(segmentId) {
    const p = versePoint(segmentId);
    const block = p && this.blocks.find((b) => b.key === `${p[0]}.${p[1]}`);
    const v = block && block.data.verses.find((x) => x.number <= p[2] && p[2] <= x.last_number);
    return v ? this.verseEl(v.segment_id) : null;
  }

  /** A linked tab's move (specs/23 §7): load the chapter if needed, no animation, not
   *  reported. */
  follow(segmentId) {
    if (this.top && this.top.segment_id === segmentId) return;
    if (!this.verseEl(segmentId)) {
      const key = chapterOfSegment(segmentId);
      if (!key) return;
      this.open(key, { segmentId });
    } else {
      this.scrollToSegment(segmentId);
      this.fill();
      this.top = this.measureTop();
      this.updateHeader();
    }
    this.reported = { live: segmentId, settled: this.top };
  }

  /** Back to the verse last measured at the top, after a reflow (text size, column width). */
  restore() {
    if (!this.top) return;
    if (!this.scrollToSegment(this.top.segment_id, this.top.scalar)) return;
    this.fill();
  }

  /** Keeps the window around the block in view: loads neighbours as the reader nears either
   *  end, releases blocks far away, and shifts scrollTop by whatever changed above the anchor. */
  fill() {
    if (!this.blocks.length || !this.text.isConnected) return;
    const view = this.scroller.clientHeight;
    const anchorBlock = this.blockAtTop();
    const anchor = anchorBlock.section;
    const before = anchor.getBoundingClientRect().top;
    let i = this.blocks.indexOf(anchorBlock);
    const box = this.scroller.getBoundingClientRect();
    // Positions as they will be once scrollTop has taken up what changed above the anchor.
    const pending = () => anchor.getBoundingClientRect().top - before;
    const topOf = (el) => el.getBoundingClientRect().top - pending();
    const textBottom = () => this.text.getBoundingClientRect().bottom - pending();
    let changed = false;
    // Release far blocks first so the cap leaves room for the side being read into.
    while (i > WINDOW.eachSide && this.blocks.length > 1) {
      if (box.top - topOf(this.blocks[1].section) < WINDOW.reach * view) break; // still needed as reach above
      this.blocks.shift().section.remove(); i -= 1; this.stats.releases += 1; changed = true;
    }
    while (this.blocks.length - 1 - i > WINDOW.eachSide) {
      const b = this.blocks[this.blocks.length - 1];
      if (topOf(b.section) - box.bottom < WINDOW.reach * view) break;
      b.section.remove(); this.blocks.pop(); this.stats.releases += 1; changed = true;
    }
    // Then load: two each side, and more while the text does not reach far enough.
    for (;;) {
      const first = this.blocks[0];
      if (!first.prev || this.blocks.length >= WINDOW.cap) break;
      if (i >= WINDOW.eachSide && box.top - topOf(first.section) >= WINDOW.reach * view) break;
      const b = this.load(first.prev);
      this.text.insertBefore(b.section, first.section);
      this.blocks.unshift(b); i += 1; this.stats.prepends += 1; changed = true;
    }
    for (;;) {
      const last = this.blocks[this.blocks.length - 1];
      if (!last.next || this.blocks.length >= WINDOW.cap) break;
      if (this.blocks.length - 1 - i >= WINDOW.eachSide && textBottom() - box.bottom >= WINDOW.reach * view) break;
      const b = this.load(last.next);
      this.text.appendChild(b.section);
      this.blocks.push(b); this.stats.appends += 1; changed = true;
    }
    if (!changed) return;
    const shift = anchor.getBoundingClientRect().top - before;
    if (shift) {
      const programmatic = this.expected != null && Math.abs(this.scroller.scrollTop - this.expected) < 1;
      this.scroller.scrollTop += shift;
      if (programmatic) this.expected = this.scroller.scrollTop;
    }
    if (this.onBlocks) this.onBlocks();
  }

  // MARK: - Position

  /** The segment and scalar on the reading line, as `tab.record_scroll` takes them: the
   *  first verse with a line below it, and the first character of that verse below it. Pure
   *  geometry, so a menu or the palette over the text does not change the answer. */
  measureTop() {
    if (!this.blocks.length || !this.text.isConnected) return null;
    const y = this.edge() + 2;
    let block = this.blockAtTop();
    if (!block) return null;
    let verses = block.verses.querySelectorAll("[data-segment]");
    const bottom = (v) => { const r = v.getClientRects(); return r.length ? r[r.length - 1].bottom : -Infinity; };
    // Binary search: the first verse whose last line ends below the reading line.
    let lo = 0, hi = verses.length;
    while (lo < hi) { const mid = (lo + hi) >> 1; if (bottom(verses[mid]) > y) hi = mid; else lo = mid + 1; }
    if (lo === verses.length) {
      block = this.blocks[this.blocks.indexOf(block) + 1];
      if (!block) return null;
      verses = block.verses.querySelectorAll("[data-segment]");
      lo = 0;
      if (!verses.length) return null;
    }
    const verse = verses[lo];
    const node = verse.querySelector(".t").firstChild;
    const rects = verse.getClientRects();
    if (!node || !rects.length || rects[0].bottom > y) return { segment_id: verse.dataset.segment, scalar: 0 };
    // The verse began above the line: its first character whose line ends below it.
    const r = document.createRange();
    let a = 0, b = node.length;
    while (a < b) {
      const mid = (a + b) >> 1;
      r.setStart(node, mid); r.setEnd(node, Math.min(mid + 1, node.length));
      const rr = r.getClientRects();
      if (rr.length && rr[rr.length - 1].bottom > y) b = mid; else a = mid + 1;
    }
    return { segment_id: verse.dataset.segment, scalar: [...node.data.slice(0, a)].length };
  }

  updateHeader() {
    const b = this.blockAtTop();
    const label = b ? `${this.resource.short} · ${b.data.heading.short_label}` : this.resource.short;
    if (label !== this.headerLabel) {
      this.headerLabel = label;
      this.header.textContent = label;
    }
  }

  scrolled() {
    if (this.scheduled) return;
    this.scheduled = true;
    requestAnimationFrame(() => { this.scheduled = false; this.tick(); });
  }

  /** One frame of scrolling: the window, the header, and the verse at the top reported when it
   *  changed (unless the scroll was ours). */
  tick() {
    if (!this.text.isConnected) return;
    const t0 = performance.now();
    const ours = this.expected != null && Math.abs(this.scroller.scrollTop - this.expected) < 1;
    if (!ours) this.expected = null;
    this.fill();
    const top = this.measureTop();
    if (top) this.top = top;
    this.updateHeader();
    if (this.onFrame) this.onFrame(performance.now() - t0);
    if (ours || !top) return;
    if (!this.reported || this.reported.live !== top.segment_id) {
      this.reported = { live: top.segment_id, settled: top };
      this.onTop(top, false);
    }
    clearTimeout(this.settleTimer);
    this.settleTimer = setTimeout(() => this.settle(), SETTLE_MS);
  }

  settle() {
    if (!this.text.isConnected || this.expected != null) return;
    const top = this.measureTop();
    if (!top) return;
    this.top = top;
    const last = this.reported && this.reported.settled;
    if (last && last.segment_id === top.segment_id && last.scalar === top.scalar) return;
    this.reported = { live: top.segment_id, settled: top };
    this.onTop(top, true);
  }

  // MARK: - Mark and flash

  /** The quiet mark on the typed verses (specs/23 §7), on every rendered block. */
  setMark(range) {
    this.mark = range;
    for (const v of this.text.querySelectorAll(".verse")) v.classList.toggle("target", inRange(range, v.dataset.segment));
  }

  /** Flashes the marked verses once. */
  flash() {
    for (const v of this.text.querySelectorAll(".verse.target")) {
      v.classList.remove("flash");
      void v.offsetWidth;
      v.classList.add("flash");
      v.addEventListener("animationend", () => v.classList.remove("flash"), { once: true });
    }
  }

  /** Whether any verse a range touches is a placeholder (specs/23 §5). */
  touchesPlaceholder(range) {
    for (const v of this.text.querySelectorAll(".verse.placeholder-verse")) if (range.intersectsNode(v)) return true;
    return false;
  }
}
