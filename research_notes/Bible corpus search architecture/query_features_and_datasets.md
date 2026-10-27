# Query features and datasets in scholarly Bible and ancient-text corpus tools

Scope: which query types and relation types existing tools offer, which data and index technique supports each, and which open datasets provide each layer (with licenses). Research date: 2026-10-27. Some figures were computed locally from downloaded primary data files. Those are marked "(computed)".

## Q1. What query features do Logos, Accordance, BibleWorks, STEPBible, Text-Fabric, CQP, ANNIS and MACULA expose?

### Takeaway
All of these tools build on the same few query primitives:
- **Token predicates:** lemma, surface form, morphology fields, Strong's number, semantic domain.
- **Sequence and proximity:** BEFORE, AFTER, WITHIN n words, inside verse or clause.
- **Feature agreement between tokens:** Accordance AGREE, Text-Fabric `.f.`.
- **Containment in structural units:** phrase, clause, sentence, verse.
- **Tree or edge relations:** dominance, subject-of, referent-of.

Commercial tools (Logos, Accordance) wrap these in graphical builders (Syntax Search, Construct window). Academic tools (Text-Fabric, CQP, ANNIS) expose them as a text query language. The driving question ("ἀγάπη/ἀγαπάω vs φιλέω with English renderings") needs a lemma predicate, an alignment layer, and faceted counts. It needs no syntax engine.

### Cited Findings
**Logos**
- Logos has separate search types: Morph Search for grammatical features of words in tagged Greek NT, Hebrew Bible and LXX texts (lemma, part of speech, case, tense, mood), Syntax Search, and Clause Search. — [Logos search results summary / support](https://support.logos.com/hc/articles/360039621591); [Logos syntax search](https://faithlife.com/logos-syntax-search)
- Clause Search searches every sentence clause "for terms acting in specific grammatical or semantic roles". Logos's example is finding where Jesus is the subject and Peter the object. It requires a Logos Max subscription or the former Bronze feature set or higher. — [Logos syntax search page](https://faithlife.com/logos-syntax-search) (via search snippet)
- Example Morph Search syntax from a user: `lemma:διά@P BEFORE 1 WORD @NG` finds a preposition followed immediately by a genitive noun. This shows lemma predicates, morph-code predicates and positional operators combined in one query. — [Logos community forum](https://community.logos.com/discussion/121385)
- Clause and Syntax search work only on texts that have syntax analysis. One user reported that only SBLGNT, LXX Swete and LBH were searchable in the Clause tab. — [Logos forum](https://community.logos.com/discussion/comment/1238195)
- A suggested workaround for queries Clause search cannot express is a Morph search that combines a Louw-Nida domain with a nominative noun. So Logos lets users query by semantic domain. — [Logos forum](https://community.logos.com/discussion/comment/1238204)

**Accordance**
- The Construct window is graphical. Columns hold word items (NOUN, PREPOSITION…), and connectors are dragged across columns. WITHIN n words sets a proximity limit. AGREE requires matching features between columns (Lexical Form, Case…). A user's noun–preposition–noun example used one AGREE on lemma and another on case. — [Accordance forum](https://forums.accordancebible.com/topic/7781-want-to-find-noun-preposition-same-now/); [Accordance help: AGREE connecting item](https://accordancefiles1.com/helpfiles/OSX13/content/topics/11_ggtt/the_agree_connecting_item.htm)
- Users report false hits and missed hits from AGREE, even with "search both directions" turned on. Agreement queries are hard to get right. — [Accordance forum 7781](https://forums.accordancebible.com/topic/7781-want-to-find-noun-preposition-same-now/)
- WITHIN can also be used in the plain Search window. In one worked example it cut 122 hits to 112. — [DTS library Accordance guide](https://library.dts.edu/Pages/ER/EBS/accordance.pdf) (via search snippet)
- A comparison of Logos and Accordance graphical searches. — [three-things.ca](https://three-things.ca/?p=108)

**Text-Fabric (ETCBC BHSA, N1904)**
- The search template language uses indentation for embedding, for example `chapter` → `sentence` → `word pos=verb`. — [TF search usage](https://annotation.github.io/text-fabric/tf/about/searchusage.html)
- Feature conditions: `pos=verb|noun`, `sp#verb` (not), `gn` (has a value), numeric `n>3`, regex `lex~^walk`, and `pos*` (report the feature without restricting it). — same
- Relations:
  - node identity and order: `= # < >`
  - slot relations: `==` same slots, `&&` overlap, `##` different slots, `||` disjoint, `[[ ]]` embedding
  - position: `<< >>` before/after, `<: :>` immediately adjacent, `=:` same start, `:=` same end, `::` same span
  - within k slots: `<k: =k: :k= :k:`
  - same source as all three points above
- Feature agreement between nodes uses `.f.`, `.f=g.`, `.f#g.`, `.f<g.`, `.f~regex~g.`. This is the equivalent of Accordance AGREE. — same
- Edge relations (`-sub>`, `<crossref>95>`, either direction with `<name>`) traverse typed graph edges, including value-filtered edges. — same
- Quantifiers: `/without/`, `/with/…/or/`, `/where/…/have/`. Example: "clauses where every predicate phrase contains a verb". Each quantifier runs separate searches whose results are combined by set difference, intersection or union, and the result feeds the main search as a custom node set. — same

**CQP / Corpus Workbench and ANNIS**
- CWB (Evert & Hardie 2011) is "a set of tools for indexing, managing and querying very large corpora with multiple layers of word-level annotation". — [Evert & Hardie CL2011](https://cwb.sourceforge.io/files/EvertHardie_CL2011.pdf); [Lancaster eprint](https://eprints.lancs.ac.uk/id/eprint/62721/)
- ANNIS AQL searches for annotation attributes and the relations between them. Nodes can be tokens (`tok="dogs"`) or attribute-value pairs, optionally namespaced (`tiger:cat="PP"`). Nodes are referenced as `#1`, `#2` and linked by overlap, adjacency and hierarchy operators such as `#1 > #2` for dominance. Some operators accept edge labels. — [ANNIS AQL user guide](https://korpling.github.io/ANNIS/4/user-guide/aql/index.html)

**MACULA (Clear Bible)**
- Annotation layers: syntax trees, semantic roles (Agent, Verb, Patient), participant referents, Louw-Nida-based word senses (UBS MARBLE), English glosses, and morphology from Nestle1904 by Ulrik Sandborg-Petersen. — [macula-greek README](https://github.com/Clear-Bible/macula-greek)
- MACULA Greek TSV columns are: `xml:id ref role class type english mandarin gloss text after lemma normalized strong morph person number gender case tense voice mood degree domain ln frame subjref referent`. Ids are token-level, for example `n40001001001` = book 40, chapter 001, verse 001, word 001. `ref` looks like `MAT 1:1!1`. `domain` and `ln` can hold several values (e.g. `010002 033003` / `10.24 33.19`). — [macula-greek SBLGNT TSV](https://github.com/Clear-Bible/macula-greek/tree/main/SBLGNT/tsv) (computed from the downloaded file)
- MACULA Hebrew TSV columns are: `xml:id ref class text transliteration after strongnumberx stronglemma sensenumber greek greekstrong gloss english mandarin stem morph lang lemma pos person gender number state type lexdomain contextualdomain coredomain sdbh extends frame subjref participantref`. The `greek`/`greekstrong` columns give each Hebrew token a corresponding Greek word, which is a direct hook for comparing the Hebrew Bible with the LXX. — [macula-hebrew WLC/tsv (Git LFS)](https://github.com/Clear-Bible/macula-hebrew) (computed)

**STEPBible**
- TAGNT words are tagged with disambiguated Strong's numbers linked to LSJ, morphology (Robinson, based on Tauber), and context-sensitive translations. Each word also records which editions contain it (NA27/28, TR, …) plus positional and meaning variants. So textual-variant facets are possible. — [STEPBible-Data](https://github.com/STEPBible/STEPBible-Data)
- TTESV applies extended Strong's tags to the ESV. That is a Strong's-keyed English↔original link. — same

### Inferences
- Catalogue of query types, each mapped to the data it needs and a suitable index technique:
  1. **Lemma or Strong's occurrence** ("every ἀγαπάω"): token table with a `lemma` column. Index: inverted index lemma → sorted token-id list (or bitmap).
  2. **Form or morphology filter** (aorist imperatives of X): per-field columns such as tense and mood. Index: bitmaps per (field, value), intersected with the lemma list.
  3. **Proximity** (BEFORE / AFTER / WITHIN n words): positional token ids. Index: merge-join of two sorted position lists with a distance check.
  4. **Within a unit** (same verse, clause or sentence): span tables (start, end) for each unit type. Index: per token, the id of its containing unit for each level (a "parent column"), or an interval join.
  5. **Agreement** (AGREE / `.f.`): columns compared across tuple members at join time. No special index is needed.
  6. **Syntax and clause roles** (subject = Jesus, object = Peter): tree or edge table (parent, role). Index: parent-pointer columns plus a preorder/postorder interval encoding for dominance tests.
  7. **Semantic domain** (Louw-Nida, SDBG/SDBH): a domain column, possibly with several values per token. Index: inverted index domain → token list, with hierarchical prefix ranges.
  8. **Referent** (all mentions of Peter, including pronouns): referent column linking to an entity id. Index: inverted index entity → tokens.
  9. **English rendering of an original-language word, per translation**: alignment table (source token id ↔ target token id or span, per translation). Index: forward source→target map and a reverse map.
  10. **Faceted counts** (hits by book, form, rendering): group-by over the hit list using its columns. Implementation: counting over a compact columnar store.
  11. **KWIC concordance**: surface text plus the `after` (punctuation/spacing) column, to reconstruct context windows.
  12. **Cross-references, OT quotations in the NT, parallel passages**: verse-range ↔ verse-range edge tables with a type and weight. Index: adjacency list keyed by verse.
- The driving question decomposes as follows:
  1. Look up lemma posting lists for {ἀγάπη, ἀγαπάω, φιλέω}.
  2. Join them with the per-translation alignment table.
  3. Group by translation × English rendering and by book.

  In SBLGNT/MACULA these lists are small: ἀγαπάω 143 tokens, ἀγάπη 116, φιλέω 25 (computed from the MACULA SBLGNT TSV). Any reasonable layout answers this in milliseconds. The hard part is getting the alignment data, not the index.

### Gaps
- Primary Logos help pages (support.logos.com) returned HTTP 403, so the full Logos operator list could not be verified directly. Unverified operators: NEAR, INTERSECTS, ANDEQUALS, `sense:`, `ln:`, `person:`.
- Not researched in this pass:
  - Bible Word Study and Bible Sense Lexicon internals
  - Accordance's hits graph
  - BibleWorks (discontinued in 2018, per general knowledge; not verified here)
  - STEPBible's UI query features
- Specific open datasets and tools for detecting intertextuality, OT quotations in the NT, and synoptic parallels were not covered.
- ANNIS operator details (`.`, `.*`, `_=_`, `_i_`, `->`, `^`) and the graphANNIS backend were not fetched, because the intro page lacks them.

## Q2. What are the underlying data models?

### Takeaway
The dominant model is a dense token table with one column per feature, plus span or containment structures for higher units and typed edges for syntax, reference and alignment. Text-Fabric's model (integer slots, integer nodes above them, `oslots` containment, features as node→value maps, edges as node-pair maps) is the clearest open reference design. CWB uses the same idea for flat token streams: positional attributes plus structural ranges.

### Cited Findings
- **Text-Fabric slots and nodes:** Slots (usually words) are nodes 1..maxSlot, in order. Nodes above maxSlot represent phrases, clauses, sentences, verses, chapters and books, and each links to zero or more slots. — [TF data model](https://annotation.github.io/text-fabric/tf/about/datamodel.html)
- **Text-Fabric structure:** `otype` gives each node its type. `oslots` is a valueless edge from each non-slot node to every slot it contains. Containment and a canonical node order are computed from `oslots`. — same
- **Text-Fabric features and storage:**
  - Node features map integer → value. Edge features map (int, int) → value.
  - `otext` names the section levels (book, chapter, verse) and text-format templates.
  - "Warp" means otype + oslots + otext. All other features are "weft". A dataset with only weft is a "module", which is how third-party annotations layer on.
  - Data is stored as plain `.tf` text files, compiled after first load into compressed binary `.tfx` caches.
  - Source: same page
- **TF search** is a constraint-satisfaction search over the node/feature/edge graph, with quantifiers evaluated as separate searches and combined with set algebra. — [TF search usage](https://annotation.github.io/text-fabric/tf/about/searchusage.html)
- **MACULA formats:** `nodes` (nested Node XML for recursive and NLP processing), `lowfat` (XML suited to query systems and display), and `TSV` (word-level, no tree). The Greek repo also has `tei`. — [macula-greek](https://github.com/Clear-Bible/macula-greek); [macula-hebrew](https://github.com/Clear-Bible/macula-hebrew)
- **N1904 Text-Fabric** is built from Clear-Bible's MACULA LowFat trees. So the MACULA trees can be converted into a TF-style node/feature graph. — [ETCBC/nestle1904](https://github.com/ETCBC/nestle1904)
- **OSHB** marks each word in OSIS XML with `lemma` (augmented Strong's, e.g. `c/m/6529`, where prefixes are split by `/`), `morph` (e.g. `HC/R/Ncmsc`) and an immutable `id` (e.g. `018xz`). — [openscriptures/morphhb](https://github.com/openscriptures/morphhb)
- **MorphGNT** is a flat whitespace table: `book/chapter/verse POS parsing-code text word normalized lemma`. For example, `010101 N- ----NSF- Βίβλος Βίβλος βίβλος βίβλος` (book 01 = Matthew, chapter 01, verse 01). — [morphgnt/sblgnt](https://github.com/morphgnt/sblgnt) (row sample computed from 61-Mt-morphgnt.txt)
- **STEPBible data** is UTF-8 TSV. Some datasets use multi-line records separated by `$`. Strong's numbers (extended, but compatible with original Strong's) are "the primary linking key". References use the form `Gen.1.10-12`. — [STEPBible-Data](https://github.com/STEPBible/STEPBible-Data)
- **Clear alignments** (Clear-Bible/Alignments) follow the Scripture Burrito Working Group's "Scripture Alignments" flavor, which is still a "working proposal". The repo ships a `bible-alignments` Python package. — [Clear-Bible/Alignments](https://github.com/Clear-Bible/Alignments)

### Inferences
- **Two ways to key alignments:**
  - Strong's-keyed (STEPBible TTESV, classic KJV+Strong's): easy to build, but it loses the exact source token, so it cannot tell apart two ἀγαπάω tokens in the same verse.
  - Token-keyed (MACULA-style ids such as `n40001001001` ↔ target-token ids): exact, and needed for "English renderings per occurrence".
  - OpenLogos should store token-keyed alignments and derive Strong's-level views from them.
- MACULA's `xml:id` (BBCCCVVVWWW) is a natural stable global token key for the NT. OSHB's immutable word ids play the same role for the Hebrew Bible. A crosswalk to BHSA slot numbers would be needed to use ETCBC data.
- A Rust engine could mirror TF: a slot-indexed columnar token store, a per-level `oslots` range table (most Bible units cover contiguous slots, so (start, end) pairs suffice), and CSR-encoded edge tables. This combines well with an inverted-index or bitmap layer for fast predicate lookup.

### Gaps
- The CWB on-disk index format (lexicon, reversed index, compression) could not be verified. The PDFs (CQP Manual, Evert2008 InsideCWB) either returned binary that could not be extracted (no pdftotext available) or HTTP 403. The Encoding Manual appendix on "Magic" compression is the likely source. — [CWB Encoding Tutorial](https://cwb.sourceforge.io/files/CWB_Encoding_Tutorial.pdf)
- The exact Clear alignment file schema (JSON fields, target-token id scheme) was not visible on the repo landing page.

## Q3. Which open datasets supply each layer, and under what licenses?

### Takeaway
| Layer | Dataset | License |
|---|---|---|
| Greek NT text | SBLGNT | CC BY 4.0 since Dec 2022 |
| Hebrew Bible text | WLC | Public domain |
| Hebrew morphology and lemmas | OSHB | CC BY 4.0 |
| Greek morphology and lemmas | MorphGNT | CC BY-SA 3.0 |
| Greek syntax trees, roles, referents | MACULA Greek | CC BY 4.0 |
| Hebrew syntax trees, roles, referents | MACULA Hebrew | CC BY 4.0 |
| Tagged texts and lexicons | STEPBible (TAGNT, TAHOT, TBESG/H, TIPNR, TVTMS) | CC BY 4.0 |
| Alignments | Clear alignments | CC BY 4.0 |
| Cross-references | OpenBible | CC BY |
| Cross-references | TSK | Public domain |
| Semantic-domain dictionaries | UBS SDBG/SDBH | CC BY-SA 4.0 |

Two traps:
1. MACULA's Louw-Nida `ln`/`domain` columns are MARBLE data "used with permission", with no open license.
2. ETCBC BHSA is CC BY-NC 4.0, which is incompatible with commercial redistribution.

### Cited Findings
- **SBLGNT:** CC BY 4.0. Copyright 2010 SBL and Logos. v1.1 (2022-12-19) "Update public version and license on github". v1.2 (2023-07-10) added John 7:53–8:11. — [LogosBible/SBLGNT](https://github.com/LogosBible/SBLGNT)
  - The MorphGNT README still says the SBLGNT text is "subject to the SBLGNT EULA". This is outdated relative to the upstream CC BY 4.0 relicensing. — [morphgnt/sblgnt](https://github.com/morphgnt/sblgnt); contradicted by [LogosBible/SBLGNT](https://github.com/LogosBible/SBLGNT)
- **MorphGNT:** parsing and lemmatization under CC BY-SA 3.0. Version 6.12 (Tauber 2017). The POS and parsing codes, inherited from CCAT, are slated for deprecation. — [morphgnt/sblgnt](https://github.com/morphgnt/sblgnt)
- **OSHB:** lemma and morphology under CC BY 4.0. WLC text is public domain. Lemmas cover "most words", and morphology is still "in the process" of being added (incomplete per README). — [openscriptures/morphhb](https://github.com/openscriptures/morphhb)
- **MACULA Greek licensing, per its LICENSE.md:**
  - Clear datasets (syntax trees in Node and Lowfat, morphology attributes, semantic frames, adjunct types, participant referents): CC BY 4.0, attributed as "MACULA Greek Linguistic Datasets".
  - `@gloss` comes from the Berean Interlinear. The Berean Bible became public domain on 2023-04-30.
  - Cherith English/Chinese glosses: CC BY 4.0.
  - `@ln` and `@domain` come from UBS MARBLE, "used with permission", with no open license stated.
  - Source: [macula-greek LICENSE.md](https://raw.githubusercontent.com/Clear-Bible/macula-greek/main/LICENSE.md)
- **MACULA Hebrew:**
  - WLC: public domain (Groves Center).
  - Westminster Hebrew Syntax: CC BY 4.0.
  - Cherith Glosses: CC BY 4.0.
  - Morphology: from OSHB.
  - Word senses: UBS MARBLE, based on SDBH.
  - The TSV is stored in Git LFS (about 84 MB).
  - Source: [macula-hebrew](https://github.com/Clear-Bible/macula-hebrew) (size computed from the LFS pointer)
- **ETCBC BHSA:**
  - License: CC BY-NC 4.0. Commercial use needs consent from the German Bible Society. The repo sidebar also shows MIT, which most likely covers the code.
  - Versions go back to 2011 and are stored side by side.
  - Status: "Active".
  - Source: [ETCBC/bhsa](https://github.com/ETCBC/bhsa)
- **N1904 Text-Fabric:** ETCBC/nestle1904 was archived on 2025-04-17 and moved to CenterBLC/N1904. It is MIT-licensed and built from MACULA LowFat. — [ETCBC/nestle1904](https://github.com/ETCBC/nestle1904); successor [CenterBLC/N1904](https://github.com/CenterBLC/N1904)
- **STEPBible:** all data is CC BY 4.0 (credit "STEP Bible" with a link). Datasets:
  - TAHOT: Leningrad-based text. Morphology converted from ETCBC to OSHB format, with Ketiv/Qere handling and LXX additions from the BHS/BHK apparatus.
  - TAGNT: NA27/28, TR and other editions, with variants.
  - TBESH: abridged BDB.
  - TBESG: brief definitions, based on Abbott-Smith.
  - TFLSJ: full LSJ entries up to G5624.
  - TIPNR: proper names linked to forms and references, with family relationships and OpenBible geolocation. Descriptions were "created by Claude 3 AI".
  - TVTMS: versification mapping.
  - TEHMC/TEGMC: morphology code descriptions.
  - In progress: TAGOT (Greek OT), TFBDB, TOTMM/TNTMM (manuscripts and meanings), TBCWG.
  - Source: [STEPBible-Data](https://github.com/STEPBible/STEPBible-Data)
- **Clear-Bible/Alignments:** data under CC BY 4.0 (copyright held by the contributing parties). Code is MIT, © Biblica 2024. — [Clear-Bible/Alignments](https://github.com/Clear-Bible/Alignments)
- **OpenBible cross-references:** about 340,000 references, mostly from TSK (public domain), with votes seeded from OpenBible's Topical Bible and Twitter search. CC BY license, available as a 2 MB zip. — [OpenBible.info cross-references](https://www.openbible.info/labs/cross-references/)
- **UBS dictionaries:**
  - UBS releases its dictionaries "in [their] entirety" under CC BY-SA 4.0: Semantic Dictionary of Biblical Greek/SDGNT, UBS Dictionary of NT Greek, and the Semantic Dictionary of Biblical Hebrew.
  - They are available as XML/JSON, apparently in the ubsicap GitHub org (`dictionaries/greek`).
  - Sources: [UBS Dictionary of the Greek NT](https://translation.bible/tools-resources/ubs-dictionary-of-the-greek-new-testament/); [UBS Open Access resources](https://translation.bible/open-access-resources/); [UBS Dictionary of Biblical Hebrew](https://translation.bible/tools-resources/ubs-dictionary-of-biblical-hebrew/); [Open Bible Data UBS details](https://freely-given.org/OBD/UBS/details.htm)
  - A Hugging Face SDBH mirror sits behind a contact-sharing gate. — [NuBerea/sdbh](https://huggingface.co/datasets/NuBerea/sdbh)

### Inferences
- **For an open-source app that may ever be sold or redistributed commercially:**
  - Avoid BHSA (NC) as a bundled default. Use OSHB/WLC plus MACULA Hebrew (CC BY), and offer BHSA as an optional user-downloaded module.
  - Treat MACULA's `ln`/`domain` columns as non-redistributable unless UBS gives permission. Use the CC BY-SA SDBG/SDBH dictionaries as the open substitute for Louw-Nida-style domains, and keep the share-alike data in a separate module so its license stays isolated.
- **MorphGNT is CC BY-SA**, so derived morphology layers must stay share-alike. MACULA's CC BY morphology (from Nestle1904) is the more permissive choice.
- **Louw-Nida itself** (the UBS 1988/89 lexicon) is copyrighted. Only the UBS-released SDBG material is openly licensed. This inference rests on MACULA's "used with permission" wording; the Louw-Nida print copyright terms themselves were not verified.

### Gaps
- Not verified:
  - CenterBLC/N1904 current license and features
  - The exact LICENSE file of the ubsicap dictionaries repo
  - Which translations Clear-Bible/Alignments covers
  - The license of MACULA's Nestle1904 morphology upstream (biblicalhumanities/Nestle1904)
- LXX datasets (e.g. CATSS/CCAT LXX license restrictions, Rahlfs-based open texts, STEPBible's in-progress TAGOT) were not researched.
- TSK's public-domain status is asserted by OpenBible but was not independently verified.

## Q4. Corpus scale numbers

### Takeaway
Greek NT (SBLGNT/MACULA): about 137.7k tokens. Hebrew Bible (MACULA Hebrew, segmented): about 475.9k rows. All biblical corpora are tiny by information-retrieval standards: under 1M tokens for each original-language text and under 1M for each English translation.

### Cited Findings
- MACULA Greek SBLGNT TSV: 137,741 lines including the header, so **137,740 word tokens**. There are 5,518 distinct lemmas. Matthew has 18,329 tokens, Mark 11,286 and Luke 19,446. — [macula-greek SBLGNT TSV](https://github.com/Clear-Bible/macula-greek/tree/main/SBLGNT/tsv) (computed)
- MACULA Hebrew WLC TSV: 475,911 lines including the header, so **475,910 rows**. The rows are morpheme or segment level: prefixes such as conjunctions, articles and prepositions are split. — [macula-hebrew](https://github.com/Clear-Bible/macula-hebrew) (computed)
- Example lemma frequencies in SBLGNT: ἀγαπάω 143, ἀγάπη 116, φιλέω 25. — (computed, same TSV)

### Inferences
- A full Greek NT token table with about 25 small integer or dictionary-encoded columns is a few MB uncompressed. The whole multi-layer corpus, including several English translations and alignments, fits comfortably in memory on phones and in WASM.

### Gaps
- Not verified from primary sources:
  - BHSA's slot count. Commonly cited as about 426k word slots; not fetched.
  - The orthographic Hebrew Bible word count (often cited as about 305k).
  - LXX token counts (often cited as about 600k+).
  - Typical English translation size (about 780k for the KJV is widely cited; no primary source fetched).

## Q5. Performance and indexing strategies: is a custom columnar plus positional-bitmap engine typical?

### Takeaway
The tools I could document do use custom, in-memory, column-oriented designs, not general-purpose search engines. Text-Fabric keeps per-feature arrays in memory, compiled to binary caches, and answers queries by constraint search plus set algebra. CWB indexes positional attributes for very large corpora. Given corpus sizes under 1M tokens, a custom columnar token store with per-value posting lists or bitmaps (e.g. Roaring) and span/edge tables is a well-precedented and appropriate design for an offline Rust/WASM core.

### Cited Findings
- Text-Fabric converts features to binary form after the first read so later loads are fast, precomputes warp-derived data such as containment and order, and targets efficient interactive work (Jupyter). — [TF data model](https://annotation.github.io/text-fabric/tf/about/datamodel.html)
- TF quantifiers run as separate searches whose node sets are combined by difference, intersection or union, then fed back as custom sets. In effect this is set-algebra query processing. — [TF search usage](https://annotation.github.io/text-fabric/tf/about/searchusage.html)
- CWB indexes and queries "very large corpora with multiple layers of word-level annotation". — [Evert & Hardie 2011](https://eprints.lancs.ac.uk/id/eprint/62721/)

### Inferences
- **Recommended pattern, drawn from the precedents above:**
  1. A dense token table, with columns dictionary-encoded per feature.
  2. Per-(feature, value) posting lists or Roaring bitmaps over token ids, for lemma, morph-field, domain and referent predicates.
  3. Per-level parent-id columns (verse id, clause id, sentence id) for "within unit" constraints.
  4. A tree stored as parent array plus pre/post-order intervals for dominance.
  5. CSR edge tables for typed relations: cross-references, alignment, referent.
  6. A join engine for sequences and proximity (merge of sorted lists with a distance check) and for AGREE (filter during the join).
- At this scale, even brute-force column scans (about 138k rows) finish in under 10 ms. Indexes matter mostly for interactive faceting across many translations and for syntax queries with several nodes.

### Gaps
- No primary documentation was found on the internal index architectures of Logos or Accordance, which are proprietary.
- CWB's internal index layout was not verified (see Q2 gaps).
- No published benchmarks were found comparing Bible-specific engines.
