#!/usr/bin/env python3
"""B057: checks a built cross-reference pack against the NAS source with a reader written apart
from the importer's (the standard library's zipfile and sqlite3). `just preview-pack-xrefs-verify`.

The pack uses the coded edge storage (`edge_storage = coded-1`). This reader decodes `edge_row`
through `edge_term` itself: every published line must be one row with that line as its id, the
same source verse, the same target (a range as `first-last`), the published votes as its weight,
the constant relation, provenance and review state; nothing more, nothing less. The `edge` view
must hold the same rows. Every range must be covered by its `xref_range` segments, first to last.
The manifest must carry the attribution and the source file's hash.

Read-only on the NAS and the pack. Needs the corpus, so it is not part of `just ci`; it says
so and exits 2 when the corpus or the pack is not there. With --show it prints, for the spread
of docs/evidence/B057.md, the source's own rows best first, to set beside
`just preview-pack-xrefs-show`.
"""

import argparse
import hashlib
import io
import os
import sqlite3
import sys
import zipfile
from pathlib import Path

# OSIS book id → the BBCCCVVV book number (Paratext numbering: Matthew is 41).
BOOKS = (
    "Gen Exod Lev Num Deut Josh Judg Ruth 1Sam 2Sam 1Kgs 2Kgs 1Chr 2Chr Ezra Neh Esth Job Ps Prov "
    "Eccl Song Isa Jer Lam Ezek Dan Hos Joel Amos Obad Jonah Mic Nah Hab Zeph Hag Zech Mal"
).split()
NT = (
    "Matt Mark Luke John Acts Rom 1Cor 2Cor Gal Eph Phil Col 1Thess 2Thess 1Tim 2Tim Titus Phlm "
    "Heb Jas 1Pet 2Pet 1John 2John 3John Jude Rev"
).split()
NUMBER = {b: i + 1 for i, b in enumerate(BOOKS)} | {b: i + 41 for i, b in enumerate(NT)}
SPREAD = ["Gen.1.1", "Ps.23.1", "Isa.53.5", "John.3.16", "Rom.8.28", "Mal.4.5", "Ps.51.1",
          "Eph.3.20", "Rom.16.25"]


def verse_id(ref):
    book, chapter, verse = ref.split(".")
    return NUMBER[book] * 1_000_000 + int(chapter) * 1000 + int(verse)


def locator(target):
    if "-" in target:
        a, b = target.split("-")
        return f"{verse_id(a)}-{verse_id(b)}"
    return str(verse_id(target))


def read_source(corpora):
    directory = corpora / "S09-openbible-xrefs"
    listing = (directory / "MANIFEST.sha256").read_text()
    expected = dict(line.split("  ", 1)[::-1] for line in listing.splitlines() if line.strip())
    data = (directory / "cross-references.zip").read_bytes()
    if hashlib.sha256(data).hexdigest() != expected["cross-references.zip"]:
        raise SystemExit("cross-references.zip does not hash to MANIFEST.sha256")
    with zipfile.ZipFile(io.BytesIO(data)) as z:
        return z.read("cross_references.txt")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--pack", default=os.environ.get("OPENLOGOS_PREVIEW_PACK",
                                                         str(Path.home() / ".cache/openlogos/preview-pack")))
    parser.add_argument("--corpora", default="/Volumes/office-hub/openlogos/corpora")
    parser.add_argument("--show", action="store_true")
    args = parser.parse_args()
    pack = Path(args.pack) / "relations" / "openbible-xrefs.sqlite"
    corpora = Path(args.corpora)
    if not pack.is_file():
        print(f"SKIPPED: no cross-reference pack at {pack} (just preview-pack-xrefs)")
        return 2
    if not (corpora / "S09-openbible-xrefs").is_dir():
        print(f"SKIPPED: no corpus at {corpora}")
        return 2

    raw = read_source(corpora)
    text = raw.decode("utf-8")
    lines = text.split("\n")
    rows = {}
    for number, line in enumerate(lines[1:], start=2):
        if not line:
            continue
        source, target, votes = line.split("\t")
        rows[number] = (source, target, int(votes))

    db = sqlite3.connect(f"file:{pack}?mode=ro&immutable=1", uri=True)
    manifest = dict(db.execute("SELECT key, value FROM manifest"))
    failures = []
    if manifest.get("xref_file_sha256") != hashlib.sha256(raw).hexdigest():
        failures.append("manifest xref_file_sha256 is not the file's hash")
    if "OpenBible.info" not in manifest.get("attribution", "") or "CC BY 4.0" not in manifest.get("attribution", ""):
        failures.append("manifest attribution does not name OpenBible.info and CC BY 4.0")
    if manifest.get("scheme") != "eng":
        failures.append(f"manifest scheme {manifest.get('scheme')!r}")
    if manifest.get("xref_rows") != str(len(rows)):
        failures.append(f"manifest xref_rows {manifest.get('xref_rows')} != {len(rows)}")

    if manifest.get("edge_storage") != "coded-1":
        failures.append(f"manifest edge_storage {manifest.get('edge_storage')!r}")
    terms = {(kind, i): value for kind, i, value in db.execute("SELECT kind, id, value FROM edge_term")}
    edges = {}
    decoded = []
    for rowid, sw, src, rl, dw, dst, weight, ow, me, pr, rv in db.execute(
        "SELECT id, src_work, src_locator, rel, dst_work, dst_locator, weight, source_work, method,"
        " provenance_kind, review_state FROM edge_row"
    ):
        src_work, rel, dst_work, source_work = terms[("work", sw)], terms[("rel", rl)], terms[("work", dw)], terms[("work", ow)]
        method, provenance, review = terms[("method", me)], terms[("provenance", pr)], terms[("review", rv)]
        src, dst = str(src), str(dst)
        decoded.append((src_work, src, rel, dst_work, dst, weight, source_work, method, provenance, review))
        edges[rowid] = (src, dst, weight)
        constant = (src_work, rel, dst_work, source_work, method, provenance, review)
        if constant != ("bible", "cross_reference", "bible", "S09", "S09:votes", "curated", "source_attested"):
            failures.append(f"edge {rowid}: {constant}")
    if set(edges) != set(rows):
        failures.append(f"{len(set(rows) - set(edges))} lines without an edge, {len(set(edges) - set(rows))} edges without a line")
    mismatched = 0
    for number, (source, target, votes) in rows.items():
        edge = edges.get(number)
        if edge != (str(verse_id(source)), locator(target), float(votes)):
            mismatched += 1
            if mismatched <= 10:
                failures.append(f"line {number}: source {source}\t{target}\t{votes}, pack {edge}")
    view = db.execute(
        "SELECT src_work, src_locator, rel, dst_work, dst_locator, weight, source_work, method,"
        " provenance_kind, review_state FROM edge"
    ).fetchall()
    if sorted(view) != sorted(decoded):
        failures.append("the edge view does not hold the decoded rows")
    cover = {}
    for first, last, edge in db.execute("SELECT first, last, edge FROM xref_range"):
        cover.setdefault(edge, set()).update({first, last})
    ranges = {n: r for n, r in rows.items() if "-" in r[1]}
    for number, (_, target, _) in ranges.items():
        first, last = (verse_id(t) for t in target.split("-"))
        covered = cover.get(number, set())
        if min(covered, default=None) != first or max(covered, default=None) != last:
            failures.append(f"line {number}: {target} covered {sorted(covered)[:3]}…")
    if set(cover) != set(ranges):
        failures.append(f"xref_range names {len(set(cover) - set(ranges))} edges that are not ranges")

    print(
        f"S09: {len(rows)} lines, {len(edges)} edges, {len(ranges)} ranges, "
        f"{db.execute('SELECT count(*) FROM xref_range').fetchone()[0]} range rows; {mismatched} rows differ"
    )
    if args.show:
        for verse in SPREAD:
            listed = sorted(((v, t, n) for n, (s, t, v) in rows.items() if s == verse), key=lambda r: (-r[0], r[2]))
            print(f"{verse}: {len(listed)} rows")
            for votes, target, number in listed[:8]:
                print(f"  {votes:>5}  {target}  line {number}")
    for failure in failures:
        print("FAIL", failure)
    print("OK" if not failures else f"{len(failures)} failures")
    return 0 if not failures else 1


if __name__ == "__main__":
    sys.exit(main())
