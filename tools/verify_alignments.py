#!/usr/bin/env python3
"""B067: checks built alignment layers against Clear's files (S55) and the text packs, with
code written apart from the importer's (the standard library's sqlite3 and json).
`just preview-pack-alignment-verify`.

Read-only on the NAS, the preview pack, the morphology layers and the alignment layers. Needs
the corpus, so it is not part of `just ci`; it says SKIPPED and exits 2 when the corpus, the
packs or the layers are missing. Of S55 it opens only the BSB alignment, target and source
files. Nothing it prints goes into the repository.

Checks, per layer:
1. every Clear record is one layer record with the same id and metadata, and its source and
   target ids are the layer record's words' ids, except words the layer lists as not joined;
2. every Clear word (not punctuation) is one target word; a joined one's letters are exactly
   the pack token's letters at its range, and joined words follow Clear's order through the
   pack (verse, token, range never go back);
3. every BSB pack token of the testament has joined words or one absence row, never both;
4. every Clear source token is one source word or listed; a source word's morphology layer
   position carries its MACULA id and the same token, and its letters equal Clear's once
   points and accents are set aside (or a stated class: a movable letter, an elision mark);
5. both tokenization hashes, recomputed from the packs, equal the manifest's;
6. counts per join class, recounted.
"""

import argparse
import hashlib
import json
import sqlite3
import sys
import unicodedata
from collections import Counter
from pathlib import Path

PAIRS = [
    ("clear-sblgnt-bsb", "sblgnt-bsb.clear.sqlite", "sblgnt", "sblgnt.macula-greek.sqlite",
     "data/eng/alignments/BSB/SBLGNT-BSB-manual.json", "data/eng/targets/BSB/nt_BSB.tsv",
     "data/sources/SBLGNT+required.tsv", True),
    ("clear-wlc-bsb", "wlc-bsb.clear.sqlite", "wlc", "wlc.macula-hebrew.sqlite",
     "data/eng/alignments/BSB/WLCM-BSB-manual.json", "data/eng/targets/BSB/ot_BSB.tsv",
     "data/sources/WLCM+required.tsv", False),
]
ELISION = {"ʼ", "’", "᾽", "᾿"}


def skipped(why):
    print(f"SKIPPED: {why}")
    sys.exit(2)


def bare(text):
    """Base letters: no combining marks, no punctuation, NFD."""
    return "".join(c for c in unicodedata.normalize("NFD", text)
                   if not unicodedata.category(c).startswith("M") and c not in ELISION
                   and (c.isalnum()))


def pack(pack_dir):
    """Token id → text, in file order, and the manifest."""
    manifest = json.loads((pack_dir / "manifest.json").read_text(encoding="utf-8"))
    tokens = {}
    for book in manifest["books"]:
        for chapter in book["chapters"]:
            chunk = json.loads((pack_dir / chapter["path"]).read_text(encoding="utf-8"))
            for verse in chunk["verses"]:
                chars = list(verse["text"])
                for t in verse["tokens"]:
                    tokens[t["id"]] = "".join(chars[t["start"]:t["end"]])
    return manifest, tokens


def tokenization_hash(tokens):
    h = hashlib.sha256()
    for tid in sorted(tokens):
        h.update(f"{tid}\t{tokens[tid]}\n".encode("utf-8"))
    return h.hexdigest()


def token_id(edition, verse_id, word):
    return "%s:%02d%03d%03d%03d" % (edition, verse_id // 1000000, verse_id // 1000 % 1000, verse_id % 1000, word)


def u32s(blob):
    return [int.from_bytes(blob[i:i + 4], "little") for i in range(0, len(blob), 4)]


def tsv(path):
    lines = path.read_text(encoding="utf-8").splitlines()
    header = lines[0].lstrip("﻿").split("\t")
    return [dict(zip(header, line.split("\t"))) for line in lines[1:] if line]


def check(pair, corpus, packs, morphology, layers, failures, bsb):
    alignment_id, file, edition, morph_file, alignment_path, targets_path, sources_path, nt = pair
    say = lambda msg: failures.append(f"{alignment_id}: {msg}")
    db = sqlite3.connect(f"file:{layers / file}?mode=ro", uri=True)
    manifest = dict(db.execute("SELECT key, value FROM manifest"))
    strings = dict(db.execute("SELECT id, value FROM string"))

    # 5. Tokenization hashes.
    source_manifest, source_tokens = pack(packs / edition)
    bsb_manifest, bsb_tokens = bsb
    if tokenization_hash(source_tokens) != manifest["source_tokenization_hash"]:
        say("source tokenization hash differs from the pack's")
    if tokenization_hash(bsb_tokens) != manifest["target_tokenization_hash"]:
        say("target tokenization hash differs from the pack's")

    # Layer tables.
    targets = {}
    for tid, verse, word, s, e, ext, text, join in db.execute(
            "SELECT id, verse_id, word, span_start, span_end, external_id, text, join_class FROM target_word ORDER BY id"):
        targets[tid] = (verse, word, s, e, ext, strings[text], strings[join])
    sources = {}
    for sid, pos, verse, word, m, ms, ext, text, join in db.execute(
            "SELECT id, pos, verse_id, word, morpheme, morphemes, external_id, text, join_class FROM source_word"):
        sources[sid] = (pos, verse, word, m, ms, ext, text, strings[join])
    records = {}
    for ext, meta, s, t in db.execute("SELECT external_id, meta, sources, targets FROM record"):
        records[ext] = (json.loads(strings[meta]), [sources[i][5] for i in u32s(s)], [targets[i][4] for i in u32s(t)])
    absent_target = {(v, w): r for side, v, w, r in db.execute("SELECT side, verse_id, word, reason FROM absence") if side == 1}

    # 1. Records.
    clear = json.loads((corpus / alignment_path).read_bytes())
    target_rows = tsv(corpus / targets_path)
    source_rows = tsv(corpus / sources_path)
    words = [r for r in target_rows if r["exclude"] != "y"]
    unjoined_targets = {t[4] for t in targets.values() if t[1] == 0}
    in_layer_sources = {s[5] for s in sources.values()}
    unjoined_sources = {s[5] for s in sources.values() if s[2] == 0} | ({r["id"] for r in source_rows} - in_layer_sources)
    if len(clear["records"]) != len(records):
        say(f"{len(clear['records'])} Clear records, {len(records)} layer records")
    for r in clear["records"]:
        meta = dict(r["meta"])
        rid = meta.pop("id")
        got = records.get(rid)
        if got is None:
            say(f"record {rid} missing")
            continue
        if got[0] != meta:
            say(f"record {rid} meta {got[0]} != {meta}")
        if set(got[1]) != {s for s in r["source"] if s in in_layer_sources}:
            say(f"record {rid} sources {got[1]} != {r['source']}")
        if set(got[2]) != set(r["target"]):
            say(f"record {rid} targets {got[2]} != {r['target']}")

    # 2. Target words.
    by_ext = {t[4]: t for t in targets.values()}
    if len(by_ext) != len(targets) or set(by_ext) != {w["id"] for w in words}:
        say("target words are not Clear's words one to one")
    last = None
    classes = Counter()
    held = set()
    for w in words:
        t = by_ext.get(w["id"])
        if t is None:
            continue
        verse, word, s, e, ext, text, join = t
        classes[join] += 1
        if text != w["text"]:
            say(f"{ext}: text {text!r} != Clear's {w['text']!r}")
        if word == 0:
            continue
        tok = token_id("bsb", verse, word)
        held.add((verse, word))
        letters = bsb_tokens.get(tok)
        if letters is None:
            say(f"{ext}: token {tok} is not in the pack")
            continue
        if letters[s:e] != w["text"]:
            say(f"{ext}: {tok}[{s}:{e}] = {letters[s:e]!r}, Clear {w['text']!r}")
        here = (verse, word, s)
        if last is not None and here < last:
            say(f"{ext}: goes back in the pack ({last} then {here})")
        last = here

    # 3. Every BSB token of the testament.
    for tok in bsb_tokens:
        book = int(tok[4:6])
        if (book >= 41) != nt:
            continue
        verse = int(tok[4:12])
        word = int(tok[12:15])
        has = (verse, word) in held
        why = absent_target.get((verse, word))
        if has == (why is not None):
            say(f"{tok}: {'both words and an absence' if has else 'neither words nor an absence'}")

    # 4. Source words against the morphology layer.
    mdb = sqlite3.connect(f"file:{morphology / morph_file}?mode=ro", uri=True)
    layer = {pos: (ext, verse, word, surface) for pos, ext, verse, word, surface in mdb.execute(
        "SELECT t.pos, a.external_id, t.verse_id, t.word, t.surface FROM token t JOIN analysis a ON a.pos = t.pos")}
    if manifest["source_morphology_revision"] != dict(mdb.execute("SELECT key, value FROM manifest"))["layer_revision"]:
        say("the morphology layer is not the revision the alignment was joined to")
    source_classes = Counter()
    by_source = {s[5]: s for s in sources.values()}
    for row in source_rows:
        s = by_source.get(row["id"])
        if s is None:
            source_classes["(not in layer)"] += 1
            continue
        pos, verse, word, m, ms, ext, text, join = s
        source_classes[join] += 1
        lext, lverse, lword, surface = layer[pos]
        if lext != ext or (lverse, lword) != (verse, word):
            say(f"{ext}: layer position {pos} is {lext} {lverse}/{lword}")
        if text != row["text"]:
            say(f"{ext}: text {text!r} != Clear's {row['text']!r}")
        a, b = bare(text), bare(surface)
        if a != b and not (join == "movable_letter_omitted" and b[:len(a)] == a and len(b) == len(a) + 1):
            say(f"{ext}: letters {text!r} vs layer {surface!r} ({join})")

    aligned_ids = {x for r in records.values() for x in r[2]}
    unaligned_targets = sum(1 for w in words if w["id"] not in aligned_ids)
    print(f"{alignment_id}: {len(records)} records; {len(words)} Clear words "
          f"({len(words) - len(unjoined_targets)} joined, {unaligned_targets} unaligned); "
          f"{len(source_rows)} Clear source tokens ({len(source_rows) - len(unjoined_sources)} joined)")
    print(f"  target classes {dict(sorted(classes.items()))}")
    print(f"  source classes {dict(sorted(source_classes.items()))}")


def main():
    home = Path.home() / ".cache/openlogos"
    parser = argparse.ArgumentParser()
    parser.add_argument("--corpora", default="/Volumes/office-hub/openlogos/corpora")
    parser.add_argument("--out", default=str(home / "preview-pack"))
    parser.add_argument("--morphology", default=str(home / "morphology"))
    parser.add_argument("--alignments", default=str(home / "alignments"))
    args = parser.parse_args()
    corpus = Path(args.corpora) / "S55-clear-alignments"
    packs = Path(args.out) / "packs"
    morphology = Path(args.morphology) / "layers"
    layers = Path(args.alignments) / "layers"
    for path, what in [(corpus / "data", "the S55 corpus"), (packs / "bsb", "the text packs"),
                       (morphology, "the morphology layers"), (layers, "the alignment layers")]:
        if not path.exists():
            skipped(f"{what} not found at {path}")
    failures = []
    bsb = pack(packs / "bsb")
    for pair in PAIRS:
        check(pair, corpus, packs, morphology, layers, failures, bsb)
    for f in failures[:50]:
        print("FAIL", f)
    if failures:
        print(f"{len(failures)} failures")
        sys.exit(1)
    print("OK")


if __name__ == "__main__":
    main()
