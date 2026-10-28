// The Node side of B061 item 3: the same checks the browser worker test runs
// (crates/ffi/tests/worker.rs), against the one `--target web` build in ../pkg.
// Node has no fetch for file: URLs, so the module bytes are read and handed to init().
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { before, test } from "node:test";

import init, * as core from "../pkg/openlogos_ffi.js";

const pkg = new URL("../pkg/", import.meta.url);
const repo = new URL("../../../../", import.meta.url);

before(async () => {
  const bytes = await readFile(new URL("openlogos_ffi_bg.wasm", pkg));
  await init({ module_or_path: bytes });
});

test("apiVersion() is the core's API_VERSION", async () => {
  const source = await readFile(new URL("crates/core/src/lib.rs", repo), "utf8");
  const match = source.match(/pub const API_VERSION: &str = "([^"]+)";/);
  assert.ok(match, "API_VERSION constant found in crates/core/src/lib.rs");
  assert.equal(core.apiVersion(), match[1]);
  assert.equal(core.runConformance().api_version, match[1]);
});

test("parseReference() parses a compound citation", () => {
  const set = core.parseReference("Jn 3:16, 18; Rom 8:28-30");
  assert.equal(set.ranges.length, 3);
  assert.deepEqual(set.ranges[0].start, { book: "John", chapter: 3, verse: 16, subverse: null });
  assert.deepEqual(set.ranges[2].end, { book: "Rom", chapter: 8, verse: 30, subverse: null });
  assert.deepEqual(set.flags, []);
  assert.equal(core.formatOsis(set), "John.3.16;John.3.18;Rom.8.28-Rom.8.30");
  assert.equal(core.formatHuman(set), "John 3:16, 18; Rom 8:28–30");
});

test("a core error is a JS Error with kind and details", () => {
  assert.throws(
    () => core.parseReference("Hezekiah 3:16"),
    (error) =>
      error instanceof Error &&
      error.kind === "unknown_book" &&
      error.message === "unknown book `Hezekiah`" &&
      error.details.name === "Hezekiah",
  );
  assert.throws(() => core.formatOsis({ not: "a reference set" }), { kind: "invalid_argument" });
});

test("mapVerse() answers the Psalm 9/10 fixture cases", () => {
  const fixtures = JSON.parse(core.conformanceFixturesJson());
  const psalms = fixtures.find((f) => f.id === "psalm-9-10-numbering");
  assert.ok(psalms.cases.length > 10);
  for (const c of psalms.cases) {
    const result = core.mapVerse(c.reference, c.from, c.to);
    const targets = result.targets.map((t) => core.formatOsis({ ranges: [{ start: t, end: t }], flags: [] }));
    assert.deepEqual({ status: result.status, targets, path: result.path }, { status: c.status, targets: c.targets, path: c.path }, c.name);
  }
  assert.throws(() => core.mapVerse("Ps 9:1-2", "eng", "org"), { kind: "not_one_reference" });
});

test("a UTF-16 selection round-trips through an anchor on the Hebrew fixture", () => {
  const fixtures = JSON.parse(core.conformanceFixturesJson());
  const hebrew = fixtures.find((f) => f.id === "hebrew-cantillation-genesis-1");
  assert.ok(hebrew.selections.length >= 5);
  for (const selection of hebrew.selections) {
    const segment = hebrew.segments.find((s) => s.segment_id === selection.segment_id);
    const paragraph = {
      edition_id: hebrew.edition_id,
      content_revision: hebrew.content_revision,
      joiner: "",
      segments: [
        {
          segment_id: segment.segment_id,
          text: segment.text,
          tokens: segment.tokens.map((t) => ({ id: core.parseTokenId(t.id), range: { start: t.start, end: t.end } })),
        },
      ],
    };
    // What a DOM Selection reports: JS string indexes.
    const { start, end } = selection.utf16;
    const anchors = core.selectionToAnchor(paragraph, start, end);
    assert.equal(anchors.length, 1, selection.name);
    const [anchor] = anchors;
    assert.equal(anchor.segment_id, selection.segment_id);
    assert.deepEqual(anchor.range, selection.scalar, selection.name);
    assert.equal(anchor.quote.exact, segment.text.slice(start, end), selection.name);
    assert.deepEqual(anchor.tokens.map(core.formatTokenId), selection.tokens, selection.name);
    // And back: where the highlight is painted.
    assert.deepEqual(core.anchorToUtf16(anchor, segment.text), { start, end }, selection.name);
    // Reanchoring onto the same revision changes nothing.
    const revision = { content_revision: hebrew.content_revision, text: segment.text, tokens: paragraph.segments[0].tokens };
    assert.deepEqual(core.reanchor(anchor, revision, revision), { status: "exact", value: anchor }, selection.name);
  }
  // An index inside a surrogate pair is refused, not rounded.
  const bidi = fixtures.find((f) => f.id === "bidi-isolation-note");
  const note = bidi.segments[0];
  const thumb = note.text.indexOf("\u{1F44D}");
  assert.ok(thumb >= 0);
  const paragraph = { edition_id: bidi.edition_id, content_revision: bidi.content_revision, joiner: "", segments: [{ segment_id: note.segment_id, text: note.text, tokens: [] }] };
  assert.throws(() => core.selectionToAnchor(paragraph, 0, thumb + 1), { kind: "splits_surrogate_pair" });
});

test("runConformance() passes every fixture case", () => {
  const report = core.runConformance();
  const failures = report.cases.filter((c) => !c.passed).map((c) => `${c.fixture}/${c.name}: expected ${c.expected} got ${c.actual}`);
  assert.deepEqual(failures, []);
  assert.equal(report.failed, 0);
  assert.equal(report.cases.length, report.passed);
  assert.ok(report.passed > 100);
  console.log(`conformance cases: ${report.passed} passed, ${report.failed} failed (api ${report.api_version})`);
});
