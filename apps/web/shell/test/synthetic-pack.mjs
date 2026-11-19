// B071: a small made-up preview pack for the shell's cases, written to a temporary directory at
// run time so no corpus byte is involved. It has what the real editions have that the fixture
// packs do not: verses an edition does not print, a chapter ending early, a Hebrew edition in
// `tanakh_bhs` order and right to left, a new content revision of the fixture verses, and a
// missing chunk and a chunk with the wrong hash. It has no search packs: search over real
// packs is `pack-real-*`, which skips visibly without one.
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";

const MADE_UP = "made-up words for the shell cases, none of them from any edition, long enough to wrap across a line of the column so that scrolling has something to move";
const HEBREW = "מילים לדוגמה בלבד שאינן מן הטקסט ואורכן מספיק כדי לגלוש לשורה נוספת בטור הקריאה";

function verse(book, chapter, number, text) {
  return { segment_id: `${book}.${chapter}.${number}`, number, last_number: number, label: String(number), text: `${number} ${text}`, paragraph_start: number % 5 === 1, poetry: false, superscription: false, placeholder: false, tokens: [] };
}

/** `chapters`: [book, chapter, first, last, absent] */
async function edition(root, meta, books, extra = {}) {
  const manifest = {
    format: "openlogos-text-pack", format_version: "1.1", ...meta, license: "made up for tests",
    source: "apps/web/shell/test/synthetic-pack.mjs", fixture_only: true, complete: true, books: [],
  };
  const dir = join(root, "packs", meta.resource_id);
  for (const [book, title, chapters] of books) {
    const listed = [];
    for (const [chapter, first, last, absent = []] of chapters) {
      const path = `chapters/${book}/${chapter}.json`;
      const key = `${book}.${chapter}`;
      const own = extra.verses && extra.verses[key];
      const verses = own || [];
      if (!own) for (let n = first; n <= last; n++) if (!absent.includes(n)) verses.push(verse(book, chapter, n, meta.direction === "rtl" ? HEBREW : MADE_UP));
      const bytes = Buffer.from(JSON.stringify({ format: "openlogos-text-pack-chapter", resource_id: meta.resource_id, chapter_key: key, verses }));
      const sha256 = extra.badHash === key ? "0".repeat(64) : createHash("sha256").update(bytes).digest("hex");
      const entry = { chapter, first_verse: first, last_verse: last, path, length: bytes.length, sha256 };
      if (absent.length) entry.absent = absent;
      listed.push(entry);
      if (extra.missing === key) continue;
      await mkdir(dirname(join(dir, path)), { recursive: true });
      await writeFile(join(dir, path), bytes);
    }
    manifest.books.push({ book, title, chapters: listed });
  }
  await mkdir(dir, { recursive: true });
  await writeFile(join(dir, "manifest.json"), JSON.stringify(manifest));
}

const span = (n, first = 1, last = 30) => Array.from({ length: n }, (_, i) => [i + 1, first, last]);

export async function writeSyntheticPack(root, repoRoot) {
  const fixture = JSON.parse(await readFile(join(repoRoot, "crates/conformance/fixtures/packs/bsb/chapters/John/3.json"), "utf8"));
  const [v16, v17] = fixture.verses;
  // A new revision of the fixture verses: 3:16 the same, 3:17 with words before it (the range
  // moves), 3:18 made up. The tests' highlights are stored on the fixture's `bsb-2024`.
  const shift = "Indeed ";
  const moved = { ...v17, text: shift + v17.text, tokens: [] };
  const john = [{ ...v16 }, moved, verse("John", 3, 18, MADE_UP)];
  const noon = (id) => ({ title: id.toUpperCase(), name: `Made-up ${id}`, content_revision: `${id}-synthetic-1` });
  await edition(root, {
    resource_id: "bsb", ...noon("bsb"), language: "en", direction: "ltr", scheme: "eng", book_order: "protestant",
    attribution: "Made-up credit for the BSB pack. Licence: https://example.org/licence/ and nothing else.",
  }, [
    ["Mal", "Malachi", [[3, 1, 18], [4, 1, 6]]],
    ["Matt", "Matthew", [[17, 1, 27, [21]]]],
    ["John", "John", [[3, 16, 18]]],
    ["Rom", "Romans", [...span(15), [16, 1, 27, [24]]]],
    ["Rev", "Revelation", [[1, 1, 20], [2, 1, 29]]],
  ], { verses: { "John.3": john }, missing: "Rev.1", badHash: "Rev.2" });
  await edition(root, {
    resource_id: "sblgnt", ...noon("sblgnt"), language: "grc", direction: "ltr", scheme: "org", book_order: "protestant",
    attribution: "Made-up credit for the SBLGNT pack.",
  }, [
    ["Matt", "ΚΑΤΑ ΜΑΘΘΑΙΟΝ", [[17, 1, 27, [21]]]],
    ["John", "ΚΑΤΑ ΙΩΑΝΝΗΝ", [[3, 16, 18]]],
    ["Rom", "ΠΡΟΣ ΡΩΜΑΙΟΥΣ", [...span(15), [16, 1, 24]]],
  ]);
  await edition(root, {
    resource_id: "wlc", ...noon("wlc"), language: "hbo", direction: "rtl", scheme: "org", book_order: "tanakh_bhs",
    attribution: "Made-up credit for the WLC pack.",
  }, [
    ["Gen", "בראשית", [[1, 1, 31]]],
    ["Mal", "מלאכי", [[3, 1, 24]]],
    ["Ps", "תהלים", [[1, 1, 6], [2, 1, 12]]],
  ]);
}
