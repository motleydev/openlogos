#!/usr/bin/env python3
"""B016: checks a built Institutes work pack against the two digitized volumes on the NAS with
a reading written apart from the importer's: line scanning and regular expressions from the
standard library, its own book-name table, and the print edition's own Scripture index as an
outside witness for the citations. `just institutes-pack-verify`.

Read-only on the NAS and the pack. Needs both, so it is not part of `just ci`; it says SKIPPED
and exits 2 without them. With --show it prints places for reading by eye; nothing it prints
goes into the repository.
"""

import argparse
import collections
import hashlib
import json
import os
import re
import sqlite3
import sys
from pathlib import Path

FILES = ["pg45001.txt", "pg64392.txt"]
START = "*** START OF THE PROJECT GUTENBERG EBOOK"
END = "*** END OF THE PROJECT GUTENBERG EBOOK"
HYPHEN = "‐"
ROMAN = {"i": 1, "v": 5, "x": 10, "l": 50, "c": 100}

# This reader's own table: the names the notes use, to OSIS. Written from the notes, not from
# the importer's alias table.
BOOKS = {
    "Gen": "Gen", "Genesis": "Gen", "Exod": "Exod", "Ex": "Exod", "Exodus": "Exod", "Lev": "Lev",
    "Le": "Lev", "Leviticus": "Lev", "Num": "Num", "Numb": "Num", "Numbers": "Num", "Deut": "Deut",
    "De": "Deut", "Deuteronomy": "Deut", "Josh": "Josh", "Joshua": "Josh", "Judg": "Judg",
    "Judges": "Judg", "Ruth": "Ruth", "1 Sam": "1Sam", "2 Sam": "2Sam", "1 Samuel": "1Sam",
    "2 Samuel": "2Sam", "1 Kings": "1Kgs", "2 Kings": "2Kgs", "1 Kgs": "1Kgs", "2 Kgs": "2Kgs",
    "1 Chron": "1Chr", "2 Chron": "2Chr", "1 Chr": "1Chr", "2 Chr": "2Chr", "Ezra": "Ezra",
    "Neh": "Neh", "Nehemiah": "Neh", "Esth": "Esth", "Esther": "Esth", "Job": "Job",
    "Psalm": "Ps", "Psalms": "Ps", "Ps": "Ps", "Psal": "Ps", "Prov": "Prov", "Proverbs": "Prov",
    "Eccles": "Eccl", "Eccl": "Eccl", "Ecc": "Eccl", "Ecclesiastes": "Eccl", "Cant": "Song",
    "Song": "Song", "Isaiah": "Isa", "Isa": "Isa", "Is": "Isa", "Jer": "Jer", "Jerem": "Jer",
    "Jeremiah": "Jer", "Lam": "Lam", "Lament": "Lam", "Lamentations": "Lam", "Ezek": "Ezek",
    "Ezekiel": "Ezek", "Dan": "Dan", "Daniel": "Dan", "Hos": "Hos", "Hosea": "Hos", "Joel": "Joel",
    "Amos": "Amos", "Obad": "Obad", "Jonah": "Jonah", "Micah": "Mic", "Mic": "Mic", "Nah": "Nah",
    "Nahum": "Nah", "Hab": "Hab", "Habakkuk": "Hab", "Zeph": "Zeph", "Zephaniah": "Zeph",
    "Hag": "Hag", "Haggai": "Hag", "Zech": "Zech", "Zechariah": "Zech", "Mal": "Mal",
    "Malachi": "Mal", "Matt": "Matt", "Matthew": "Matt", "Mark": "Mark", "Luke": "Luke",
    "John": "John", "Acts": "Acts", "Rom": "Rom", "Romans": "Rom", "1 Cor": "1Cor", "2 Cor": "2Cor",
    "Gal": "Gal", "Galatians": "Gal", "Eph": "Eph", "Ephes": "Eph", "Ephesians": "Eph",
    "Phil": "Phil", "Philip": "Phil", "Philippians": "Phil", "Col": "Col", "Coloss": "Col",
    "Colossians": "Col", "1 Thess": "1Thess", "2 Thess": "2Thess", "1 Thes": "1Thess",
    "2 Thes": "2Thess", "1 Tim": "1Tim", "2 Tim": "2Tim", "Titus": "Titus", "Tit": "Titus",
    "Philem": "Phlm", "Philemon": "Phlm", "Heb": "Heb", "Hebr": "Heb", "Hebrews": "Heb",
    "James": "Jas", "Jas": "Jas", "1 Peter": "1Pet", "2 Peter": "2Pet", "1 Pet": "1Pet",
    "2 Pet": "2Pet", "1 Pe": "1Pet", "1 John": "1John", "2 John": "2John", "3 John": "3John",
    "Jude": "Jude", "Rev": "Rev", "Revelation": "Rev", "Apoc": "Rev",
}

# The Scripture index's book headings.
INDEX_BOOKS = {
    "GENESIS": "Gen", "EXODUS": "Exod", "LEVITICUS": "Lev", "NUMBERS": "Num", "DEUTERONOMY": "Deut",
    "JOSHUA": "Josh", "JUDGES": "Judg", "RUTH": "Ruth", "1 SAMUEL": "1Sam", "2 SAMUEL": "2Sam",
    "1 KINGS": "1Kgs", "2 KINGS": "2Kgs", "1 CHRONICLES": "1Chr", "2 CHRONICLES": "2Chr",
    "EZRA": "Ezra", "NEHEMIAH": "Neh", "ESTHER": "Esth", "JOB": "Job", "PSALMS": "Ps",
    "PROVERBS": "Prov", "ECCL": "Eccl", "ECCLESIASTES": "Eccl", "SONG": "Song", "ISAIAH": "Isa",
    "JEREMIAH": "Jer", "LAMENTA": "Lam", "EZEKIEL": "Ezek", "DANIEL": "Dan", "HOSEA": "Hos",
    "JOEL": "Joel", "AMOS": "Amos", "OBADIAH": "Obad", "JONAH": "Jonah", "MICAH": "Mic",
    "NAHUM": "Nah", "HABAKKUK": "Hab", "ZEPHANIAH": "Zeph", "HAGGAI": "Hag", "ZECHARIAH": "Zech",
    "MALACHI": "Mal", "MATTHEW": "Matt", "MARK": "Mark", "LUKE": "Luke", "JOHN": "John",
    "ACTS": "Acts", "ROMANS": "Rom", "1 CORINTHIANS": "1Cor", "2 CORINTHIANS": "2Cor",
    "GALATIANS": "Gal", "EPHESIANS": "Eph", "PHILIPPIANS": "Phil", "COLOSSIANS": "Col",
    "1 THESSALONIANS": "1Thess", "2 THESSALONIANS": "2Thess", "1 TIMOTHY": "1Tim",
    "2 TIMOTHY": "2Tim", "TITUS": "Titus", "PHILEMON": "Phlm", "HEBREWS": "Heb", "JAMES": "Jas",
    "1 PETER": "1Pet", "2 PETER": "2Pet", "1 JOHN": "1John", "2 JOHN": "2John", "3 JOHN": "3John",
    "JUDE": "Jude", "REVELATIONS": "Rev", "REVELATION": "Rev", "1. SAMUEL": "1Sam",
    "1 CHRON": "1Chr", "2 CHRON": "2Chr", "1 CORINTH": "1Cor", "2 CORINTH": "2Cor",
    "1 THESSALON": "1Thess", "2 THESSALON": "2Thess",
}

SPREAD = ["dedication", "1.1.1", "1.13.1", "1.13.7", "1.13.29", "3.2.7", "3.21.1", "3.21.7",
          "4.17.1", "4.17.50", "4.20.32"]


def roman(text):
    text = text.lower()
    if not text or any(c not in ROMAN for c in text):
        return None
    total = 0
    for i, c in enumerate(text):
        v = ROMAN[c]
        total += -v if i + 1 < len(text) and ROMAN[text[i + 1]] > v else v
    return total


class Failures:
    def __init__(self):
        self.items = []

    def add(self, text):
        self.items.append(text)

    def equal(self, label, ours, theirs):
        if ours != theirs:
            self.add(f"{label}\n  pack   {ours!r}\n  source {theirs!r}")


def body_lines(raw):
    text = raw.decode("utf-8")
    lines = [l[:-1] if l.endswith("\r") else l for l in text.split("\n")]
    start = next(i for i, l in enumerate(lines) if l.startswith(START))
    end = next(i for i, l in enumerate(lines) if l.startswith(END))
    return lines[start + 1:end]


def blocks(lines):
    """[(first line index, [lines])] separated by blank lines."""
    out, cur, first = [], [], 0
    for i, line in enumerate(lines):
        if line.strip():
            if not cur:
                first = i
            cur.append(line)
        elif cur:
            out.append((first, cur))
            cur = []
    if cur:
        out.append((first, cur))
    return out


def rebuild(lines):
    """A paragraph's text as the decision in the evidence states it, markers still in."""
    if all(l.startswith(" ") for l in lines):
        return "\n".join(l.strip() for l in lines)
    text = ""
    for i, line in enumerate(lines):
        piece = line.strip()
        if i and not text.endswith(HYPHEN):
            text += " "
        text += piece
    return text


def take_markers(text, pattern, counter):
    """Removes the markers numbered next in sequence; returns (text, [(number, scalar)])."""
    out, anchors, pos = "", [], 0
    for m in re.finditer(pattern, text):
        n = int(m.group(1))
        if n != counter[0]:
            continue
        out += text[pos:m.start()]
        anchors.append((n, len(out)))
        pos = m.end()
        counter[0] += 1
    return out + text[pos:], anchors


def read_volume_one(lines, failures):
    """{unit key: [paragraph texts]} and notes, with units keyed as the pack keys chapters."""
    units = collections.OrderedDict()
    notes = {}
    paras = blocks(lines)
    counter = [1]
    key = None
    book = None
    front = {"ADVERTISEMENT.": "advertisement", "THE TRANSLATOR’S PREFACE.": "translator-preface",
             "DEDICATION.": "dedication", "GENERAL SYLLABUS.": "syllabus"}
    sections = collections.Counter()
    in_notes = False
    started = False
    seen_contents = False
    for first, para in paras:
        joined = " ".join(l.strip() for l in para)
        if in_notes:
            m = re.match(r"^\s*(\d+) (.*)$", para[0])
            notes[int(m.group(1))] = rebuild([m.group(2)] + [l.strip() for l in para[1:]]).replace("\n", " ")
            continue
        if joined == "FOOTNOTES":
            in_notes = True
            continue
        if joined == "CONTENTS":
            seen_contents = True
            continue
        if not started and joined != "ADVERTISEMENT.":
            if not seen_contents:
                units.setdefault("title.1", []).append(rebuild(para))
            continue
        if joined in front or joined.startswith("THE AUTHOR’S PREFACE TO AN EDITION"):
            key = front.get(joined, "author-preface")
            started = True
            units[key] = []
            continue
        m = re.match(r"^BOOK ([IVX]+)\. ", joined)
        if m:
            book = roman(m.group(1))
            key = None
            continue
        if joined == "Argument." and book:
            key = f"{book}.argument"
            units[key] = []
            continue
        m = re.match(r"^Chapter ([IVXL]+)\. ", para[0])
        if m and book:
            key = f"{book}.{roman(m.group(1))}"
            units[key] = []
            continue
        if key is None:
            continue
        text, anchors = take_markers(rebuild(para), r"\((\d+)\)", counter)
        units[key].append((text, anchors))
        if re.match(r"^[IVXLC]+\. ", para[0]) and "." in key and not key.endswith("argument") and key[0].isdigit():
            sections[key] += 1
    return units, notes, sections


def read_volume_two(lines, failures):
    units = collections.OrderedDict()
    notes = {}
    counter = [1]
    key = None
    book = 3
    sections = collections.Counter()
    current_note = None
    title = []
    seen_book3 = False
    for first, para in blocks(lines):
        joined = " ".join(l.strip() for l in para)
        m = re.match(r"^Footnote (\d+):$", joined)
        if m:
            current_note = int(m.group(1))
            notes[current_note] = ""
            continue
        if current_note is not None and all(l.startswith("  ") and not l.startswith("   ") for l in para):
            notes[current_note] = (notes[current_note] + " " if notes[current_note] else "") + rebuild(
                [l.strip() for l in para])
            continue
        current_note = None
        if joined.startswith("INDEX OF THE PRINCIPAL MATTERS"):
            break
        if not seen_book3:
            if joined == "BOOK III.":
                seen_book3 = True
                units["title.2"] = title
            else:
                title.append(rebuild(para))
            continue
        head = para[0].strip()
        m = re.match(r"^BOOK ([IVX]+)\.$", head)
        if m:
            book = roman(m.group(1))
            key = None
            continue
        if joined == "ARGUMENT.":
            key = f"{book}.argument"
            units[key] = []
            continue
        m = re.match(r"^CHAPTER ([IVXL]+)\.$", head)
        if m:
            key = f"{book}.{roman(m.group(1))}"
            units[key] = []
            continue
        text, anchors = take_markers(rebuild(para), r"\[(\d+)\]", counter)
        units[key].append((text, anchors))
        if re.match(r"^[IVXLC]+\. ", para[0]) and not key.endswith("argument"):
            sections[key] += 1
    units["title.2"] = [(t, []) for t in units["title.2"]]
    return units, notes, sections


def osis_starts(note):
    """This reader's citations of one note: ['Rom.8.28', 'Ps.51', ...] (range starts)."""
    out = []
    names = "|".join(sorted((re.escape(k) for k in BOOKS), key=len, reverse=True))
    item = re.compile(
        r"(?<![A-Za-z])(" + names + r")\.?,? (?:ver\. )?([ivxlc]+\.?(?= |$|,|;)|\d+)( ?\d+(?:[-‐–]\d+)?(?:, \d+(?:[-‐–]\d+)?)*)?")
    pos = 0
    while True:
        m = item.search(note, pos)
        if not m:
            break
        book = BOOKS[m.group(1)]
        chapter_text = m.group(2)
        if not chapter_text.isdigit():
            chapter = roman(chapter_text.rstrip("."))
            verses = m.group(3)
        else:
            chapter, verses = 1, chapter_text  # a single-chapter book's verse
        if chapter is None:
            pos = m.end()
            continue

        def push(ch, vs):
            if vs:
                for part in vs.strip().split(","):
                    first = re.split(r"[-‐–]", part.strip())[0]
                    if first.isdigit() and not (len(first) == 1 and first in "123" and False):
                        out.append(f"{book}.{ch}.{int(first)}")
            else:
                out.append(f"{book}.{ch}")

        # `, 1 Cor`: a trailing ordinal belongs to the next book.
        if verses and re.search(r", [123]$", verses) and re.match(r" [A-Z]", note[m.end():]):
            verses = verses[: verses.rfind(",")]
            end = m.start(3) + len(verses)
        else:
            end = m.end()
        push(chapter, verses)
        pos = end
        cont = re.compile(r"(?:, &c\.)?(?:[;.,] ?| )(?:and )?([ivxlc]+)\.( \d+(?:[-‐–]\d+)?(?:, \d+(?:[-‐–]\d+)?)*)?")
        while True:
            c = cont.match(note, pos)
            if not c or roman(c.group(1)) is None:
                break
            if not c.group(2):
                after = note[c.end():c.end() + 2]
                if not (after == "" or after[0] in ",;" or re.match(r" [ivxlc]", after) or after.startswith(" &")):
                    break
            push(roman(c.group(1)), c.group(2))
            pos = c.end()
    return out


def scripture_index(lines):
    """(book, chapter, first verse) of every entry of the print edition's Scripture index."""
    at = next(i for i, l in enumerate(lines) if "SCRIPTURE INDEX TO CALVIN" in l)
    out = collections.Counter()
    book = chapter = None
    for line in lines[at:]:
        s = line.strip()
        if not s:
            continue
        head = s.rstrip(".").upper()
        if head in INDEX_BOOKS and s.upper() == s:
            book, chapter = INDEX_BOOKS[head], None
            continue
        if line.startswith(" ● Transcriber") or "Transcriber" in s:
            break
        m = re.match(r"^\s+([ivxlc]+)?\s+([\d][\d\- ]*?)\s+(\d)\s+(\d+)$", line)
        if not m or book is None:
            continue
        if m.group(1):
            chapter = roman(m.group(1))
        if chapter is None:
            continue
        first = re.split(r"[ \-]", m.group(2).strip())[0]
        out[(book, chapter, int(first))] += 1
    return out


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--corpus", default="/Volumes/office-hub/openlogos/corpora/S75-gutenberg-calvin-institutes")
    parser.add_argument("--pack", default=os.path.expanduser(
        os.environ.get("OPENLOGOS_WORKS", "~/.cache/openlogos/works")) + "/institutes-allen.sqlite")
    parser.add_argument("--show", nargs="*")
    args = parser.parse_args()
    corpus, pack = Path(args.corpus), Path(args.pack)
    if not pack.exists():
        print(f"SKIPPED: no work pack at {pack} (just institutes-pack)")
        return 2
    if not (corpus / "MANIFEST.sha256").exists():
        print(f"SKIPPED: no corpus at {corpus}")
        return 2
    failures = Failures()
    listed = dict(reversed(l.split("  ", 1)) for l in (corpus / "MANIFEST.sha256").read_text().splitlines() if l)
    raw = {}
    for name in FILES:
        raw[name] = (corpus / name).read_bytes()
        failures.equal(f"{name} sha256", hashlib.sha256(raw[name]).hexdigest(), listed[name])
    lines1, lines2 = body_lines(raw[FILES[0]]), body_lines(raw[FILES[1]])
    units1, notes1, sections1 = read_volume_one(lines1, failures)
    units2, notes2, sections2 = read_volume_two(lines2, failures)

    db = sqlite3.connect(f"file:{pack}?mode=ro", uri=True)
    manifest = dict(db.execute("select key, value from manifest"))
    failures.equal("work_format", manifest.get("work_format"), "openlogos-work-1")
    structure = json.loads(manifest["work_structure"])
    rows = db.execute("select s.ordinal, s.locator, s.chapter, s.meta, t.body from work_section s "
                      "join text t on t.verse_id = s.ordinal order by s.ordinal").fetchall()
    by_chapter = collections.OrderedDict()
    pack_notes = {}
    for ordinal, locator, chapter, meta, body in rows:
        meta = json.loads(meta)
        paras = body.split("\n\n")
        failures.equal(f"{locator} styles", len(meta["styles"]), len(paras))
        for i, p in enumerate(paras):
            anchors = [(int(n["id"].split(":")[1]), n["scalar"]) for n in meta["notes"] if n["paragraph"] == i]
            by_chapter.setdefault(chapter, []).append((p, anchors))
        for n in meta["notes"]:
            pack_notes[n["id"]] = n["text"]
        if meta["number"] and meta["number"] > 1:
            failures.equal(f"{locator} starts with its label", paras[[s for s in meta["styles"]].index("prose")].split(" ", 1)[0] if "prose" in meta["styles"] else None, meta["label"])

    expected = collections.OrderedDict()
    expected.update(units1)
    expected.update(units2)
    paragraphs = 0
    for key, paras in expected.items():
        ours = by_chapter.get(key)
        if ours is None:
            failures.add(f"{key}: not in the pack")
            continue
        theirs = [p if isinstance(p, tuple) else (p, []) for p in paras]
        if len(ours) != len(theirs):
            failures.add(f"{key}: {len(ours)} paragraphs in the pack, {len(theirs)} in the source")
            continue
        for i, (a, b) in enumerate(zip(ours, theirs)):
            paragraphs += 1
            failures.equal(f"{key} paragraph {i}", a[0], b[0])
            failures.equal(f"{key} paragraph {i} anchors", a[1], b[1])
    extra = [k for k in by_chapter if k not in expected and k != "title.1"]
    for k in extra:
        failures.add(f"{k}: in the pack, not found in the source")

    numbered = collections.Counter()
    for division in structure:
        for chapter in division["chapters"]:
            if chapter["kind"] == "chapter":
                numbered[chapter["locator"]] = len(chapter["sections"])
    for key, n in list(sections1.items()) + list(sections2.items()):
        failures.equal(f"{key} sections (numbered paragraphs + the first)", numbered[key], n + 1)

    for v, notes in ((1, notes1), (2, notes2)):
        for n, text in notes.items():
            failures.equal(f"note {v}:{n}", pack_notes.get(f"{v}:{n}"), text)
    failures.equal("notes", len(pack_notes), len(notes1) + len(notes2))

    # Citations: this reader's per note against the pack's.
    pack_cites = collections.defaultdict(list)
    unresolved = collections.Counter()
    for note, target, status, cls in db.execute("select note, target, status, class from work_citation order by id"):
        if status == "resolved":
            for r in target.split(";"):
                pack_cites[note].append(r.split("-")[0])
        else:
            unresolved[cls] += 1
    differ = []
    for v, notes in ((1, notes1), (2, notes2)):
        for n, text in notes.items():
            nid = f"{v}:{n}"
            mine = osis_starts(text)
            if sorted(mine) != sorted(pack_cites.get(nid, [])):
                differ.append((nid, text, mine, pack_cites.get(nid, [])))
    agree = len(notes1) + len(notes2) - len(differ)

    # The print edition's Scripture index against the pack's links.
    index = scripture_index(lines2)
    linked = collections.Counter()
    for starts in pack_cites.values():
        for s in starts:
            parts = s.split(".")
            if len(parts) == 3:
                linked[(parts[0], int(parts[1]), int(parts[2]))] += 1
    index_keys, linked_keys = set(index), set(linked)
    both = index_keys & linked_keys

    if args.show is not None:
        places = args.show or SPREAD
        for place in places:
            for ordinal, locator, chapter, meta, body in rows:
                if locator == place:
                    print(f"=== {locator}")
                    print(body)
                    for n in json.loads(meta)["notes"]:
                        print(f"  note {n['id']} {n['marker']}: {n['text']}")

    sections_total = sum(1 for _ in rows)
    print(f"pack: {sections_total} sections, {len(pack_notes)} notes; {paragraphs} paragraphs compared with the source")
    print(f"source read apart: {len(units1) + len(units2)} units, {len(notes1)} + {len(notes2)} notes")
    print(f"citations: this reader and the pack agree on {agree} of {len(notes1) + len(notes2)} notes; "
          f"{len(differ)} differ (listed); pack unresolved by class {dict(unresolved)}")
    for nid, text, mine, theirs in differ[:40]:
        print(f"  {nid}: {text[:90]!r}\n     reader {mine}\n     pack   {theirs}")
    print(f"scripture index (Livermore, in volume II): {sum(index.values())} entries, {len(index_keys)} distinct verses; "
          f"pack links {len(linked_keys)} distinct verses; in both {len(both)} "
          f"({100 * len(both) / max(1, len(index_keys)):.1f} % of the index, {100 * len(both) / max(1, len(linked_keys)):.1f} % of the pack)")
    lowered = " ".join(b.lower() for (_, _, _, _, b) in rows) + " ".join(t.lower() for t in pack_notes.values())
    failures.equal("the digitizer named anywhere in the text", "gutenberg" in lowered, False)
    # The lead's ruling (B016): no byte of what ships names the digitizer, in any case.
    failures.equal("the digitizer named in any byte of the pack", b"gutenberg" in pack.read_bytes().lower(), False)
    if failures.items:
        print(f"FAILED: {len(failures.items)} differences")
        for f in failures.items[:60]:
            print(f)
        return 1
    print("OK: no differences")
    return 0


if __name__ == "__main__":
    sys.exit(main())
