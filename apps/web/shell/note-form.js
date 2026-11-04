// B074 (specs/24 §2.1): a note's stored block form and the note editor's DOM, converted at the
// edge. The core holds the note; the editor shows it and gives it back.
//
// To the DOM: a `paragraph` is `<p>`, a `heading` `<h2>`, a run of `list_item`s one `<ul>` of
// `<li>`, a `quote` `<blockquote>`; an empty block holds a `<br>` so it can take the caret. A
// span is text in `<b>` and `<i>` as marked, inside a passage link (`a.passage-link` with its
// OSIS reference in `data-reference`) when it has one.
//
// From the DOM: the editor's blocks in order, whatever `contenteditable` and `execCommand`
// made of them (`<div>` is a paragraph, any `<h1>`–`<h6>` a heading, `<ol>` items list items,
// a block inside a quote or a list item takes that kind, a `<br>` inside a block starts the
// next block of the same kind); `<b>`, `<strong>` and a bold weight are bold, `<i>` and `<em>`
// italic; adjacent text with the same marks is one span. Anything else (colours, fonts) is
// dropped.

const BLOCK_TAGS = new Set(["P", "DIV", "H1", "H2", "H3", "H4", "H5", "H6", "UL", "OL", "LI", "BLOCKQUOTE"]);
const HEADINGS = new Set(["H1", "H2", "H3", "H4", "H5", "H6"]);

/** Fills `editor` with a stored note's blocks; `link(reference, text)` makes a passage link. */
export function renderNote(editor, note, link) {
  const doc = editor.ownerDocument;
  const out = [];
  let list = null;
  for (const block of note.blocks) {
    let el;
    if (block.kind === "list_item") {
      if (!list) { list = doc.createElement("ul"); out.push(list); }
      el = doc.createElement("li");
      list.appendChild(el);
    } else {
      list = null;
      el = doc.createElement({ heading: "h2", quote: "blockquote" }[block.kind] || "p");
      out.push(el);
    }
    for (const span of block.spans) {
      let node = span.link ? link(span.link, span.text) : doc.createTextNode(span.text);
      if (span.italic) { const i = doc.createElement("i"); i.appendChild(node); node = i; }
      if (span.bold) { const b = doc.createElement("b"); b.appendChild(node); node = b; }
      el.appendChild(node);
    }
    if (!block.spans.length) el.appendChild(doc.createElement("br"));
  }
  if (!out.length) { const p = doc.createElement("p"); p.appendChild(doc.createElement("br")); out.push(p); }
  editor.replaceChildren(...out);
}

function isBold(el) {
  if (el.tagName === "B" || el.tagName === "STRONG") return true;
  const w = el.style && el.style.fontWeight;
  return w === "bold" || Number(w) >= 600;
}

function isItalic(el) {
  return el.tagName === "I" || el.tagName === "EM" || (el.style && el.style.fontStyle === "italic");
}

/** The stored blocks of the editor's DOM, for `putNote`. */
export function noteFromEditor(editor, id) {
  const blocks = [];
  let current = null;
  const open = (kind) => { current = { kind, spans: [] }; blocks.push(current); };
  const push = (text, marks) => {
    if (!text) return;
    const last = current.spans[current.spans.length - 1];
    if (last && !marks.link && !last.link && !!last.bold === marks.bold && !!last.italic === marks.italic) { last.text += text; return; }
    const span = { text };
    if (marks.bold) span.bold = true;
    if (marks.italic) span.italic = true;
    if (marks.link) span.link = marks.link;
    current.spans.push(span);
  };
  // Inline content of a block: text with the marks of its ancestors; a nested block (a `<p>`
  // inside a `<li>`) opens a block of the kind it is in.
  const inline = (node, kind, marks) => {
    for (const child of node.childNodes) {
      if (child.nodeType === 3) {
        if (!current) open(kind);
        push(child.data.replace(/​/g, ""), marks);
      } else if (child.nodeType === 1) {
        if (child.tagName === "BR") {
          // A trailing `<br>` only keeps an empty line open; one between text breaks the line.
          if (child.nextSibling && current && current.spans.length) open(kind);
          else if (!current) open(kind);
          continue;
        }
        if (child.matches && child.matches("a.passage-link[data-reference]")) {
          if (!current) open(kind);
          push(child.textContent, { ...marks, link: child.dataset.reference });
          continue;
        }
        if (BLOCK_TAGS.has(child.tagName)) { block(child, kind); continue; }
        inline(child, kind, { bold: marks.bold || isBold(child), italic: marks.italic || isItalic(child), link: null });
      }
    }
  };
  const block = (el, inherited) => {
    let kind = inherited;
    if (el.tagName === "LI") kind = "list_item";
    else if (el.tagName === "BLOCKQUOTE") kind = "quote";
    else if (HEADINGS.has(el.tagName)) kind = inherited === "paragraph" ? "heading" : inherited;
    if (el.tagName === "UL" || el.tagName === "OL") {
      for (const child of el.childNodes) {
        if (child.nodeType === 1 && child.tagName === "LI") block(child, "list_item");
        else if (child.nodeType === 1 || (child.nodeType === 3 && child.data.trim())) { current = null; inline({ childNodes: [child] }, "list_item", { bold: false, italic: false, link: null }); }
      }
      current = null;
      return;
    }
    current = null;
    inline(el, kind, { bold: isBold(el), italic: isItalic(el), link: null });
    if (!current) open(kind);
    current = null;
  };
  // Text and inline elements straight in the editor run together as one paragraph.
  for (const child of editor.childNodes) {
    if (child.nodeType === 1 && BLOCK_TAGS.has(child.tagName)) { block(child, "paragraph"); continue; }
    if (child.nodeType === 3 && !child.data.trim() && !current) continue;
    inline({ childNodes: [child] }, "paragraph", { bold: false, italic: false, link: null });
  }
  return { id, blocks };
}
