// The shell's resource metadata and the fixture texts (B072): a resource id and a passage in,
// display metadata and core segments out, over the conformance fixture texts the core ships
// (BSB John 3:16–17, WLC Genesis 1:1–2, SBLGNT John 3:16, the study note). Since B078 reading
// tabs take their text from the core's ReadingHandle (placeholder editions with these fixture
// verses overlaid; B071's packs later); this source still names the resources, holds the study
// note and gives the fixture verses' licence line.
//
//   resource(contentId)          → { id, short, name, dir, lang, kind } (always; an unknown id
//                                  gets its id as its name)
//   titles()                     → { contentId: short name } for the core's palette rows
//   languages()                  → { contentId: BCP 47 tag } for the core's tool well (B076:
//                                  `grc`, `he` and `hbo` resources get the original-language tools)
//   passage(contentId, reference) → { paragraph, attribution } or null when the resource has no
//                                  text for that passage (the shell shows specs/21's
//                                  placeholder, not an error)
//   useEditions(editions)        → B071: installed packs' `ReadingEdition`s name their resources
//                                  from now on, and their fixture verses are no longer offered
//   edition(contentId)           → that `ReadingEdition`, or null on a placeholder
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
  const editions = new Map();
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
  const named = (id) => {
    const r = RESOURCES[id];
    const e = editions.get(id);
    return e ? { ...r, short: e.title, name: e.name, dir: e.direction, lang: e.language } : r;
  };
  return {
    useEditions(list) {
      for (const e of list) editions.set(e.resource_id, e);
    },
    edition(contentId) {
      return editions.get(contentId) || null;
    },
    titles() {
      return Object.fromEntries(Object.keys(RESOURCES).map((id) => [id, named(id).short]));
    },
    languages() {
      return Object.fromEntries(Object.keys(RESOURCES).filter((id) => !RESOURCES[id].kind).map((id) => [id, named(id).lang]));
    },
    resource(contentId) {
      const r = RESOURCES[contentId] && named(contentId);
      return r ? { id: contentId, kind: r.kind || "reading", short: r.short, name: r.name, dir: r.dir, lang: r.lang }
        : { id: contentId, kind: "reading", short: contentId, name: contentId, dir: "ltr", lang: "und" };
    },
    /** B071: a fixture verse at the revision a highlight may have been stored on, whatever is
     *  installed now; null if that is not the revision this host carries. */
    fixtureRevision(contentId, revision, segmentId) {
      const text = paragraphs.get(contentId);
      if (!text || text.paragraph.content_revision !== revision) return null;
      const segment = text.paragraph.segments.find((s) => s.segment_id === segmentId);
      return segment ? { content_revision: revision, text: segment.text, tokens: segment.tokens } : null;
    },
    passage(contentId, reference) {
      const text = paragraphs.get(contentId);
      if (!text || editions.has(contentId)) return null;
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
