# Deterministic search and cross references

## Release scope

Local search and cross references are core study capabilities, not features contingent on an LLM account or the P4 enrichment pipeline. P1 provides reference navigation, scoped literal word/phrase search and a small approved cross-reference layer with declared coverage. P4 adds original-language filters and richer evidence only where cleared datasets and evaluation permit them. Partial coverage is stated, never filled with generated links.

## Query contract

- A reference parser resolves book aliases and ranges against the selected edition and its division system. Ambiguous abbreviations offer choices. Mixed German/English book aliases are useful even before full German UI localization.
- Plain words match all query terms within a verse by default. Quoted phrases require adjacent normalized tokens in order. The interface states the search unit and has a separate later passage-span search option; it does not silently broaden a verse query across chapters.
- P1 supports explicit AND, OR, NOT and parentheses with documented precedence (NOT, AND, OR), exact phrases, book/range and edition scope. Invalid or unsupported syntax gives a local error and recovery example; it never falls through to AI. Quoted operator words search literally.
- A small query builder expresses these filters and shows the equivalent query. Parse to a bounded AST; compile parameterized queries, never execute raw user SQL. Set length/depth/result limits and cancellable search behavior.
- Search normalization is language-specific, versioned and separate from displayed/source bytes. A mode may ignore case or diacritics, but it never alters exported Scripture or conflates different Hebrew/Greek tokens without explanation. Substring, fuzzy and transliteration expansion are explicit options when implemented.
- Results show edition, native reference, matching spans and enough context to inspect the hit. Preserve the query and result position on Back. Count verse results and token occurrences separately. No claimed full-library total while indexes are incomplete.
- P4 query filters for lemma/morphology use namespaced IDs and explicit dataset coverage. Speaker, semantic role, topic and sense filters remain unavailable unless the installed reviewed layer supports them. A guessed model tag is never an exact filter.

Default scope is the chosen Bible edition within the Protestant canon. A broader scope is an explicit choice. Reformed editorial preferences cannot remove lexical hits from that chosen scope. Search timing targets in specs/01-product.md apply to the named benchmark corpus, not an unlimited library.

## Cross-reference contract

Acquire exact artifacts for a curated, attributable baseline (B003/B057). OpenBible and historical cross-reference resources are candidates, not licenses or theological endorsements by themselves [S09]. Publisher footnotes and lexical databases visible in the screenshots cannot be scraped into our dataset. A human reviews representative links and the published scope before release.

Each record includes source edition/locator, source passage selector, target passage selector, relation kind, original direction, coverage and review status. Kinds include imported traditional cross reference, explicit citation, quotation, allusion, lexical parallel and thematic connection. If the upstream collection does not identify a kind, use imported cross reference/unspecified; do not upgrade it to an explicit citation. Machine suggestions occupy a separate optional list and never enter the reviewed default layer automatically.

Deduplicate visible target passages while preserving every source assertion. Reverse lookup is a derived navigation aid and labeled as such; it is not proof the upstream source asserted a bidirectional relation. Interpretive topical links record their editorial scheme and Reformed/confessional context where applicable. Frequency, crowdsourced votes or doctrinal fit do not establish textual dependence.

Reference mapping retains exact/partial/ambiguous/unmapped states across editions. An unavailable target can show metadata with a clear missing-resource action; it must not silently display a different translation. A fetched passage preview uses installed or explicitly authorized content, respects quotation rights, and adds an evidence-bearing link to notes. The linked reading pane stays fixed unless the reader deliberately navigates it.

## Acceptance examples

B056 uses a deterministic, locally stored expected-result fixture covering case, combining marks, phrase order, AND/OR/NOT precedence, ambiguous references, unsupported syntax, range/edition filters, no results and incomplete indexes. Model-generated results are not test oracles. Include hostile query inputs and offline operation with the research adapter disabled.

B057 includes overlapping links from two sources, an unspecified imported relation, one partial or unmapped reference, a reverse lookup, an unavailable target edition and an unreviewed suggestion. The UI preserves attribution and cannot make an unreviewed thematic link look like an explicit biblical quotation. Human source review is required for published corpus coverage.

B031 later adds multiword alignments, repeated lemma occurrences in one verse, homographs, ambiguous morphology and unknown token coverage to word-study evaluations. Tests verify that denominators and the occurrence table agree. They do not certify the linguistic correctness of the source dataset; that remains a separate human-reviewed gate.
