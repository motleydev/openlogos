# The morphology layers (B026)

MACULA Greek and MACULA Hebrew (S47) as one layer per edition, keyed to the tokens of the preview
pack's SBLGNT and WLC text packs (docs/preview-pack.md). Lemma, morphology, gloss and the layer's
attribution for any token; lemma search over an edition. Nothing here is ever in the repository:
the output is outside it, and the importer refuses an output inside the repository, the corpora or
the preview pack (which `just preview-pack` replaces whole).

```sh
just preview-pack                        # the text packs first, if they are not built
just preview-pack-morphology             # about 25 s, half of it the rights gate over the NAS
just preview-pack-morphology-verify      # a second reader against the TSVs and the text packs
just preview-pack-morphology-bench       # lookup and lemma search p95 natively, against 50 ms
just preview-pack-morphology-bench-wasm  # the same in a Chrome worker (web shell)
cargo run --release -p openlogos-preview-pack -- morphology-show Gen.1.1 Dan.2 Ruth.3.12
```

Inputs, read-only: `/Volumes/office-hub/openlogos/corpora/{S47-macula-greek,S47-macula-hebrew,S07-morphhb}/`
(`--corpora`), `rights/` (`--rights`) and the built preview pack (`--out`, default
`~/.cache/openlogos/preview-pack`). Each source passes the B071 gate first: `status: approved`,
the permissions, `MANIFEST.sha256` against the record and `rights/<id>/manifest.sha256`, every
listed file's hash. S07 (the OSHB the WLC pack was built from) is read only for its ketiv and qere
markup, so qere words join the ketiv tokens they are read for.

**Restricted columns.** The rights records flag MACULA's Louw-Nida columns (`ln`, `domain`) and
its SDBH columns (`sdbh`, `lexdomain`, `coredomain`, `contextualdomain`): UBS data used by
permission, not CC BY. They never enter a layer, an index or a fixture. The build reads the flag
from each record and stops if it names other columns than the importer excludes; the TSV reader
reads through an allowlist; tests build layers from snippets whose restricted cells hold markers
and look for them in every output byte. Each layer's manifest lists the columns it excludes.

Output, default `~/.cache/openlogos/morphology` (`OPENLOGOS_MORPHOLOGY` or `--morphology`),
replaced only when the whole build passed:

```
layers/sblgnt.macula-greek.sqlite      one layer per edition: a B064 pack (token, feature
layers/wlc.macula-hebrew.sqlite        columns, roaring postings) plus analysis, string,
                                       join_class and absence tables
joins/sblgnt.json, joins/wlc.json      every place of every join class but `exact`
morphology.json                        counts, classes, sizes, timings, the layers' manifests
bench-samples.json                     written by the native bench, replayed by the wasm one
```

A layer names the text pack it was joined to (`text_content_revision`, the manifest's SHA-256);
rebuild the layers after rebuilding the text packs from other bytes.

**The layer.** A position is a morpheme: one per Greek word; one or more per Hebrew word, where
prefixes and suffixes are their own positions with their scalar range in the token, and the
article a prefix's vowel implies (`בָּעוֹף`) is a position with no letters. Morpheme ids follow
`core::identity::mint`: `wlc:01001001001a`, `…b`; a one-morpheme token keeps the token's id.
MACULA's `xml:id` is kept as the external key. Features with dictionary columns and postings:
`lemma`, `lemma_key` (the lemma's `ol-norm-1` search key), `strong`, `morph`, `pos`,
`language` (`grc`, `hbo`, `arc`), and the decoded parsing fields (Greek `type person number
gender case tense voice mood degree`; Hebrew and Aramaic `type person gender number state stem`).

**Hosts.** `ShellHandle.use_morphology(<out>)` natively, or `useMorphology([Uint8Array, ...])`
with the fetched `layers/*.sqlite` in a browser: attaches the layers and enables Search lemma on
the words of their editions (until then the row says "Needs the lexicon import"; with layers, a
word of another edition says "Needs a morphology layer for BSB"). Then:

- `token_analysis(token_id)` / `tokenAnalysis`: per layer, the reading analysed (`written`;
  `qere` for a ketiv token, with the qere's letters in `text`), the join class, the morphemes
  (lemma, Strong's, part of speech, code, decoded fields, `description` in words, gloss, English,
  transliteration) and the attribution to show. `status` is `analyzed`, `conflicting` (two
  layers disagree; both are returned) or `absent` with a reason (`no_layer`, `ketiv_not_read`,
  `ketiv_read_with_previous_token`, `surface_differs`, `not_analyzed`).
- `lemma_search(editions, query, limit)` / `lemmaSearch`: exact when the query is a lemma as
  spelled, else by search key (`ראשית` finds the `רֵאשִׁית` inside `בְּרֵאשִׁית`; `λογος` every
  form of `λόγος`). Homographs that share a key stay separate lemmas with their counts.
- `lemma_search_token(token_id, limit)` / `lemmaSearchToken`: Search lemma from a word, by its
  last morpheme that is not a suffix. The menu row carries the word's `reference` and
  `resource_id`; the host finds the token in the verse it rendered.

Show the attribution with any analysis: CC BY 4.0 requires it.

Every decision, count, mismatch and measurement: docs/evidence/B026.md.
