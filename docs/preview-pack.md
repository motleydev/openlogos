# The preview pack (B071)

The BSB, SBLGNT and WLC as text packs (specs/23 §4) and FTS5 search packs (ADR 0002), built on
this machine from the approved artifacts on the NAS. Nothing here is ever in the repository: the
output is outside it, and the importer refuses an output inside the repository or the corpora.

```sh
just preview-pack                 # from the core shell; about 10 s plus the first compile
just preview-pack-verify          # a second set of parsers against the NAS; --show prints a spread
just preview-pack-bench           # text search p95 natively, against 50 ms (specs/22 §7)
just preview-pack-bench-wasm      # the same in a Chrome worker (web shell)
```

Inputs, read-only: `/Volumes/office-hub/openlogos/corpora/{S52-bsb,S06-sblgnt,S07-morphhb}/`
(`--corpora` to change) and `rights/` (`--rights`). Before any text is parsed, each source must
have `status: approved` and the needed permissions in its rights record, a `MANIFEST.sha256` that
hashes to the record's `artifact_sha256` and equals `rights/<id>/manifest.sha256`, and every listed
file must hash to what is listed; otherwise the build stops and names what failed. The parsers read
the bytes that were hashed.

Output, default `~/.cache/openlogos/preview-pack` (`OPENLOGOS_PREVIEW_PACK` or `--out`), replaced
only when the whole build passed:

```
packs/{bsb,sblgnt,wlc}/manifest.json, chapters/<Book>/<C>.json    text packs (specs/23 §4)
search/{bsb,sblgnt,wlc}.sqlite                                    B064 packs: text + text_fts
preview-pack.json                                                 hashes, counts, what was left
                                                                  out, every difference from the
                                                                  canon table, build time
```

Hosts: `ReadingHandle.open_pack(directory)` (natively) or `addPackManifest` + `supplyChunk` (a
browser) for each `packs/<id>`; `ShellHandle.use_edition_search(<out>/search)` natively, or
`useEditionSearch([Uint8Array, ...])` with the fetched `search/*.sqlite` in a browser. Show each
edition's `attribution`: the CC BY text of the SBLGNT and the OSHB requires it.

What each source contributes and every decision about the text: docs/evidence/B071.md.

A test that needs the corpus never runs in `just ci`: `just preview-pack-verify` and
`just preview-pack-bench-wasm` need the built pack, and say `SKIPPED` without it.

## Cross references (B057)

The OpenBible cross references (S09) go beside the text packs, built on their own:

```sh
just preview-pack-xrefs             # about 6 s plus the first compile; after just preview-pack
just preview-pack-xrefs-verify      # a second reader against the NAS; --show prints the spread
just preview-pack-xrefs-show        # the spread through the core API (--limit N, --preview)
just preview-pack-xrefs-bench       # a verse's list latency natively, against 16 ms
just preview-pack-xrefs-bench-wasm  # the same in a Chrome worker (web shell)
```

Input, read-only: `S09-openbible-xrefs/` under the corpora, through the same rights and hash gate
(`rights/S09.json`). Output, written in place and kept by later `just preview-pack` runs:

```
relations/openbible-xrefs.sqlite    a B064 pack: one curated `cross_reference` edge per published
                                    line (rowid = line, weight = votes as published, a range
                                    target as `first-last`), `xref_cover` for reverse lookups
                                    into ranges, `xref_*` manifest keys (kind, direction, votes,
                                    coverage, the source file's hash)
relations/openbible-xrefs.json      the fidelity report: scheme check, duplicates, reversed
                                    pairs, ranges, votes, every target placed in each text pack
```

The references are in `eng`; each list is mapped into the edition asked about at read time, so
one pack serves the BSB, SBLGNT and WLC. Hosts: `ReadingHandle.open_cross_references(<out>/
relations/openbible-xrefs.sqlite)` natively, or `useCrossReferences(Uint8Array)` with the fetched
file in a browser; then `ShellHandle.set_data_layers(["cross_references"])`, which enables the
"Cross references" row of the context menu and palette. `cross_references(resource_id, passage,
…)` returns a page of the list (25 fills a panel); open an item with `tab.open_edition`
(`resource_id`, the item's `target`), which leaves the main passage where it is. Show each item's
`attribution` (CC BY 4.0), its `direction` when it is `reverse_derived`, and its `availability`
when the edition does not print the target. Decisions and counts: docs/evidence/B057.md.
