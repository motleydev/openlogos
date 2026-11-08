#!/usr/bin/env python3
"""B026: checks built morphology layers against MACULA's TSVs and the text packs, with code
written apart from the importer's (the standard library's sqlite3, csv and json).
`just preview-pack-morphology-verify`.

Read-only on the NAS, the preview pack and the layers. Needs the corpus, so it is not part of
`just ci`; it says SKIPPED and exits 2 when the corpus, the packs or the layers are missing.
The TSVs are read by column name through an allowlist: the columns the rights records restrict
(`ln`, `domain`; `sdbh`, `lexdomain`, `coredomain`, `contextualdomain`) are never looked at.
Nothing it prints goes into the repository.

Checks, per layer:
1. every TSV row is one layer position, by `xml:id`, and the layer holds nothing else;
2. each position's lemma and morphology code equal the row's;
3. every text-pack token has positions or one absence row, never both;
4. a token joined `exact` is its morphemes' letters run together, and their ranges tile it;
5. a token joined by the Greek letter rules has the same letters once punctuation (and, for
   `normalization`, NFC; for `diacritics`, accents and case) are set aside;
6. a token whose analysis is a qere carries no Hebrew point (it is the unpointed ketiv);
7. the layer's manifest names the excluded columns, and no table has a column by those names.
"""

import argparse
import csv
import json
import sqlite3
import sys
import unicodedata
from collections import Counter, defaultdict
from pathlib import Path

LAYERS = [
    ("sblgnt", "sblgnt.macula-greek.sqlite", "S47-macula-greek/SBLGNT/tsv/macula-greek-SBLGNT.tsv",
     ["xml:id", "ref", "lemma", "morph", "text"], {"ln", "domain"}),
    ("wlc", "wlc.macula-hebrew.sqlite", "S47-macula-hebrew/WLC/tsv/macula-hebrew.tsv",
     ["xml:id", "ref", "lemma", "morph", "text"], {"sdbh", "lexdomain", "coredomain", "contextualdomain"}),
]
ELISION = {"ʼ", "’", "᾽", "᾿"}


def skipped(why):
    print(f"SKIPPED: {why}")
    sys.exit(2)


def letters(text):
    return "".join(c for c in text if c not in ELISION and (c.isalnum() or unicodedata.category(c).startswith("M")))


def base(text):
    return "".join(c for c in unicodedata.normalize("NFD", text) if not unicodedata.category(c).startswith("M")).lower()


def pack_tokens(pack_dir):
    manifest = json.loads((pack_dir / "manifest.json").read_text(encoding="utf-8"))
    tokens = {}
    for book in manifest["books"]:
        for chapter in book["chapters"]:
            chunk = json.loads((pack_dir / chapter["path"]).read_text(encoding="utf-8"))
            for verse in chunk["verses"]:
                chars = list(verse["text"])
                for t in verse["tokens"]:
                    tokens[t["id"]] = "".join(chars[t["start"]:t["end"]])
    return tokens


def token_id(edition, verse_id, word):
    return "%s:%02d%03d%03d%03d" % (edition, verse_id // 1000000, verse_id // 1000 % 1000, verse_id % 1000, word)


def check(edition, layer_path, tsv_path, wanted, restricted, pack_dir, failures):
    say = lambda msg: failures.append(f"{edition}: {msg}")
    db = sqlite3.connect(f"file:{layer_path}?mode=ro", uri=True)
    features = {name: fid for fid, name in db.execute("SELECT id, name FROM feature")}
    values = defaultdict(dict)
    for fid, vid, value in db.execute("SELECT feature_id, value_id, value FROM feature_value"):
        values[fid][vid] = value
    columns = {}
    for fid, blob in db.execute("SELECT feature_id, value_ids FROM token_column"):
        columns[fid] = [int.from_bytes(blob[i:i + 4], "little") for i in range(0, len(blob), 4)]
    strings = dict(db.execute("SELECT id, value FROM string"))
    rows = db.execute(
        "SELECT t.pos, t.verse_id, t.word, t.surface, a.external_id, a.lemma, a.span_start, a.span_end, a.reading, j.name "
        "FROM token t JOIN analysis a ON a.pos = t.pos JOIN join_class j ON j.id = a.join_class ORDER BY t.pos"
    ).fetchall()
    absent = {(v, w): r for v, w, r in db.execute("SELECT verse_id, word, reason FROM absence")}
    manifest = dict(db.execute("SELECT key, value FROM manifest"))

    # 7. Excluded columns.
    if set(manifest.get("excluded_columns", "").split(",")) != restricted:
        say(f"manifest excluded_columns {manifest.get('excluded_columns')!r}")
    for (table,) in db.execute("SELECT name FROM sqlite_schema WHERE type = 'table'"):
        for col in db.execute(f'PRAGMA table_info("{table}")'):
            if col[1] in restricted:
                say(f"table {table} has a column {col[1]}")
    if set(features) & restricted:
        say(f"features {set(features) & restricted}")

    # 1, 2. The TSV row by row.
    by_id = {r[4]: r for r in rows}
    if len(by_id) != len(rows):
        say("an external id is held twice")
    seen = 0
    with open(tsv_path, encoding="utf-8", newline="") as handle:
        reader = csv.reader(handle, delimiter="\t", quoting=csv.QUOTE_NONE)
        header = next(reader)
        index = {name: header.index(name) for name in wanted}
        for cells in reader:
            if not cells:
                continue
            seen += 1
            xml_id = cells[index["xml:id"]]
            row = by_id.get(xml_id)
            if row is None:
                say(f"{xml_id} is not in the layer")
                continue
            pos = row[0]
            lemma = row[5] and strings[row[5]] or values[features["lemma"]].get(columns[features["lemma"]][pos], "")
            if unicodedata.normalize("NFC", lemma) != unicodedata.normalize("NFC", cells[index["lemma"]]):
                say(f"{xml_id} lemma {lemma!r} is not {cells[index['lemma']]!r}")
            morph = values[features["morph"]].get(columns[features["morph"]][pos], "")
            if morph != cells[index["morph"]]:
                say(f"{xml_id} morph {morph!r} is not {cells[index['morph']]!r}")
            if row[3] != cells[index["text"]]:
                say(f"{xml_id} surface {row[3]!r} is not {cells[index['text']]!r}")
    if seen != len(rows):
        say(f"{seen} TSV rows, {len(rows)} layer positions")

    # 3–6. Token by token.
    tokens = pack_tokens(pack_dir)
    joined = defaultdict(list)
    for r in rows:
        if r[2]:
            joined[token_id(edition, r[1], r[2])].append(r)
    classes = Counter()
    for tid, text in tokens.items():
        verse_id = int(tid.split(":")[1][:8])
        word = int(tid.split(":")[1][8:11])
        have, why = joined.get(tid), absent.get((verse_id, word))
        if bool(have) == bool(why):
            say(f"{tid}: positions {bool(have)}, absence {why!r}")
            continue
        if not have:
            classes["absent:" + why] += 1
            continue
        kinds = {r[9] for r in have}
        classes[",".join(sorted(kinds))] += 1
        if kinds == {"exact"}:
            if "".join(r[3] for r in have) != text:
                say(f"{tid}: morphemes {''.join(r[3] for r in have)!r} are not {text!r}")
            at = 0
            for r in have:
                if r[6] != at or r[7] != at + len(r[3]):
                    say(f"{tid}: span {r[6]}..{r[7]} at {at}")
                at += len(r[3])
        elif kinds & {"qere", "qere_for_ketiv_run"}:
            if any("֑" <= c <= "ׇ" and c not in "־׀׃׆" for c in text):
                say(f"{tid}: a qere is joined to a pointed token {text!r}")
            if any(r[8] != 1 for r in have):
                say(f"{tid}: a qere join that does not read as qere")
        else:
            (kind,) = kinds
            s, p = letters(have[0][3]), letters(text)
            ok = {"punctuation": s == p, "elision": s == p,
                  "normalization": unicodedata.normalize("NFC", s) == unicodedata.normalize("NFC", p),
                  "diacritics": base(s) == base(p)}.get(kind)
            if not ok:
                say(f"{tid}: class {kind} does not hold for {have[0][3]!r} and {text!r}")
    for (v, w) in absent:
        if token_id(edition, v, w) not in tokens:
            say(f"absence for {token_id(edition, v, w)}, which the pack does not have")
    print(f"{edition}: {seen} TSV rows = {len(rows)} positions; {len(tokens)} pack tokens: "
          + ", ".join(f"{k} {n}" for k, n in sorted(classes.items())))


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    home = Path.home() / ".cache/openlogos"
    parser.add_argument("--corpora", type=Path, default=Path("/Volumes/office-hub/openlogos/corpora"))
    parser.add_argument("--preview-pack", type=Path, default=home / "preview-pack")
    parser.add_argument("--morphology", type=Path, default=home / "morphology")
    args = parser.parse_args()
    failures = []
    for edition, layer, tsv, wanted, restricted in LAYERS:
        layer_path = args.morphology / "layers" / layer
        tsv_path = args.corpora / tsv
        pack_dir = args.preview_pack / "packs" / edition
        for path in (layer_path, tsv_path, pack_dir / "manifest.json"):
            if not path.exists():
                skipped(f"{path} is not there")
        check(edition, layer_path, tsv_path, wanted, restricted, pack_dir, failures)
    for f in failures[:50]:
        print("FAIL", f)
    if failures:
        print(f"{len(failures)} differences")
        sys.exit(1)
    print("OK: no differences")


if __name__ == "__main__":
    main()
