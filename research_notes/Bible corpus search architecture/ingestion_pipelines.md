# Data Ingestion Pipelines for Bible and Ancient-Text Corpora (OpenLogos)

Scope: how raw sources become normalized, token-level, versification-mapped, aligned data for an offline-first app with a Rust core and a build pipeline that produces signed, content-addressed packages. Research done on 2026-10-27. These notes use about 25 tool calls (search and fetch). Where a fact comes from my training knowledge and I did not re-verify it this session, it is labeled "unverified" and placed under Inferences or Gaps.

---

## 1. Source formats and their parsers/tooling (USFM 3.x, USX, USJ, OSIS, TEI, Zefania, SWORD, Paratext)

### Takeaway
USFM/USX/USJ is the live, actively maintained family. There are two credible MIT-licensed Python toolchains, both releasing in autumn 2026: **usfmtc**, the USFM Technical Committee's reference implementation, and **usfm-grammar**, Bridgeconn's tree-sitter parser. There is also a young MIT Rust crate, **usfm3** (jcuenod). That crate is the most natural fit for a Rust-core build tool, but it should be checked against usfmtc output. Proskomma is still labeled experimental. Treat it as an optional JS ecosystem, not a pipeline dependency.

### Cited Findings
- **usfmtc** 0.4.10 was released on 2026-10-22. It is authored by the "USFM Technical Committee" (contact fonts@sil.org), the PyPI maintainer is `mhosken`, and it is MIT-licensed on Python 3.8 or later. It reads and writes USFM, USX and USJ, and it "is designed to be a reference implementation of the standard." It also handles scripture reference parsing (ranges, lists, validity), remaps versification using built-in schemes or custom `.vrs` files, and extracts subsets by reference. CLI tools: `usfmconv`, `usfmreversify`, `urnc2rng`. Versions 0.4.8 to 0.4.10 shipped between Aug and Oct 2026, with 30+ releases since 0.2 in Nov 2024. No PyPI provenance attestations. — [PyPI usfmtc](https://pypi.org/project/usfmtc/)
- **usfm-grammar** 3.2.1 was released on 2026-09-30 under the MIT license. The maintainer is Bridgeconn (Kavitha Raju, "BCS Team"). Outputs: USX (`to_usx()`), USJ (`to_usj()`), BibleNLP format (`to_biblenlp_format()`), list/table, and syntax tree, with Markdown in the CLI. Its USX example uses version "3.1.2", and it links the USFM 3.1.2 extension docs. — [PyPI usfm-grammar](https://pypi.org/project/usfm-grammar/)
- usfm-grammar is built on tree-sitter-usfm3 and needs Python 3.10 plus a C compiler (MSVC 14+ on Windows). Stable 3.0.0 shipped 2024-12-13 after alpha and beta releases going back to 2022. — [piwheels usfm-grammar](https://www.piwheels.org/project/usfm-grammar); [PyPI 3.0.0b4](https://pypi.org/project/usfm-grammar/3.0.0b4)
- **usfm3 (Rust)** 0.2.0 was released on 2026-04-09 under the MIT license. Its sole crates.io owner is jcuenod, and the repo is github.com/jcuenod/usfm3. It describes itself as an "error-tolerant USFM 3.x parser with USJ, USX, and USFM output" with a lazy parse handle, an eager AST and CST-first lowering. Earlier 0.1.x docs mention verse-reference maps. — [docs.rs usfm3 0.2.0](https://docs.rs/crate/usfm3/0.2.0); [docs.rs usfm3 0.1.8](https://docs.rs/crate/usfm3/0.1.8)
- **Proskomma-core** is "A Javascript Implementation of the Proskomma Scripture Processing Model". The repo is marked **[EXPERIMENTAL]** and MIT-licensed, with no published releases, 2 stars and no archive banner. It runs queries over USX/USFM input. — [GitHub Proskomma/proskomma-core](https://github.com/Proskomma/proskomma-core)
- unfoldingWord USFM carries word-alignment milestones (`\z…` user-defined milestones and `x-` attributes on `\w`). Their forum notes that it is "not perfectly straight-forward" to extract plain translated text without dropping the alignment data. — [Door43 forum: whitespace in uW USFM](https://forum.door43.org/t/whitespace-in-unfoldingword-usfm-files/504)
- The `word-aligner` npm package has a `usfm-js` peer dependency that needs a legacy peer-deps install. This is a sign that the uW JS tooling is aging. — [word-aligner CLAUDE.md on jsDelivr](https://cdn.jsdelivr.net/npm/word-aligner@1.1.1/CLAUDE.md)
- **SWORD/OSIS:** STEPBible-Data publishes SWORD Bible modules (OSIS) alongside its TSV datasets. — [STEPBible-Data](https://github.com/STEPBible/STEPBible-Data)
- **Scripture Burrito** is the packaging metadata standard. Documentation lives at docs.burrito.bible and the source is github.com/bible-technology/scripture-burrito. unfoldingWord has migration guides from Resource Container (RC) to Scripture Burrito, which list flavors such as `parascripturalWordAlignment` and `peripheralWordAlignment`. — [Scripture Burrito wiki](https://github.com/bible-technology/scripture-burrito/wiki); [uW RC→SB migration guide](https://docs.page/unfoldingWord/uW-Tools-Collab/5-migration/rc-to-sb/general-conversion-guide)

### Inferences
- Recommended format: normalize every USFM/USX input to **USJ** (JSON), the newest form in the USFM 3.1 family. usfmtc, usfm-grammar and usfm3 all emit it, and it is easy to consume from Rust with serde.
- Recommended setup: use the Rust `usfm3` crate inside the build tool, and run **usfmtc as a conformance oracle** in CI by diffing the USJ both produce for each source book. usfmtc is the committee's reference implementation, while usfm3 is a single-maintainer crate (bus-factor risk).
- Do not build on Proskomma. It is experimental with no releases.
- OSIS (SWORD, morphhb, STEPBible modules) and TEI (MACULA, EpiDoc) are best handled with a generic streaming XML reader (e.g. `quick-xml` in Rust) plus one small adapter per corpus. A generic OSIS or TEI parser would not justify its cost, because each corpus uses its own subset.
- Zefania XML and SWORD binary modules should be treated as **legacy, last-resort** inputs. They lose structure (notes, morphology granularity) compared with USFM/USX/OSIS originals. *Unverified: I did not re-check Zefania tooling status this session.*

### Gaps
- I did not fetch the official USFM/USX spec pages at ubsicap.github.io this session. Find the current USFM release number (3.1.x?) and USJ status there.
- No current info on Zefania XML tooling, the JSword/SWORD maintenance status, or the Paratext project-folder format (Settings.xml + per-book `.SFM`). *Unverified: Paratext stores books as USFM and versification as `.vrs`.*
- EpiDoc/TEI parsers were not researched.
- I could not confirm whether Proskomma has a Rust port (e.g. "proskomma-rs"). None was found.

---

## 2. Original-language datasets ingestion and token ID schemes

### Takeaway
Every major dataset ships a word-level table (TSV or OSIS `<w>`) with lemma, morphology and some kind of word ID. The ID schemes differ and are tied to each dataset's own versification and edition. The robust pattern is to **keep each source's native ID as an immutable external key** and mint an OpenLogos-internal ID per (edition, token). Cross-edition identity should go through explicit mapping tables, not through reused IDs.

### Cited Findings
- **OSHB morphhb** is OSIS XML. Each word is `<w>` with `lemma` (augmented Strong's numbers plus prefix codes, e.g. `c/m/6529`), `morph` (OSHM codes, e.g. `HC/R/Ncmsc`), and `id` (a "permanent unique identifier" whose first two digits are the KJV book number). The base text is the WLC (public domain). Lemma and morphology are CC BY 4.0 (credit the Open Scriptures Hebrew Bible Project). The README is dated 2017-01-27, notes that morphology was still in progress, and the repo has 36 open issues. Text follows Hebrew versification, and the Perl JSON script has `--remapVerses` to map to English numbering. — [openscriptures/morphhb](https://github.com/openscriptures/morphhb)
- **MorphGNT SBLGNT** has one `-morphgnt.txt` file per book. Columns: book/chapter/verse, POS, parsing code, text (with punctuation), word (punctuation stripped), normalized word, lemma. The POS and parsing codes come from CCAT tagging and "will be deprecated in the next major release." The SBLGNT text is under the SBLGNT EULA, and the morphology and lemmas are CC BY-SA. Version 6.12. — [morphgnt/sblgnt](https://github.com/morphgnt/sblgnt)
- **MACULA Greek** ships four variants: `tei` (formatted text), `nodes` (nested Node elements for NLP), `lowfat` (for query systems and displays), and `TSV` (word-level, no tree). Sources are Nestle1904 (Diego Santos transcription, Ulrik Sandborg-Petersen morphology, Jonathan Robie markup) and SBLGNT (from Logos, as of a 2023-07-10 commit including the pericope adulterae). Enrichments: Clear syntax trees, BSB English glosses, UBS MARBLE word senses (Louw-Nida), semantic roles and participant referents. Licenses are per source, in LICENSE.md. — [Clear-Bible/macula-greek README](https://github.com/Clear-Bible/macula-greek/blob/main/README.md)
- **MACULA Hebrew** ships `WLC/nodes`, `WLC/lowfat` and `WLC/tsv`. The base is WLC (public domain, tanach.us), with morphology from OSHB morphhb. The Groves Center Westminster Hebrew Syntax is CC BY 4.0, as are the Cherith glosses. — [Clear-Bible/macula-hebrew](https://github.com/Clear-Bible/macula-hebrew)
- **STEPBible-Data** files are UTF-8 TSV. Records are mostly one line each. Some datasets use multi-line records separated by lines starting with `$`, with a header line and tab-prefixed subrecords. Datasets: TAHOT (Leningrad-based Hebrew OT), TAGNT (Greek NT with variants and morphology), TBESH/TBESG (brief lexicons linked to Extended Strong's), TFLSJ, TIPNR (proper names with geolocation), TVTMS, TEHMC/TEGMC (expanded morphology codes) and TTESV. TAGOT, TFBDB, TOTMM, TNTMM and TBCWG are listed as "coming soon". The license is CC BY 4.0: credit "STEP Bible" with a link, record and share changes, and keep mirrors up to date. The README also states normalization rules for Hebrew glyph order and the allowed Greek glyphs. — [STEPBible-Data](https://github.com/STEPBible/STEPBible-Data)
- A Hugging Face derivative describes TAGNT as "amalgamating multiple critical and traditional editions" and TAHOT as covering the Leningrad Codex with Qere/Ketiv. — [NuBerea/stepbible (gated)](https://huggingface.co/datasets/NuBerea/stepbible)
- Tyndale House's official involvement in STEPBible data development ended when STEPBible.org became an independent charity. — [STEPBible-Data releases](https://github.com/STEPBible/STEPBible-Data/releases)
- **ETCBC BHSA** is a Text-Fabric representation of the Hebrew Bible database. Versions go back to 2011 and are immutable ("new versions will be stored next to the old versions"). Ingest it with `pip install text-fabric`, then `use('etcbc/bhsa')`, which downloads to `~/text-fabric-data`. The license is **CC BY-NC 4.0**, with commercial use requiring consent from the German Bible Society and attribution via DOI 10.17026/dans-z6y-skyh. The GitHub sidebar separately shows an MIT link, presumably for code. — [ETCBC/bhsa](https://github.com/ETCBC/bhsa)

### Inferences
- **License-gating matters for package builds.** BHSA (NC), the SBLGNT text (EULA) and MorphGNT (BY-SA, share-alike) need per-package rights flags. CC BY sources (morphhb, STEPBible, MACULA parts, Clear alignments) are safe to redistribute with attribution. For a redistributable app, the Nestle1904/MACULA Greek and WLC/OSHB/MACULA Hebrew stack is the cleanest base.
- **Stable token IDs.** The pattern is: (a) keep `source_id` exactly as given (morphhb `id`, MACULA `xml:id`, TAGNT/TAHOT ref+word index, BHSA slot number + version); (b) mint an internal ID `edition:BBCCCVVVWWW[s]`, where s is the subword or morpheme index (Hebrew prefixes/suffixes need it); (c) when a new edition release arrives, compute a token-level diff (by surface form + lemma within a verse) and emit an `id_lineage` table (old→new, op = same/split/merge/insert/delete). Never renumber silently. BHSA's immutable-version policy is a good model.
- *Unverified (training knowledge):* MACULA word IDs look like `n40001001001` (NT, book-chapter-verse-word) and `o010010010011` (OT, with a trailing morpheme digit). The `ref` attribute uses forms like `MAT 1:1!1`. Confirm in the macula `doc/` directories.
- Hebrew tokenization differs by source. OSHB segments prefixes with `/` inside one `<w>`, while BHSA and MACULA split them into separate slots or morphemes. The internal model should hold both a "word" layer and a "morpheme" layer.

### Gaps
- I did not obtain the exact TAHOT/TAGNT column layouts. They are in each file's header, and the HF derivative's spec is gated.
- MACULA TSV column names and ID format are not confirmed (README does not list them).
- Nestle1904 standalone repos (biblicalhumanities/Nestle1904) and their current maintenance were not checked.
- The current OSHB morphology completeness status (2026) is unknown. The README is from 2017.

---

## 3. Versification (Copenhagen Alliance, Paratext .vrs, STEPBible TVTMS)

### Takeaway
There are three complementary layers. Paratext `.vrs` files (org/eng/lxx/vul/rso/rsc) are the de facto definition of schemes. The Copenhagen Alliance JSON format (derived from VRS) adds machine-readable mappings and **sniffing rules** that detect which scheme a text uses. TVTMS is the most detailed scholarly mapping table and covers splits, merges, partial verses and LXX sub-verses. Recommended setup: store every text in its native versification, map to one canonical pivot (`org`, the Hebrew/Greek original-language scheme) using per-scheme mapping tables, and use TVTMS to resolve edge cases.

### Cited Findings
- The Copenhagen Alliance Versification Working Group includes UBS, Tyndale House, YouVersion, SIL International and Global Bible Initiative. The repo ("Versification mappings and versification sniffing") now resolves to **nida-institute/versification**. — [Copenhagen-Alliance/versification-specification](https://github.com/Copenhagen-Alliance/versification-specification)
- The mapping JSON format was created by Mark Howe (UBS), "based on the VRS format used in Paratext". A mapping "identifies the versification of a specific Scripture text by describing how it differs from a base text." Sniffing rules apply tests to a text to identify its versification. The original rules are by David Instone-Brewer, and the JSON rule format is by Chris Vaughn and Jonathan Robie. — [same repo](https://github.com/Copenhagen-Alliance/versification-specification)
- The `versification-mappings/` directory holds `json-schema/`, `standard-mappings/` (including the base `org.json` and sample files), `sample-custom-mappings/`, and `VersificationMappings.ipynb`, which it calls "the best documentation we have." — [versification-mappings dir](https://github.com/Copenhagen-Alliance/versification-specification/tree/master/versification-mappings)
- According to the GitHub org page, the repo was last updated around 2025-08-04, and YouVersion maintains a fork. — [Copenhagen-Alliance org](https://github.com/Copenhagen-Alliance); [YouVersion GitHub](https://github.com/youversion)
- A Feb 2024 sword-devel post criticizes the format as not modular. Each versification must be fully defined in one file, and mappings are always relative to a single base versification. — [sword-devel msg36332](https://www.mail-archive.com/sword-devel@crosswire.org/msg36332.html)
- **TVTMS** compares OT traditions (Hebrew, Latin, Greek) and early NT versification against an English standard defined by the **NRSV**. It uses per-section rules (e.g. "if this chapter has 29 verses, it is using Greek versification"), which allows reversification to English, Hebrew, Greek or Latin schemes. — [STEPBible-Data](https://github.com/STEPBible/STEPBible-Data)
- A derived registry (bcv-commons/versification on HF) uses the TVTMS "Expanded Version" layout: `SourceType, SourceRef, StandardRef, Action`. It treats KJV as standard, which conflicts with STEPBible's own NRSV framing. It also lists as out of scope TVTMS ranges, LXX sub-verses, concatenations (the `;`, `-` and `!` notations), long verses, and merges or splits. That shows those cases are where the complexity lies. — [bcv-commons/versification](https://huggingface.co/datasets/bcv-commons/versification)
- `bibletags-versification` (JS) aligns verses across versions using a few "versification models" (traditions). — [bibletags-versification](https://github.com/educational-resources-and-services/bibletags-versification)
- usfmtc can remap versification with built-in schemes or custom `.vrs` files (`usfmreversify`). — [PyPI usfmtc](https://pypi.org/project/usfmtc/)
- In the Door43 BSB alignment fork, versification differences between UGNT and BSB could not be handled, so those verses were left unaligned. — [Worldview_en_bsb commit](https://content.bibletranslationtools.org/DCS-Mirror/Worldview_en_bsb/commit/99f4f239c84ebc305e87f9dcc58fd8224ded7a2e)
- The bcp parser supports versification systems `default, ceb, csb, kjv, nab, nlt, nrsv, nrsvue, vulgate` and custom ones via `add_translations()`. — [Bible-Passage-Reference-Parser](https://github.com/openbibleinfo/Bible-Passage-Reference-Parser)

### Inferences
- *Unverified (training knowledge):* a Paratext `.vrs` file lists the max verse per chapter (`GEN 1:31 2:25 …`), plus mapping lines `GEN 31:55 = GEN 32:1`, range mappings `PSA 51:1-2 = PSA 51:3-4`, excluded verses prefixed `-`, and segmented/partial verses prefixed `*` (e.g. `*ACT 19:40`). The Copenhagen JSON mirrors these as keys like `maxVerses`, `mappedVerses`, `excludedVerses`, `partialVerses`. Confirm from `json-schema/`.
- **Data model:** a `verse_map(scheme_from, ref_from, scheme_to, ref_to, kind)` table, where kind ∈ {equal, split, merge, partial_a/b, absent, renumber}, compiled from `.vrs` plus Copenhagen JSON, with TVTMS-derived overrides. Pivot through `org` so N schemes need N maps, not N². Partial verses need sub-verse anchors (letters a/b or token offsets). The token-level layer is what makes partial mapping precise: original-language tokens carry `org` refs, and a translation's tokens get mapped through alignment.
- Run Copenhagen-style **sniffing** at ingest to auto-detect or validate each translation's declared scheme. That gives a QA gate.
- Be careful with the standard base. TVTMS uses NRSV-as-English-standard, while Paratext/Copenhagen use `org` as base and `eng` as the English scheme. Do not mix them without an explicit crosswalk.

### Gaps
- I could not read the actual JSON schema or notebook. Field names are unverified.
- The current nida-institute/versification repo activity in 2026 was not confirmed.
- I did not fetch TVTMS file internals (the condensed vs expanded tabs, test rule syntax).

---

## 4. Alignment data (Clear Bible, unfoldingWord \zaln, statistical/neural aligners)

### Takeaway
Two dominant representations exist. **Inline** (unfoldingWord `\zaln-s … \zaln-e` milestones wrapping `\w` with occurrence counts in USFM) is good for editing and poor for querying. **Standoff** (Clear-Bible/Alignments: source and target token tables plus alignment records keyed by token IDs; CC BY 4.0, moving to a Scripture Burrito "alignment" flavor) is better for an indexed package. OpenLogos should normalize everything to standoff `(source_token_ids[], target_token_ids[], status, origin, confidence)`.

### Cited Findings
- **Clear-Bible/Alignments** holds automatic and manually corrected word alignments. The data is CC BY 4.0 (copyright of contributors), and the code is MIT (© Biblica 2024), installable as the `bible-alignments` Python package. It is test data for a proposed **Scripture Burrito "Scripture Alignments" flavor** ("working proposal; format may change"). — [Clear-Bible/Alignments](https://github.com/Clear-Bible/Alignments)
- `data/` contains folders `arb, asm, ben, eng, fra, hau, hin, por, rus, spa` (ISO 639-3, my inference), plus `languages`, `legacy`, `sources`, `vref` and a `catalog.tsv`. — [Alignments/data](https://github.com/Clear-Bible/Alignments/tree/main/data)
- The first public release used pre-standard formats, which were to be superseded by Burrito-format data in April 2024. — [Alignments releases](https://github.com/Clear-Bible/Alignments/releases)
- Clear also publishes `biblealignlib`. — [depscope biblealignlib](https://mcp.depscope.dev/pkg/pypi/biblealignlib)
- **unfoldingWord/translationCore** earlier documented `\w` attributes such as `x-ugnt-phrase="υἱοῦ" x-source_occurrence="1/2" x-target-occurrence="1/2"` (occurrence n/total) and `strong="G52070:N./....GMS"`. This page reflects an early design. — [translationCore wiki: Alignment Data Structure USFM 3](https://github.com/unfoldingWord/translationCore/wiki/Alignment-Data-Structure:-USFM-3)
- Word-level alignment in uW USFM uses `\z` user-defined milestones and `x-` attributes created in translationCore. — [Door43 forum](https://forum.door43.org/t/whitespace-in-unfoldingword-usfm-files/504)
- A Door43 BSB fork (March 2024) has USFM3 alignment to UGNT (Greek) and UHB (Hebrew). — [Worldview_en_bsb](https://content.bibletranslationtools.org/DCS-Mirror/Worldview_en_bsb/commit/99f4f239c84ebc305e87f9dcc58fd8224ded7a2e)
- HF mirrors such as bcv-commons/compact-alignments and lexeme-alignments repackage alignment data. — [bcv-commons/compact-alignments](https://huggingface.co/datasets/bcv-commons/compact-alignments); [lexeme-alignments](https://huggingface.co/datasets/bcv-commons/lexeme-alignments)

### Inferences
- *Unverified (training knowledge):* current uW USFM uses `\zaln-s |x-strong="G35880" x-lemma="ὁ" x-morph="Gr,EA,,,,NMS," x-occurrence="1" x-occurrences="1" x-content="Ὁ"\*` … `\w The|x-occurrence="1" x-occurrences="1"\w*` … `\zaln-e\*`. Nested zaln-s spans express many-to-one. Converting this to standoff means resolving each (content, occurrence) pair to a source token ID within the verse, so verse-level versification must already agree (hence the unaligned-verse issue above).
- *Unverified (training knowledge):* in the Clear Alignments repo, source and target TSVs carry IDs like `n41004003001` (source) and BBCCCVVVWWW (target). Alignment JSON records have `source[]`, `target[]` and `meta` (id, origin, status such as "created"/"approved"). Verify in `data/eng/*`.
- For aligning OpenLogos's own target texts to MACULA tokens: use **eflomal** (efficient Bayesian aligner) or fast_align as fast statistical baselines. SIL's **Machine** library/SIL.Machine and Clear's tools wrap these. Neural options include SimAlign/awesome-align. *All of this is unverified this session.* Store `origin` (manual|auto:eflomal@ver) and confidence so the UI can distinguish them.
- Alignment packages must be keyed by *edition-specific* token IDs plus a content hash of both tokenizations, so a re-tokenized text invalidates the stale alignment automatically.

### Gaps
- I could not verify the exact Clear Alignments file schema (the PyPI page failed to load and data subfolders were not opened).
- The final Scripture Burrito alignment flavor schema status in 2026 is unknown.
- No source this session checked eflomal, fast_align or SIL Machine maintenance or license status. MIT/Apache/GPL status needs checking before use in a commercial build.
- Whether LEB alignments are openly available was not found.

---

## 5. Commentaries / ancient works and scripture-reference extraction

### Takeaway
CCEL ThML gives commentary references in a semi-structured form (`<scripRef passage=… parsed=…>`), and `parsed` is often present. Use it when available. Otherwise run a reference parser over prose. The openbibleinfo **Bible-Passage-Reference-Parser** (MIT, TypeScript, v4.0.0 July 2026, feature-complete, 50 bundled languages with data for 2,000+) is the reference choice and outputs OSIS refs with versification awareness. It has no Rust port, so either embed it at build time via Node/Deno or port its grammar.

### Cited Findings
- ThML represents citations with `<scripRef>` and commentary on scripture with `<scripCom>`. These elements take `version`, `passage`, `parsed`. Example: `<scripRef version="NIV" passage="Rom. viii. 27,28; x. 8-13" parsed="NIV|Romans|8|27|8|28;NIV|Romans|10|8|10|13">`. `parsed` is optional, and processing software can parse `passage` and add it. — [CCEL ThML scripref](https://www.ccel.org/ThML/scripref.htm)
- Third-party ThML seen in the wild has `parsed="|Matt|20|16|0|0"` alongside an `osisRef` attribute, so the variants are inconsistent. — [Accordance forum](https://forums.accordancebible.com/topic/188-reader-of-other-texts/)
- ThML spec 1.04. — [ThML1.04](https://ccel.org/ThML/ThML1.04.htm); [Wikipedia ThML](https://en.wikipedia.org/wiki/Theological_Markup_Language)
- **Bible-Passage-Reference-Parser** is a TypeScript rewrite (original CoffeeScript 2011) using a Peggy grammar, MIT-licensed. English is built in, 50 languages are bundled, and the Languages repo covers 2,000+ (Psalm titles English-only). Output is OSIS via `osis()`, `osis_and_translations()`, `osis_and_indices()` (gives character offsets, useful for linking prose) and `parsed_entities()`. The `osis_compaction_strategy` option takes b…bcvp. Latest is 4.0.0 (2026-07-31), with 4.1.0-alpha (2026-10-17) and 4.1.0 expected Jan 2027. The author considers the core feature-complete. The `plugins/jev` add-on is experimental and needs a paid API key. — [openbibleinfo/Bible-Passage-Reference-Parser](https://github.com/openbibleinfo/Bible-Passage-Reference-Parser)
- usfmtc also includes a scripture reference parser for USFM-style refs. — [PyPI usfmtc](https://pypi.org/project/usfmtc/)

### Inferences
- Pipeline for prose works: (1) parse the ThML/TEI structure into paragraphs with stable anchors (CCEL section IDs + paragraph index); (2) take `scripRef@parsed` where present and normalize it to OSIS; (3) otherwise run bcv_parser `osis_and_indices()` on paragraph text, choosing the versification from the source's declared translation (ThML `version`, often KJV for public-domain works); (4) map refs through the verse_map to the pivot scheme; (5) store `(work_id, anchor, char_start, char_end, osis_ref, scheme, method=markup|parser@ver, confidence)`. This gives a "who cites this verse" reverse index for search.
- Roman-numeral chapter refs (`viii. 27`) are common in older CCEL texts. Test the parser on this corpus specifically.
- CCEL hosts the patristics (ANF/NPNF) and confessions as ThML. Check the rights statement on each work, because some CCEL editions carry CCEL copyright on the markup even when the underlying text is public domain. *Unverified.*

### Gaps
- I did not verify CCEL's current licensing/terms for bulk ThML download, or whether CCEL has moved to a newer XML format.
- TEI patristic corpora (e.g. First1KGreek, Perseus canonical-greekLit with CTS URNs) were not researched this session. CTS URNs are the likely stable citation scheme for non-biblical ancient texts.
- No Rust or Python port of bcv_parser was found. A Python port, `python-bible`/`pythonbible`, exists (unverified).

---

## 6. Pipeline architecture practices (determinism, hashing, incremental builds, provenance, QA, orchestration)

### Takeaway
Use a content-addressed, make-like DAG implemented in Rust (or `just`/Make driving Rust binaries). Inputs are pinned by hash, each stage is a pure function from input hashes to output hashes, and the output package carries a provenance/rights manifest. Heavyweight orchestrators (Dagster) are not necessary for a corpus of this size. QA gates should include verse-count checks against the declared `.vrs`, versification sniffing, token-count diffs between releases, and a **Hebrew-aware Unicode policy**: do not blindly NFC-normalize pointed Hebrew for display text.

### Cited Findings
- Unicode canonical ordering of Hebrew points conflicts with typographic order (see the SBL Hebrew font manual). Normalization reorders shin/sin dot, dagesh and vowels, which can break rendering in some fonts. — [ConTeXt list](https://mailman.ntg.nl/archives/list/ntg-context@ntg.nl/thread/X4OC7XCYR4TSW3J53JD6FCRNZ7KMYXJR); [ContextGarden wiki](https://wiki.contextgarden.net/Special:MobileDiff/10132...28559)
- Meteg has combining class 22, after the vowels (10–20), so normalization moves it. U+034F COMBINING GRAPHEME JOINER is needed where its position matters. — [Wikipedia: Meteg](https://en.wikipedia.com/wiki/Meteg); [episteme wiki](https://episteme.tllm.fr/wiki/Meteg) (same text; not independent)
- BabelPad offers custom normalizations that override Hebrew combining classes using the SBL manual's Appendix B. — [BabelPad Custom Normalizations](https://babelstone.co.uk/Software/BabelPad_Custom_Normalizations.html)
- A 2003 Unicode list debate questioned whether this is a Unicode defect or a rendering issue. These sources are mailing lists and wikis, not current official Unicode guidance. — [Unicode ML 2003-06](https://www.unicode.org/mail-arch/unicode-ml/y2003-m06/0340.html); [Unicode ML 2003-07](https://unicode.org/mail-arch/unicode-ml/y2003-m07/0664.html)
- STEPBible-Data defines its own normalization rules for Hebrew glyph order and the allowed Greek glyphs. Different upstream datasets therefore apply different normalizations, which must be reconciled at ingest. — [STEPBible-Data](https://github.com/STEPBible/STEPBible-Data)
- BHSA keeps immutable versioned data side by side, a model for reproducible upstream pinning. — [ETCBC/bhsa](https://github.com/ETCBC/bhsa)
- STEPBible's CC BY 4.0 terms require that changes be recorded and shared and that mirrors stay current, which is a provenance obligation. — [STEPBible-Data](https://github.com/STEPBible/STEPBible-Data)
- usfmtc 0.4.x was published without PyPI provenance attestations. Pin by hash in lockfiles rather than trusting the index. — [PyPI usfmtc](https://pypi.org/project/usfmtc/)

### Inferences (design recommendations)
- **Source registry (`sources.toml`)**: for each upstream, record git URL + commit SHA (or release tarball + SHA-256), license SPDX ID, attribution string, NC/SA/EULA flags, upstream versification scheme, and the normalization the source claims. The fetch stage verifies hashes and fails on drift.
- **Stage DAG**: fetch → parse (format adapter → canonical intermediate model, e.g. USJ for translations and a token table for original languages) → normalize (Unicode policy, punctuation split) → tokenize + ID-mint (with lineage diff vs previous build) → versify (map to `org` pivot) → align (import standoff, or run aligner with pinned seed and version) → enrich (lexicon links, cross-refs) → index (tantivy/FST/SQLite FTS) → package (deterministic tar/zip with sorted entries and zeroed mtimes, BLAKE3/SHA-256 content address, then sign, e.g. minisign/ed25519 or Sigstore).
- **Determinism**: sorted iteration everywhere, no wall-clock or locale in outputs, fixed seeds for statistical aligners, pinned tool versions recorded in the manifest. Each stage's cache key = hash(stage code version + config + input hashes), the Bazel/Nix-style approach. Incremental rebuilds then fall out naturally: only stages whose key changed rerun.
- **Orchestration**: a small Rust DAG runner (or `cargo xtask` + a hashed cache directory) is enough. Dagster/Airflow add Python ops overhead without benefit for a few dozen sources. *No source verified this; it is an engineering judgment.* Python-only tools (usfmtc oracle, Text-Fabric export, eflomal) can run as hermetic subprocess stages with pinned venvs (`uv` lockfiles).
- **Unicode policy**: store original-language text **as received (byte-exact)** for display. Derive a separate *search key* form: NFC or NFD, then optionally strip cantillation, vowels or accents (for Greek, strip accents and breathings and fold final sigma). Index the search keys. Never round-trip display text through NFC for pointed Hebrew without testing against SBL ordering. Record the normalization form per field in the schema.
- **QA gates**: (1) chapter/verse counts match the declared `.vrs`; (2) Copenhagen sniffing agrees with the declared scheme; (3) every token has a unique ID and every alignment endpoint resolves; (4) token-count/hash diffs vs the previous release are reviewed, with large changes failing the build; (5) cross-check lemma and morphology code validity against TEHMC/TEGMC and OSHM code lists; (6) round-trip USFM→USJ→USFM with usfmtc vs usfm3 diffs; (7) a license-compatibility check that a package does not mix NC/EULA content into a redistributable bundle.
- **Provenance in package**: embed a manifest (JSON, possibly in-toto/SLSA-style) listing every source hash, every stage tool version, licenses and attributions, and the output content hash. Show attribution strings in the app's About/credits view, which satisfies the CC BY attribution requirement.

### Gaps
- No primary source was consulted on Dagster vs Make vs Rust build-tool trade-offs specifically for corpus pipelines.
- The current Unicode/SBL guidance (SBL Hebrew font manual 2.x, Unicode FAQ on Hebrew) was not fetched. The cited sources are older mailing lists.
- Signing scheme recommendations (minisign vs Sigstore vs TUF) were not researched here.
