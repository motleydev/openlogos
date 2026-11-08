# The alignment layers (B067)

Clear's manual word alignments between the BSB and the Greek and Hebrew (S55, CC BY 4.0), as one
layer per testament keyed to the tokens of the preview pack's BSB, SBLGNT and WLC text packs
(docs/preview-pack.md), through the MACULA morphology layers (docs/morphology.md). For a BSB word
or selection: the Greek or Hebrew words it renders, with lemma, gloss and parsing. For a Greek or
Hebrew word: its BSB words. For a lemma: how the BSB renders it, with counts, scope and
denominator. Nothing here is ever in the repository: the output is outside it, and the importer
refuses an output inside the repository, the corpora, the preview pack or the morphology layers.

```sh
just preview-pack                        # the text packs, if they are not built
just preview-pack-morphology             # the morphology layers (their ids are the join key)
just preview-pack-alignment              # about 18 s, 3.5 s of it the rights gate over the NAS
just preview-pack-alignment-verify       # a second reader against Clear's files and the packs
just preview-pack-alignment-bench        # lookups (16 ms) and rendering tables (50 ms), natively
just preview-pack-alignment-bench-wasm   # the same in a Chrome worker (web shell)
cargo run --release -p openlogos-preview-pack -- alignment-show John.3.16 Gen.1.1
cargo run --release -p openlogos-preview-pack -- alignment-renderings λόγος חֶסֶד --book John
```

Inputs, read-only: `/Volumes/office-hub/openlogos/corpora/S55-clear-alignments/` (`--corpora`),
`rights/` (`--rights`), the built preview pack (`--out`) and morphology layers (`--morphology`).
S55 passes the B071 gate first (status, permissions, both manifest copies, every listed file's
hash). Of its files only six are parsed: `data/eng/alignments/BSB/{SBLGNT,WLCM}-BSB-manual.json`,
`data/eng/targets/BSB/{nt,ot}_BSB.tsv` and `data/sources/{SBLGNT,WLCM}+required.tsv`. YLT, BGNT
and the catalog are hashed by the gate and never read; the ESV, CSB, LEB and NET alignments the
catalog names were never fetched.

Output, default `~/.cache/openlogos/alignments` (`OPENLOGOS_ALIGNMENTS` or `--alignments`),
replaced only when the whole build passed:

```
layers/sblgnt-bsb.clear.sqlite     clear-sblgnt-bsb: SBLGNT (NT) to BSB
layers/wlc-bsb.clear.sqlite        clear-wlc-bsb: WLC (OT, Clear's WLCM ids) to BSB
joins/clear-*.json                 every place of every join class but the plain ones
alignments.json                    counts, classes, record shapes, sizes, timings, manifests
bench-samples.json                 written by the native bench, replayed by the wasm one
```

**The layer** (`openlogos-alignment-1`, `crates/retrieval-sqlite/src/alignment.rs`). Each Clear
record is one record, many-to-many, possibly discontinuous, with Clear's id (`43003016.012`) and
its metadata as given (`origin: manual`, `status: created`); the layer's own origin is
`curated:clear-alignments@c99bd0a` (specs/03). Source words are morphemes of the morphology layer,
keyed by its positions, so a Hebrew prefix or suffix is its own word (`wlc:01001001001a`); target
words are Clear's words with their scalar range inside a BSB token (`ground` and `because` in
`ground—because`). A word in no record is *unaligned*; a word that could not be joined to a pack
token stays in its record with no token id and its join class. The manifest carries both sides'
tokenization hashes (SHA-256 of `<token id>\t<surface>\n` per token in id order), the text packs'
revisions and the morphology layer revision the source words were keyed to; a different
morphology layer makes lemma answers unavailable and leaves token answers without morphology.

**Hosts.** `ShellHandle.use_alignments(<out>)` natively, or `useAlignments([Uint8Array, ...])` with
the fetched `layers/*.sqlite` in a browser; attach the morphology layers too (`use_morphology`)
for lemma and gloss. Then:

- `token_alignment([token_id, ...])` / `tokenAlignment`: per token, `status` `aligned`,
  `unaligned`, `not_joined` (with `reason`: `not_in_clear_target`, `not_in_alignment_source`,
  `unknown_token`, or the morphology layer's reason) or `no_layer`, and Clear's words inside it;
  then every record they are in (`groups`), with both sides' words, the source words' morphology,
  Clear's metadata and `discontinuous`. Works from either side.
- `lemma_renderings(editions, query, book)` / `lemmaRenderings`: one table per matched lemma and
  layer: `scope` (`SBLGNT, John`), `occurrences` (the denominator), `aligned`, `unaligned`,
  `not_covered`, then rows of renderings (the record's BSB words, `…` between runs) with counts,
  how many share their record with other source words, and examples; the unaligned row last.
  Every answer carries the note that renderings are evidence of translation choices, not
  definitions.

Show the attribution with any answer: CC BY 4.0 requires it.

Every decision, count, mismatch and measurement: docs/evidence/B067.md.
