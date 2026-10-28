# Primary-source inventory and reuse decisions

Checked 26 October 2026. These are research findings, not legal clearance. Production import must archive the exact license and artifact version. Source IDs in other documents refer here.

## S01 — Paperclip adapter and agent configuration

[Paperclip adapter and agent configuration](https://github.com/paperclipai/paperclip/blob/master/docs/adapters/overview.md) · [Additional primary source](https://github.com/paperclipai/paperclip/blob/master/docs/guides/board-operator/managing-agents.md)

Adapters bridge orchestration and runtimes; installed schemas and model support must be inspected. Agent budget/run policy varies by version.

Status: `implementation_reference`; no production artifact pinned yet.

## S02 — ESV publisher permissions

[ESV publisher permissions](https://www.crossway.org/permissions/) · [Additional primary source](https://api.esv.org/)

Copyrighted translation with specific quotation/digital terms; ordinary access does not grant the proposed open-corpus or AI translation workflow. Default exclusion; request explicit written scope if needed.

Status: `restricted`; no production artifact pinned yet.

## S03 — ESV API terms

[ESV API terms](https://api.esv.org/)

API imposes storage/display/redistribution limits and other conditions. A freely obtainable API is not an offline open-text license.

Status: `restricted`; no production artifact pinned yet.

## S04 — World English Bible

[World English Bible](https://ebible.org/eng-web/webfaq.htm) · [Additional primary source](https://ebible.org/study/content/texts/engwebp/about.html)

Publisher identifies the text as public domain; preserve edition and text integrity. Changed text must not be represented as the unchanged World English Bible.

Status: `candidate_open`; no production artifact pinned yet.

## S05 — STEPBible Data

[STEPBible Data](https://github.com/STEPBible/STEPBible-Data/blob/master/README.md)

Repository states CC BY 4.0 with attribution/change recording. Inspect each chosen file and its upstream dependencies; useful for tagged texts, lexical data and mappings.

Status: `acquired_flagged` (B003, 28 October 2026): TBESG, TBESH, TAGNT, TAHOT and TVTMS pinned at commit `be4191f0082aeedddb0eff0c02c966292b2bd641`, rights/S05.json; per-file upstreams (Online Bible BDB, NA27/28 text, MorphGNT grammar, ETCBC morphology) need provenance research before redistribution.

## S06 — SBL Greek New Testament

[SBL Greek New Testament](https://github.com/Faithlife/SBLGNT) · [Additional primary source](https://www.sblgnt.com/)

Current upstream describes CC BY 4.0. Pin exact text edition; critical editorial choices are not interchangeable with other Greek editions.

Status: `acquired_clear` (B003): repository data pinned at commit `d7f35adca9ec410e1d1499c63b4dd989d76c8d4d`, CC BY 4.0, rights/S06.json.

## S07 — Open Scriptures Hebrew Bible

[Open Scriptures Hebrew Bible](https://github.com/openscriptures/morphhb) · [Additional primary source](https://github.com/openscriptures/morphhb/blob/master/LICENSE.md)

WLC underlying text is public domain; morphology/lemma layer is CC BY 4.0. Preserve upstream token IDs and normalization cautions.

Status: `acquired_clear` (B003): `wlc/` pinned at commit `3d15126fb1ef74867fc1434be1942e837932691f`, rights/S07.json.

## S08 — unfoldingWord source-language resources

[unfoldingWord source-language resources](https://unfoldingword.org/for-translators/content/)

UHB/UGNT and related grammars/lexicons are open-licensed candidates, generally CC BY-SA 4.0 on the referenced page. Check the exact work/version and trademark terms before adaptation.

Status: `candidate_open`; no production artifact pinned yet.

## S09 — OpenBible cross references

[OpenBible cross references](https://openbible.info/labs/cross-references/)

Roughly 340,000 links, primarily derived from Treasury of Scripture Knowledge, with additional sources/votes. Page links a CC BY license; ESV quotations on the site are separate. Import reference links/attribution only after recording the exact linked license. Votes are not accuracy scores.

Status: `acquired_clear` (B003): cross-references.zip of 2026-10-26 with the page's CC BY 4.0 statement, 344,799 reference rows and no verse text, rights/S09.json.

## S10 — Perseus canonical Greek literature

[Perseus canonical Greek literature](https://github.com/PerseusDL/canonical-greekLit)

Repository default CC BY-SA 4.0 unless indicated otherwise, with varying object rights and unverified headers. Check each TEI edition/translation and preserve attribution.

Status: `per_item_review`; no production artifact pinned yet.

## S11 — CCEL and Early Church Fathers

[CCEL and Early Church Fathers](https://www.ccel.org/about/copyright.html) · [Additional primary source](https://ccel.org/s/schaff/ecf/index.html)

Ancient/patristic discovery source. Many underlying books are US public domain, but site content/editions and permission statements are mixed. Do not blanket-import; seek a clearly licensed artifact or separate public-domain transcription.

Status: `acquired_flagged` (B003): Calvin's Institutes (Beveridge) and the Westminster standards fetched as ThML and text with CCEL's copyright policy; flagged for digitization rights and (Confession) edition, rights/S11-ccel-calvin-institutes.json and rights/S11-ccel-westminster-standards.json.

## S12 — CrossWire SWORD and module rights

[CrossWire SWORD and module rights](https://www.crosswire.org/sword/about/license.jsp) · [Additional primary source](https://wiki.crosswire.org/Copyright)

Engine is GPL; each text module has its own rights. Distribution permission to CrossWire does not automatically authorize this project to redistribute everything.

Status: `per_item_review`; no production artifact pinned yet.

## S13 — Tailscale network layers

[Tailscale network layers](https://tailscale.com/docs/concepts/control-data-planes) · [Additional primary source](https://tailscale.com/security)

WireGuard data transport is distinct from coordination/identity/relay behavior. Mesh access is not app authorization or anonymity.

Status: `implementation_reference`; no production artifact pinned yet.

## S14 — Headscale DERP self-hosting

[Headscale DERP self-hosting](https://headscale.net/stable/ref/derp/)

Supports embedded/custom DERP; public-relay map exclusion must be explicit and connectivity tested. Self-hosting can create its own availability bottleneck.

Status: `implementation_reference`; no production artifact pinned yet.

## S15 — Keycloak OIDC

[Keycloak OIDC](https://www.keycloak.org/securing-apps/oidc-layers)

Reference for self-hosted OIDC and supported flows; use authorization code plus PKCE and carefully scoped redirects.

Status: `implementation_reference`; no production artifact pinned yet.

## S16 — SQLite browser and Expo storage

[SQLite browser and Expo storage](https://sqlite.org/wasm/doc/tip/persistence.md) · [Additional primary source](https://docs.expo.dev/versions/latest/sdk/sqlite/)

Browser VFS/worker support and cross-origin isolation requirements vary. Test chosen storage adapter and Safari rather than assuming native parity.

Status: `implementation_reference`; no production artifact pinned yet.

## S17 — W3C Web Annotation Data Model

[W3C Web Annotation Data Model](https://www.w3.org/TR/annotation-model/)

TextQuote and TextPosition selectors provide useful interoperable anchor conventions; immutable edition context and reanchoring policy remain project requirements.

Status: `implementation_reference`; no production artifact pinned yet.

## S18 — MorphGNT SBLGNT

[MorphGNT SBLGNT](https://github.com/morphgnt/sblgnt/blob/master/README.md)

Morphological analysis uses CC BY-SA 3.0; README references an older SBLGNT EULA whereas current SBLGNT upstream is CC BY 4.0. Resolve edition/version provenance and layered obligations, not one blanket license.

Status: `acquired_restricted` (B003): version 6.12 pinned at commit `aaed91e57c8e4a8dc9a2383e129ca5e75fe6393d`; share-alike analysis and an out-of-date EULA statement, a restricted module, rights/S18.json.

## S19 — NixOS Android tooling

[NixOS Android tooling](https://wiki.nixos.org/wiki/Android)

androidenv supports composing SDK inputs. Exact SDK/NDK/license/build configuration remains an implementation verification task.

Status: `implementation_reference`; no production artifact pinned yet.

## S20 — AndBible

[AndBible](https://github.com/AndBible/and-bible) · [Additional primary source](https://docs.andbible.org/en/latest/faq.html)

GPL-3.0-or-later offline Android study app. Study panes, note/export and module workflows; no license-blind copying into differently licensed clients.

Status: `inspiration`; no production artifact pinned yet.

## S21 — Ezra Bible App

[Ezra Bible App](https://github.com/ezra-bible-app/ezra-bible-app)

SWORD-based cross-platform topical study with a GPL-2.0 repository label. Inspect exact licensing before reuse; useful workflow inspiration for tags and notes.

Status: `inspiration`; no production artifact pinned yet.

## S22 — Calibre

[Calibre](https://github.com/kovidgoyal/calibre)

Open-source ebook management/conversion reference. Useful for library ownership/export concepts; dependency and copied-code licenses require separate review.

Status: `inspiration`; no production artifact pinned yet.

## Initial corpus proposal

- First complete English text: one pinned World English Bible edition.
- Second parallel text: a separately reviewed historical English edition (e.g. ASV 1901) or a verified open modern edition. This inventory does not clear a particular ASV digital artifact. P0 resolves that choice; test fixtures may use synthetic content until then.
- Source languages: pinned OSHB/WLC and SBLGNT with separately licensed morphology. Prefer STEP's appropriate reviewed datasets where they reduce unnecessary invention.
- Cross references: OpenBible reference data with exact linked license and source attribution; preserve its TSK ancestry and distinguish voting/ranking data. Do not scrape ESV verse wording from its pages.
- Ancient work: a short text such as the Didache or a selected patristic work in a specifically cleared original/translation edition. Discover through Perseus/CCEL, but acquire only after artifact-level rights review. A modern English translation is not automatically free because the Didache is ancient.

## Next source-research tasks, deliberately not pre-cleared

Find usable editions of the Septuagint, Vulgate/Old Latin and Syriac/Peshitta; record textual scope, editor, translation layers and rights. Evaluate ancient Jewish and wider Greco-Roman context, Josephus/Philo, early fathers and later commentaries without mixing eras. Consider TEI/CTS conventions and USFM/USX/OSIS adapters after checking their official specifications and chosen parser licenses. Evaluate public-domain historical German Bibles separately from contemporary revisions.

No Nestle-Aland/UBS apparatus, BHS/BHQ apparatus, BDAG/HALOT, contemporary copyrighted commentary or Logos library content is assumed available. Public-domain lexicons may be useful historically but their age and scholarly limitations must be labeled.

## Inspiration scope

Start with public workflows and existing open readers rather than attempting to decode Logos internals. AndBible, Ezra and SWORD cover much of the desired offline library/study behavior. STEP informs original-language navigation. Calibre informs user-owned library management and export. A later interoperability project would have its own evidence, compatibility and rights review; this blueprint requires none.


# Revision 2 primary sources

Checked 26 October 2026. Provider documentation is version-sensitive; capability and terms checks are release gates.

## S23 — OpenAI ChatGPT plan usage for OSS

[OpenAI ChatGPT plan usage for OSS](https://developers.openai.com/siwc/token-sharing-open-source)

Official optional authorization path for eligible open-source/locally hosted apps; platform and deployment conditions require verification.

Status: `implementation_reference`.

## S24 — Codex app-server plan usage

[Codex app-server plan usage](https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server)

Documented app-server integration exists; catalog listing alone does not prove model entitlement.

Status: `implementation_reference`.

## S25 — OpenAI plan-usage preview limitations

[OpenAI plan-usage preview limitations](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations)

The preview has distinct request/tool/history restrictions; adapter conformance must be tested rather than assuming ordinary API parity.

Status: `implementation_reference`.

## S26 — Obsidian Vault API

[Obsidian Vault API](https://github.com/obsidianmd/obsidian-developer-docs/blob/main/en/Plugins/Vault.md)

Vault API includes read and guarded process-style updates; use it for plugin file operations.

Status: `implementation_reference`.

## S27 — Obsidian mobile development

[Obsidian mobile development](https://docs.obsidian.md/Plugins/Getting%20started/Mobile%20development)

Node/Electron APIs are unavailable on mobile; platform support must be explicit.

Status: `implementation_reference`.

## S28 — Obsidian native block links

[Obsidian native block links](https://obsidian.md/help/links)

Native block IDs and #^ references target actual note blocks; materialized scripture can use the same mechanism.

Status: `implementation_reference`.

## S29 — perfect-freehand

[perfect-freehand](https://github.com/steveruizok/perfect-freehand)

MIT stroke geometry library; it is not a complete handwritten-note editor.

Status: `candidate_open`.

## S30 — Excalidraw source and license

[Excalidraw source and license](https://github.com/excalidraw/excalidraw/blob/master/LICENSE)

MIT editor candidate; validate the selected package, assets and platform behavior.

Status: `candidate_open`.

## S31 — tldraw licensing

[tldraw licensing](https://tldraw.dev/faq)

Current SDK is source-available/commercially licensed; excluded from the fully open-source default.

Status: `restricted`.

## S32 — Claude Code product integration terms

[Claude Code product integration terms](https://code.claude.com/docs/en/legal-and-compliance)

Current terms describe conditional integration of unmodified Claude Code with end-user authentication; review exact deployment obligations.

Status: `implementation_reference`.

## S33 — Claude Agent SDK authentication statement

[Claude Agent SDK authentication statement](https://platform.claude.com/docs/en/agent-sdk/overview)

SDK page still restricts third-party subscription login without approval; reconcile with the product-integration page before enabling the selected route.

Status: `implementation_reference`.

## S34 — Claude Code headless mode

[Claude Code headless mode](https://code.claude.com/docs/en/headless)

Programmatic mode exists; bare mode does not use subscription OAuth. Configuration isolation must be designed for the chosen auth mode.

Status: `implementation_reference`.

## S35 — OpenAI self-hosted VM plan usage

[OpenAI self-hosted VM plan usage](https://developers.openai.com/siwc/token-sharing-open-source/self-hosted-vms)

Documented personal VM path has credential/host-ID/refresh and current revocation-attribution limitations; do not generalize to a shared commercial relay.

Status: `implementation_reference`.

## S36 — Android Ink API

[Android Ink API](https://developer.android.com/develop/ui/views/touch-and-input/stylus-input/about-ink-api)

Official low-latency ink API candidate for a native adapter; verify exact library version/license and portable data conversion in the spike.

Status: `implementation_reference`.

## Revision 3 editorial and design references

**S37 — Westminster Confession, OPC edition.** Primary confessional reference; chapter 1 supports distinguishing Scripture and subordinate confession. Exact edition, proof texts and rights still require acquisition review. Source: https://opc.org/wcf.html

**S38 — Three Forms of Unity reference.** Primary church descriptions identify Belgic Confession, Heidelberg Catechism and Canons of Dort. No adoption of those churches’ other policies is implied. Search retrieval succeeded; the direct CRC page returned 403 during this audit. Source: https://www.crcna.org/welcome/beliefs?language=en Additional reference: https://www.frcna.org/our-beliefs

**S39 — 1689 London Baptist Confession, RBNet.** Primary Reformed Baptist confessional reference; offered as a possible identified collection, not silently selected as the project confession. Source: https://reformedbaptistnetwork.com/1689lbcf/

**S40 — WCAG 2.2.** Normative web accessibility requirements; product target sizes and chrome budgets are separate design proposals. Source: https://www.w3.org/TR/WCAG22/

**S41 — Obsidian workspace and ribbon.** Documented collapsible workspace elements and hideable ribbon inform independent compact-workspace prototypes. Source: https://obsidian.md/help/workspace Additional reference: https://obsidian.md/help/User%2Binterface/Ribbon

**S42 — Zed workspace actions.** Documented dock toggles and editor zoom offer relevant focus/restore interaction patterns; no copied implementation. Source: https://zed.dev/docs/all-actions

**S43 — Zotero collections and tags.** Documented nonduplicating collection membership informs source gathering. Zotero integration is not in scope. Source: https://www.zotero.org/support/collections_and_tags

These pages establish reference context, not permission to import their text. Screenshots are private user-provided workflow references and are not committed or used as corpus sources. The product interaction choices in specs/16-design-guide.md are our proposals; no usability or accessibility claim about a competitor is inferred from six screenshots.

## Revision 5 stack references

**S44 — UniFFI.** Mozilla's Rust binding generator for Swift, Kotlin and Python; candidate for the core's native bindings, verified in B061. Source: https://mozilla.github.io/uniffi-rs/ Additional reference: https://github.com/mozilla/uniffi-rs

**S45 — Apple TextKit 2.** Text layout system for UIKit/AppKit text views; candidate reading surface measured in B005. Source: https://developer.apple.com/documentation/uikit/textkit

**S46 — Tauri.** Rust desktop shell with a system web view; candidate Windows/Linux host for the web UI. Source: https://tauri.app/ Additional reference: https://github.com/tauri-apps/tauri

## Revision 5 retrieval and pipeline references

Checked 27 October 2026 through the research in reports/Bible corpus search architecture.md. Research findings, not legal clearance.

**S47 — MACULA Greek and Hebrew.** Clear Bible syntax trees, semantic roles, referents, glosses and morphology with token xml:ids. Clear's layers are CC BY 4.0; Louw-Nida domain columns are UBS data used with permission and not openly licensed. Token counts computed: SBLGNT 137,740; Hebrew 475,910 rows. Source: https://github.com/Clear-Bible/macula-greek Additional reference: https://github.com/Clear-Bible/macula-hebrew

**S48 — Text-Fabric data model and search.** Slot/feature/edge corpus model with template search, unit nesting, agreement and quantifiers. Design reference for the linguistic engine and correctness oracle on N1904; no code reuse assumed. Source: https://annotation.github.io/text-fabric/tf/about/datamodel.html Additional reference: https://annotation.github.io/text-fabric/tf/about/searchusage.html

**S49 — sqlite-vec.** SQLite vector extension (Apache-2.0/MIT); stable v0.1.9 after 2026 revival; binary and int8 vectors with brute-force search; ANN indexes only in alpha. iOS/WASM packaging to verify. Source: https://github.com/asg017/sqlite-vec Additional reference: https://github.com/asg017/sqlite-vec/releases

**S50 — roaring-rs.** Pure-Rust compressed bitmaps (Apache-2.0/MIT), compiles to mobile and wasm32; candidate posting-list format for the linguistic engine. Source: https://github.com/RoaringBitmap/roaring-rs Additional reference: https://github.com/RoaringBitmap/CRoaring

**S51 — Versification specification.** Copenhagen Alliance JSON versification mappings and scheme-sniffing rules based on Paratext .vrs files; now maintained at nida-institute/versification. Base scheme is org. Source: https://github.com/Copenhagen-Alliance/versification-specification Additional reference: https://github.com/nida-institute/versification

**S52 — Berean Standard Bible terms.** Public domain since April 2023; altered texts are asked not to use the Berean name. Candidate English base text and translation-research input. Source: https://berean.bible/terms.htm Additional reference: https://berean.bible/

**S53 — usfmtc reference implementation.** USFM Technical Committee reference parser (MIT) for USFM/USX/USJ with versification remapping; CI conformance oracle for the Rust parser. Source: https://pypi.org/project/usfmtc/ Additional reference: https://github.com/usfm-bible/tcdocs

**S54 — Bible Passage Reference Parser.** MIT reference parser with OSIS output and character offsets in many languages; candidate citation extractor for commentaries and prose. Source: https://github.com/openbibleinfo/Bible-Passage-Reference-Parser Additional reference: https://www.openbible.info/labs/reference-parser/

**S55 — Clear Bible alignments.** Token-keyed standoff alignments (data CC BY 4.0, code MIT) as a working Scripture Burrito alignment proposal; schema and translation coverage to verify. Source: https://github.com/Clear-Bible/Alignments Additional reference: https://github.com/Clear-Bible/macula-greek

**S56 — pgvector.** PostgreSQL vector extension (PostgreSQL License) with HNSW/IVFFlat; optional server-tier store for multi-user and large vector sets. Source: https://github.com/pgvector/pgvector Additional reference: https://www.postgresql.org/docs/

**S57 — UBS open access semantic dictionaries.** UBS semantic dictionaries of biblical Greek and Hebrew released openly (reported CC BY-SA 4.0); open substitute for Louw-Nida domain data. Exact repository license to verify. Source: https://translation.bible/open-access-resources/ Additional reference: https://github.com/ubsicap

**S58 — ETCBC BHSA.** Scholarly Hebrew Bible database with versioned releases; CC BY-NC 4.0, so restricted to an optional user-downloaded module. Source: https://github.com/ETCBC/bhsa Additional reference: https://etcbc.github.io/bhsa/

**S59 — SIL Serval.** MIT translation and word-alignment API behind Scripture Forge drafting; drafts land in a separate project and are human-edited and checked before merge. Workflow reference for the translation lab. Source: https://github.com/sillsdev/serval Additional reference: https://scriptureforge.org/

**S60 — Anthropic Citations API.** Returns cited_text with valid pointers into supplied documents; custom-content blocks enable verse-level citation. Not combinable with structured outputs. Source: https://platform.claude.com/docs/en/build-with-claude/citations Additional reference: https://platform.claude.com/docs/en/build-with-claude/structured-outputs

**S61 — Patristic biblical-reuse retrieval evaluation.** 2026 gold set of 2,935 expert-verified biblical parallels in patristic Greek/Latin; small adapted encoders outperform generic multilingual models. Krahn et al. document translation bias in distilled Ancient Greek embeddings. Source: https://arxiv.org/abs/2607.24542 Additional reference: https://arxiv.org/abs/2308.13116

**S62 — CTS URN specification.** Canonical Text Services URNs give works hierarchical, edition-independent citation coordinates (work, version, passage); model for non-Bible locator schemes. Source: https://cite-architecture.github.io/ctsurn_spec/

**S63 — FRBR conceptual model.** Work/expression/manifestation separation; links target the work and section, resolving to an installed expression or edition. Source: https://www.loc.gov/cds/downloads/FRBR.PDF

**S64 — SQLite implementation limits.** SQLITE_MAX_ATTACHED defaults to 10 and cannot exceed 125; per-pack files therefore need a merged device-local link index for backlinks. Source: https://www.sqlite.org/limits.html

**S65 — PGlite extensions and storage.** Postgres 18 in WASM (0.5.x, Oct 2026) with Apache AGE and pgvector packages; IndexedDB or OPFS persistence, OPFS unsupported in Safari; single connection. Source: https://pglite.dev/extensions/ Additional reference: https://pglite.dev/docs/filesystems

**S66 — PGlite v0.4 roadmap and native bindings.** Native libpglite for mobile is listed as coming next without a date; the bindings repository is a draft. No shipped iOS/Android runtime. Source: https://electric.ax/blog/2026/03/25/announcing-pglite-v04 Additional reference: https://github.com/electric-sql/pglite-bindings

**S67 — Apache AGE.** openCypher graph extension for PostgreSQL 11-18, Apache-2.0, maintained in 2026; available in PGlite as a 0.0.x package with WASM memory limits. Source: https://github.com/apache/age Additional reference: https://pglite.dev/extensions/age

**S68 — SQL/PGQ reverted from PostgreSQL 19.** SQL/PGQ property-graph queries were committed for PostgreSQL 19 and reverted on 2026-09-07 over design issues; PostgreSQL 20 at the earliest. Source: https://git.postgresql.org/gitweb/?p=postgresql.git;a=commit;h=b1f106c80cbeb18d3a0219994d98a51a6eca8ede Additional reference: https://www.commandprompt.com/blog/two-features-just-left-postgresql-19/

**S69 — LadybugDB.** MIT Kuzu fork, very active (v0.22.0, Oct 2026); Rust crate and official WASM build; Swift binding exists, iOS and Android builds unverified. Source: https://github.com/LadybugDB/ladybug

**S70 — GraphQLite and Grafeo.** GraphQLite adds Cypher and graph algorithms to SQLite files (MIT, C, Rust crate). Grafeo is a young pure-Rust Apache-2.0 multi-language graph engine with a WASM binding. Source: https://github.com/colliery-io/graphqlite Additional reference: https://github.com/GrafeoDB/grafeo

**S71 — Oxigraph.** Rust RDF/SPARQL store, MIT/Apache, active in 2026; suits linked-data export of work locators, not the runtime query path. Source: https://github.com/oxigraph/oxigraph

**S72 — CoreQuarry (re-Isearch ib, Schmate, bert.cpp).** Apache-2.0 C++17 hybrid retrieval kernel from the 1995 Isearch lineage, published 2026: positional and structural operators (PEER, XPEER, ANCESTOR, WITHIN, NEAR), score shaping for agents (MAYBE, NARROW, PROMOTE, DEMOTE), ggml/HNSW vectors. Single maintainer, no bindings or mobile/WASM builds, Schmate unlicensed. Design reference and possible server-tier engine; not a dependency. Source: https://corequarry.com/ Additional reference: https://github.com/re-Isearch/ib

**S73 — Nestle 1904 Greek New Testament in Text-Fabric (CenterBLC/N1904).** Text-Fabric 1.0.0 dataset (137,779 word slots; lemma, morphology, gloss, Strong's, nested word groups, clauses, sentences) converted by CBLC/ETCBC from MACULA Greek's Nestle 1904 lowfat trees (CC BY 4.0); the repository itself is MIT. Louw-Nida domain columns are UBS data and are not ingested. B064's R6 correctness oracle: fifty engine queries matched Text-Fabric 13.1.1 counts at commit def0fd230b9ab77c11c371b273d020b1a573c884. Source: https://github.com/CenterBLC/N1904 Additional reference: https://centerblc.github.io/N1904/about.html

**S74 — sqlite-wasm-rs and sqlite-wasm-vfs (SQLite compiled for wasm32-unknown-unknown).** MIT crate that compiles the SQLite amalgamation (FTS5, RTREE, session on) for wasm32-unknown-unknown with a musl shim, exposes the full C API and is rusqlite 0.41's own wasm backend; sahpool (OPFS) and relaxed IndexedDB VFSs live in sqlite-wasm-vfs. B064 built the engine on it: 0.5.5 bundles SQLite 3.53.0 and works with wasm-bindgen 0.2.121; 0.6.x bundles 3.53.4 and needs wasm-bindgen 0.2.128. Neither reaches the 3.54 OPFS fix specs/19 requires without a custom source directory. Source: https://github.com/Spxg/sqlite-wasm-rs Additional reference: https://crates.io/crates/sqlite-wasm-rs

**S75 — Project Gutenberg: Calvin's Institutes, John Allen translation.** eBooks #45001 (vol. 1, 2014) and #64392 (vol. 2, 2021), plain text, public domain in the United States under the Project Gutenberg licence. It is the Allen (1813) translation, not the Beveridge (1845) one B068 named; acquired by B003 as the licence-clean alternative text of the same work while the Beveridge digitization is researched. Source: https://www.gutenberg.org/ebooks/45001 Additional reference: https://www.gutenberg.org/ebooks/64392

## B003 acquisition of the initial corpus (28 October 2026)

The owner decided to acquire the whole licence-clean slate and to flag, rather than exclude, anything whose provenance needs more research; the owner signs by merging the pull request and, per record, by setting `status`, `reviewer` and `reviewed_at`. Every artifact's exact bytes live on the NAS under `/Volumes/office-hub/openlogos/corpora/<source>-<name>/` with a `MANIFEST.sha256` beside them; the repository holds the manifest's hash and a copy of the manifest under `rights/<record>/`, the record `rights/<record>.json` (schema rights-record 1.1) and the registry pin in specs/sources.json (schema 1.1). No corpus byte is committed. The statuses `acquired_clear`, `acquired_flagged` and `acquired_restricted` mean the bytes and the record exist and what the agent proposes; they are not clearance. The commands, sizes, timings and every flag are in docs/evidence/B003.md.

| Source | Record | Licence | Proposed | Flag in one line |
|---|---|---|---|---|
| S52 Berean Standard Bible | rights/S52.json | public-domain dedication (2023-04-30) | clear | Berean name only on unaltered text |
| S06 SBLGNT | rights/S06.json | CC BY 4.0 | clear | none |
| S73 Nestle 1904 Text-Fabric | rights/S73.json | MIT over CC BY 4.0 | clear | Louw-Nida features not ingested |
| S07 OSHB / WLC | rights/S07.json | public domain text, CC BY 4.0 markup | clear | none |
| S47 MACULA Greek, MACULA Hebrew | rights/S47-macula-greek.json, rights/S47-macula-hebrew.json | CC BY 4.0 | clear | Louw-Nida and SDBH columns are a restricted module |
| S09 OpenBible cross references | rights/S09.json | CC BY 4.0 | clear | none (votes are not accuracy scores) |
| S55 Clear alignments | rights/S55.json | CC BY 4.0 data, MIT code | clear | legacy ESV/CSB/LEB/NET folders not fetched |
| S75 Gutenberg Institutes (Allen) | rights/S75.json | public domain (US) | clear | not the Beveridge translation |
| S05 STEPBible | rights/S05.json | CC BY 4.0 with per-file upstreams | flagged: provenance research | Online Bible BDB, NA text, MorphGNT grammar, ETCBC morphology |
| S11 CCEL Institutes (Beveridge) | rights/S11-ccel-calvin-institutes.json | public-domain text, CCEL edition policy | flagged: provenance research | CCEL permission or an independent digitization |
| S11 CCEL Westminster standards | rights/S11-ccel-westminster-standards.json | public-domain text, CCEL edition policy | flagged: provenance research | the Confession is the American revision, not 1646 |
| S57 UBS dictionaries | rights/S57.json | CC BY-SA 4.0 | restricted module | share-alike |
| S58 ETCBC BHSA | rights/S58.json | CC BY-NC 4.0 | restricted module | non-commercial, user-downloaded only |
| S18 MorphGNT | rights/S18.json | CC BY-SA 3.0, old SBLGNT EULA cited | restricted module | share-alike; EULA mismatch |

Not acquired: Thayer's Greek-English Lexicon (1889). The lexicon is public domain but no digitization with a stated digitizer and licence was identified without downloading an unknown-rights scan or module; the provenance search (Internet Archive scans of the 1889 Harper edition with their own OCR, or a transcription project that states its source and terms) is recorded in docs/evidence/B003.md as open. The World English Bible (S04) and the ancient works proposed above (Didache, a patristic text) were not part of the owner's slate and stay `candidate_open` and `per_item_review`.
