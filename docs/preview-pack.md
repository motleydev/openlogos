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

On the Mac: `-OPENLOGOS_PREVIEW_PACK <dir>` (`none` for the placeholder library), then this
variable, then the default; apps/apple/README.md. What each source contributes and every decision
about the text: docs/evidence/B071.md; the Mac app over it: docs/evidence/B071-mac.md.

A test that needs the corpus never runs in `just ci`: `just preview-pack-verify` and
`just preview-pack-bench-wasm` need the built pack, and say `SKIPPED` without it.
