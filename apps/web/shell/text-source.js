// The shell's one door to text (B072): a resource id and a passage in, display metadata and
// core segments out. Until B071's preview pack lands this is the conformance fixture texts the
// core ships (BSB John 3:16–17, WLC Genesis 1:1–2, SBLGNT John 3:16, the study note); B071
// replaces `fixtureTextSource` with a pack-backed source of the same shape and the shell does
// not change.
//
//   resource(contentId)          → { id, short, name, dir, lang, kind } (always; an unknown id
//                                  gets its id as its name)
//   titles()                     → { contentId: short name } for the core's palette rows
//   languages()                  → { contentId: BCP 47 tag } for the core's tool well (B076:
//                                  `grc`, `he` and `hbo` resources get the original-language tools)
//   passage(contentId, reference) → { paragraph, attribution } or null when the resource has no
//                                  text for that passage (the shell shows specs/21's
//                                  placeholder, not an error)
//
// `paragraph` is the shape the core's selectionToAnchor takes (edition_id, content_revision,
// joiner, segments with token spans), so the reader surface and the anchors work unchanged.
import { JOINER } from "../reader/surface.js";

const RESOURCES = {
  bsb: { short: "BSB", name: "Berean Standard Bible", dir: "ltr", lang: "en", fixture: "cross-verse-selection-john-3" },
  sblgnt: { short: "SBLGNT", name: "SBL Greek New Testament", dir: "ltr", lang: "grc", fixture: "greek-diacritics-john-3-sblgnt" },
  wlc: { short: "WLC", name: "Westminster Leningrad Codex", dir: "rtl", lang: "he", fixture: "hebrew-cantillation-genesis-1" },
  "fixture:note:1": { short: "Note", name: "Study note", dir: "ltr", lang: "en", fixture: "bidi-isolation-note", kind: "notes" },
  // B076: Word study's tab. It has no text until the lexicon import; the shell shows the word.
  lexicon: { short: "Lexicon", name: "Lexicon", dir: "ltr", lang: "en", kind: "lexicon" },
};

/** `Book.C[.V[-…]]` → [book, chapter] (OSIS, as the core stores tab references). */
function chapterOf(reference) {
  if (!reference) return null;
  const [book, chapter] = reference.split("-")[0].split(".");
  return chapter ? [book, Number(chapter)] : null;
}

export function fixtureTextSource(core) {
  const fixtures = JSON.parse(core.conformanceFixturesJson());
  const paragraphs = new Map();
  for (const [id, r] of Object.entries(RESOURCES)) {
    const f = fixtures.find((x) => x.kind === "text" && x.id === r.fixture);
    if (!f) continue;
    const license = (f.license || "").split(" ").filter((w) => !w.startsWith("http")).join(" ").replace(/[,;: ]+$/, "");
    paragraphs.set(id, {
      attribution: `Fixture text. ${license}`,
      paragraph: {
        edition_id: f.edition_id,
        content_revision: f.content_revision,
        joiner: JOINER,
        segments: f.segments.map((s) => ({
          segment_id: s.segment_id,
          text: s.text,
          tokens: (s.tokens || []).map((t) => ({ id: core.parseTokenId(t.id), range: { start: t.start, end: t.end } })),
        })),
      },
    });
  }
  return {
    titles() {
      return Object.fromEntries(Object.entries(RESOURCES).map(([id, r]) => [id, r.short]));
    },
    languages() {
      return Object.fromEntries(Object.entries(RESOURCES).filter(([, r]) => !r.kind).map(([id, r]) => [id, r.lang]));
    },
    resource(contentId) {
      const r = RESOURCES[contentId];
      return r ? { id: contentId, kind: r.kind || "reading", short: r.short, name: r.name, dir: r.dir, lang: r.lang }
        : { id: contentId, kind: "reading", short: contentId, name: contentId, dir: "ltr", lang: "und" };
    },
    passage(contentId, reference) {
      const text = paragraphs.get(contentId);
      if (!text) return null;
      if (RESOURCES[contentId].kind === "notes") return text;
      const wanted = chapterOf(reference);
      if (!wanted) return null;
      const segments = text.paragraph.segments.filter((s) => {
        const have = chapterOf(s.segment_id);
        return have && have[0] === wanted[0] && have[1] === wanted[1];
      });
      if (segments.length === 0) return null;
      return { attribution: text.attribution, paragraph: { ...text.paragraph, segments } };
    },
  };
}
