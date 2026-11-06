#!/usr/bin/env python3
"""B071: checks a built preview pack against the NAS sources with parsers written apart from the
importer's (the standard library's XML parser and plain string handling). `just preview-pack-verify`.

Read-only on the NAS and the pack. Needs the corpus, so it is not part of `just ci`; it says
so and exits 2 when the corpus or the pack is not there. With --show it prints the spread of
awkward places for reading by eye; nothing it prints goes into the repository.
"""

import argparse
import hashlib
import json
import os
import re
import sys
import unicodedata
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

OSIS = "{http://www.bibletechnologies.net/2003/OSIS/namespace}"
SIGNS = "⸀⸁⸂⸃⸄⸅"
SPREAD = [
    ("bsb", ["Gen.1.1", "Ps.3.0", "Ps.3.1", "Ps.3.2", "Ps.11.1", "Ps.51.0", "Ps.51.1", "Ps.119.1",
             "Zech.12.1", "Hab.3.19", "Gen.35.18", "Mal.3.18", "Mal.4.1", "Mal.4.6", "Matt.17.20",
             "Matt.17.22", "Mark.16.8", "Mark.16.9", "Mark.16.20", "John.7.52", "John.7.53",
             "John.8.1", "John.8.11", "John.8.12", "Rom.16.23", "Rom.16.25", "3John.1.14",
             "Rev.12.17", "Rev.22.21"]),
    ("sblgnt", ["Matt.1.1", "Mark.16.8", "Mark.16.9", "Mark.16.20", "John.3.16", "John.7.53",
                "John.8.1", "John.8.11", "John.8.12", "Rom.16.23", "Rom.16.24", "1Cor.12.10",
                "2Cor.7.12", "2Cor.13.13", "3John.1.15", "Rev.12.18", "Rev.22.21"]),
    ("wlc", ["Gen.1.1", "Gen.1.2", "Gen.4.8", "Gen.35.22", "Num.10.35", "Ps.3.1", "Ps.3.2", "Ps.51.1",
             "Ps.51.2", "Ps.51.3", "Ruth.3.5", "Mal.3.18", "Mal.3.19", "Mal.3.24", "2Chr.36.23"]),
]


class Failures:
    def __init__(self):
        self.items = []

    def add(self, text):
        self.items.append(text)

    def equal(self, label, ours, theirs):
        if ours != theirs:
            self.add(f"{label}\n  pack   {ours!r}\n  source {theirs!r}")


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def load_pack(root, name, failures, schema):
    directory = root / "packs" / name
    raw = (directory / "manifest.json").read_bytes()
    manifest = json.loads(raw)
    if schema is not None:
        schema.validate(manifest)
    verses = {}
    order = []
    for book in manifest["books"]:
        order.append(book["book"])
        for chapter in book["chapters"]:
            data = (directory / chapter["path"]).read_bytes()
            if sha256(data) != chapter["sha256"] or len(data) != chapter["length"]:
                failures.add(f"{name} {chapter['path']}: hash or length")
            chunk = json.loads(data)
            numbers = []
            for verse in chunk["verses"]:
                verses[verse["segment_id"]] = verse
                numbers.append(verse["number"])
            if numbers != sorted(numbers) or len(set(numbers)) != len(numbers):
                failures.add(f"{name} {chunk['chapter_key']}: verse order")
            if numbers[0] != chapter["first_verse"] or numbers[-1] != chapter["last_verse"]:
                failures.add(f"{name} {chunk['chapter_key']}: first or last verse")
            gaps = sorted(set(range(numbers[0], numbers[-1] + 1)) - set(numbers))
            if gaps != chapter.get("absent", []):
                failures.add(f"{name} {chunk['chapter_key']}: absent is {chapter.get('absent')}, gaps {gaps}")
    return manifest, order, verses


def check_every_verse(name, verses, failures):
    for segment, verse in verses.items():
        text = verse["text"]
        if not text or text != text.strip() or "  " in text or "\n" in text or "\t" in text:
            failures.add(f"{name} {segment}: spacing {text!r}")
        if any(c in text for c in "\\|<>"):
            failures.add(f"{name} {segment}: markup residue {text!r}")
        if verse["placeholder"]:
            failures.add(f"{name} {segment}: placeholder")
        end = 0
        scalars = len(text)
        for word, token in enumerate(verse["tokens"], 1):
            if not (end <= token["start"] < token["end"] <= scalars):
                failures.add(f"{name} {segment}: token range {token}")
            end = token["end"]
            book, _, v = segment.rpartition(".")
            if not re.fullmatch(rf"{name}:\d{{2}}\d{{3}}{int(v):03d}{word:03d}", token["id"]):
                failures.add(f"{name} {segment}: token id {token['id']}")


def check_bsb(corpus, verses, failures):
    lines = {}
    names = []
    osis = list(order_of_books())
    text = (corpus / "bsb.txt").read_bytes().decode("utf-8-sig")
    for line in text.split("\r\n"):
        match = re.fullmatch(r"(.+) (\d+):(\d+)\t(.*)", line)
        if not match:
            continue
        if not names or names[-1] != match[1]:
            names.append(match[1])
        lines[f"{osis[len(names) - 1]}.{match[2]}.{match[3]}"] = match[4]
    empties = sorted(s for s, t in lines.items() if not t)
    seen = 0
    for segment, line in lines.items():
        if not line:
            if segment in verses:
                failures.add(f"bsb {segment}: empty in bsb.txt, in the pack")
            continue
        verse = verses.get(segment)
        if verse is None:
            failures.add(f"bsb {segment}: in bsb.txt, not in the pack")
            continue
        ours = verse["text"]
        title = verses.get(segment[: -len(".1")] + ".0") if segment.endswith(".1") else None
        if title is not None:
            ours = title["text"] + " " + ours
            seen += 1
        failures.equal(f"bsb {segment}", ours, line)
        seen += 1
    if seen != len(verses):
        failures.add(f"bsb: {len(verses)} segments in the pack, {seen} matched")
    # The USFM says where a Psalm title is; the zip is read here only for that count.
    with zipfile.ZipFile(corpus / "bsb_usfm.zip") as zipped:
        psalms = zipped.read("bsb_usfm/PSA.usfm").decode("utf-8")
    titled = len(re.findall(r"^\\d \\v 1 ", psalms, re.M))
    zero = sum(1 for s, v in verses.items() if v["superscription"])
    print(f"bsb: {len(lines)} lines of bsb.txt, {len(empties)} empty; {titled} titled Psalms in the USFM, {zero} as verse 0")
    return empties


def order_of_books():
    return [
        "Gen", "Exod", "Lev", "Num", "Deut", "Josh", "Judg", "Ruth", "1Sam", "2Sam", "1Kgs", "2Kgs", "1Chr",
        "2Chr", "Ezra", "Neh", "Esth", "Job", "Ps", "Prov", "Eccl", "Song", "Isa", "Jer", "Lam", "Ezek",
        "Dan", "Hos", "Joel", "Amos", "Obad", "Jonah", "Mic", "Nah", "Hab", "Zeph", "Hag", "Zech", "Mal",
        "Matt", "Mark", "Luke", "John", "Acts", "Rom", "1Cor", "2Cor", "Gal", "Eph", "Phil", "Col", "1Thess",
        "2Thess", "1Tim", "2Tim", "Titus", "Phlm", "Heb", "Jas", "1Pet", "2Pet", "1John", "2John", "3John",
        "Jude", "Rev",
    ]


def check_sblgnt(corpus, verses, failures):
    count = 0
    for path in sorted((corpus / "data/sblgnt/text").glob("*.txt")):
        book = path.stem
        for line in path.read_text(encoding="utf-8-sig").splitlines():
            if "\t" not in line:
                continue
            place, text = line.split("\t", 1)
            chapter, verse = place.rsplit(" ", 1)[1].split(":")
            cleaned = re.sub(rf"[{SIGNS}]\d?", "", text).rstrip(" ")
            ours = verses.get(f"{book}.{chapter}.{verse}")
            if ours is None:
                failures.add(f"sblgnt {place}: not in the pack")
                continue
            failures.equal(f"sblgnt {place}", ours["text"], cleaned)
            count += 1
    if count != len(verses):
        failures.add(f"sblgnt: {len(verses)} in the pack, {count} in the source")
    for segment, verse in verses.items():
        if any(c in verse["text"] for c in SIGNS) or re.search(r"\d", verse["text"]):
            failures.add(f"sblgnt {segment}: an apparatus sign left")
    print(f"sblgnt: {count} verses equal the source without its apparatus signs")


def read_oshb_verse(element):
    """The verse's reading stream: the elements outside notes, in order, spaced as the file has them."""
    pieces = []

    def word(w):
        return "".join(w.itertext()).replace("/", "")

    def space_after(node):
        return bool(node.tail) and node.tail.strip() == "" and len(node.tail) > 0

    for child in element:
        tag = child.tag.removeprefix(OSIS)
        if tag == "w":
            pieces.append(word(child))
            if space_after(child):
                pieces.append(" ")
        elif tag == "seg":
            kind = child.get("type")
            if kind in ("x-pe", "x-samekh"):
                pass
            else:
                if kind in ("x-maqqef", "x-sof-pasuq"):
                    while pieces and pieces[-1] == " ":
                        pieces.pop()
                pieces.append(child.text)
                if kind != "x-maqqef" and space_after(child):
                    pieces.append(" ")
        elif tag == "note":
            # A maqqef binds what follows, even past a note (2 Sam 16:23, Jer 50:29).
            bound = pieces and pieces[-1] == "־"
            if space_after(child) and not bound and pieces[-1:] != [" "]:
                pieces.append(" ")
        else:
            raise ValueError(f"element {tag}")
    return re.sub(r" +", " ", "".join(pieces)).strip()


def check_wlc(corpus, verses, failures):
    count = 0
    words = 0
    for path in sorted((corpus / "wlc").glob("*.xml")):
        if path.name == "VerseMap.xml":
            continue
        tree = ET.parse(path)
        for element in tree.iter(f"{OSIS}verse"):
            segment = element.get("osisID")
            if segment is None:
                continue
            count += 1
            ours = verses.get(segment)
            if ours is None:
                failures.add(f"wlc {segment}: not in the pack")
                continue
            failures.equal(f"wlc {segment}", ours["text"], read_oshb_verse(element))
            direct = [c for c in element if c.tag == f"{OSIS}w"]
            words += len(direct)
            if len(direct) != len(ours["tokens"]):
                failures.add(f"wlc {segment}: {len(direct)} words, {len(ours['tokens'])} tokens")
            text = ours["text"]
            for w, token in zip(direct, ours["tokens"]):
                failures.equal(
                    f"wlc {segment} token {token['id']}",
                    text[token["start"]: token["end"]],
                    "".join(w.itertext()).replace("/", ""),
                )
            if "/" in text or "־ " in text or " ׃" in text:
                failures.add(f"wlc {segment}: separator or spacing residue {text!r}")
    if count != len(verses):
        failures.add(f"wlc: {len(verses)} in the pack, {count} in the source")
    print(f"wlc: {count} verses and {words} words equal the source read by a second parser")


def check_fixtures(repo, packs, failures):
    """The conformance fixtures hold real verses; the packs must print them byte for byte."""
    for edition, name in [
        ("bsb", "cross-verse-selection-john-3"),
        ("sblgnt", "greek-diacritics-john-3-sblgnt"),
    ]:
        fixture = json.loads((repo / f"crates/conformance/fixtures/{name}.json").read_text(encoding="utf-8"))
        for segment in fixture["segments"]:
            ours = packs[edition].get(segment["segment_id"])
            failures.equal(f"fixture {edition} {segment['segment_id']}", ours and ours["text"], segment["text"])
            failures.equal(
                f"fixture tokens {edition} {segment['segment_id']}",
                ours and [(t["id"], t["start"], t["end"]) for t in ours["tokens"]],
                [(t["id"], t["start"], t["end"]) for t in segment["tokens"]],
            )
    chunk = (repo / "crates/conformance/fixtures/packs/wlc/chapters.bin").read_text(encoding="utf-8")
    for line in chunk.splitlines():
        for verse in json.loads(line)["verses"]:
            ours = packs["wlc"].get(verse["segment_id"])
            failures.equal(f"fixture wlc {verse['segment_id']}", ours and ours["text"], verse["text"])
            failures.equal(
                f"fixture tokens wlc {verse['segment_id']}",
                ours and [(t["id"], t["start"], t["end"]) for t in ours["tokens"]],
                [(t["id"], t["start"], t["end"]) for t in verse["tokens"]],
            )
    print("fixtures: the verses the conformance fixtures hold are byte-equal in the packs, tokens too")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    default = os.environ.get("OPENLOGOS_PREVIEW_PACK", str(Path.home() / ".cache/openlogos/preview-pack"))
    parser.add_argument("--pack", default=default)
    parser.add_argument("--corpora", default="/Volumes/office-hub/openlogos/corpora")
    parser.add_argument("--show", action="store_true", help="print the spread of awkward places")
    args = parser.parse_args()
    root, corpora = Path(args.pack), Path(args.corpora)
    repo = Path(__file__).resolve().parent.parent
    if not (root / "preview-pack.json").exists():
        print(f"SKIPPED: no preview pack at {root} (just preview-pack)")
        return 2
    if not (corpora / "S52-bsb" / "bsb.txt").exists():
        print(f"SKIPPED: no corpus at {corpora}")
        return 2
    try:
        import jsonschema

        schema = jsonschema.Draft202012Validator(json.loads((repo / "schemas/text-pack.schema.json").read_text()))
    except ImportError:
        schema = None
        print("NOTE: jsonschema is not on this python; the manifests are not checked against the schema")

    failures = Failures()
    packs = {}
    manifests = {}
    for name in ["bsb", "sblgnt", "wlc"]:
        manifest, order, verses = load_pack(root, name, failures, schema)
        manifests[name] = manifest
        packs[name] = verses
        check_every_verse(name, verses, failures)
        print(f"{name}: format {manifest['format_version']}, {len(order)} books, first {order[0]}, last {order[-1]}, {manifest['book_order']}, {manifest['direction']}, {manifest['scheme']}; {len(verses)} segments")
    check_bsb(corpora / "S52-bsb", packs["bsb"], failures)
    check_sblgnt(corpora / "S06-sblgnt", packs["sblgnt"], failures)
    check_wlc(corpora / "S07-morphhb", packs["wlc"], failures)
    check_fixtures(repo, packs, failures)
    for edition, verses in packs.items():
        changed = sum(1 for v in verses.values() if unicodedata.normalize("NFC", v["text"]) != v["text"])
        print(f"{edition}: {changed} segments are not NFC (kept as the source has them)")
    if args.show:
        for edition, segments in SPREAD:
            print(f"--- {edition}")
            for segment in segments:
                verse = packs[edition].get(segment)
                if verse is None:
                    print(f"{segment}: (no such segment)")
                else:
                    marks = "".join(
                        m for m, on in [("¶", verse["paragraph_start"]), ("♪", verse["poetry"]), ("T", verse["superscription"])] if on
                    )
                    print(f"{segment} [{verse['label']}] {marks} {verse['text']}")
    if failures.items:
        print(f"FAILED: {len(failures.items)} differences; the first 30:")
        for item in failures.items[:30]:
            print(item)
        return 1
    print("OK: no differences")
    return 0


if __name__ == "__main__":
    sys.exit(main())
