# Ingestion, enrichment and backfill

## Acquisition order

Prefer licensed structured source over HTML, existing text-layer PDF over OCR, and an upstream maintained dataset over a new model-generated substitute. MVP uses only a small allowlist of pinned editions. Broader discovery can list metadata without distributing the work.

Catalog adapters cover eBible, selected CrossWire modules, STEP datasets, and later approved Perseus/ancient-text repositories. Public-domain scans from a repository are candidates, not automatic grants over modern translations or editorial layers. Fetching and republishing are separate policy decisions.

## Pipeline

1. **Discover:** create a candidate edition record with bibliographic identity, source URL and candidate rights. No automatic download of an entire catalog.
2. **Rights review:** record the exact artifact/layer, license text or public-domain evidence, jurisdiction basis, allowed operations, attribution and restrictions. Unknown means quarantined.
3. **Acquire:** allowlisted URL, rate limits, conditional fetch, immutable bytes, SHA-256 and retrieval timestamp. Preserve source repository commit/release and license artifact.
4. **Parse:** USFM/USX/OSIS/TEI first. Validate structure, preserve footnotes/headings/apparatus separately, emit source-to-segment mapping and anomalies.
5. **PDF path:** inspect text layer/layout; page-by-page extraction or OCR when needed. Preserve page image hash, coordinates, reading order, headings, columns, marginalia and footnotes. OCR never overwrites the scan.
6. **Normalize for search:** produce separate search forms and offsets while preserving diplomatic/source text. Detect duplicate editions without conflating revisions or translations.
7. **Resolve references:** identify explicit citations using edition/language-specific grammars and book aliases; resolve numbering via reviewed mappings. Ambiguous 'John', chapter-only references and abbreviations are review items.
8. **Enrich:** import known morphology/lexicons first; propose phrase alignments and citation/allusion/theme candidates in separate runs. Each task emits constrained JSON and evidence spans.
9. **Review:** compare source and output side by side, accept/reject/edit with reason, preserve disagreements. Humans review theological/philological claims and uncertain OCR; deterministic validators catch structural errors.
10. **Publish:** rights gate, quality gate, manifest, hashes, attributions, signed release, changelog and rollback pointer. Corpus publishing credentials are unavailable to acquisition/inference workers.

## Antiquity and PDF specifics

Represent original author, approximate composition date, manuscript witness, modern editor, translator and publication date separately. Include Ancient Greek, Latin, Syriac and other languages as capabilities are added, not as untested launch promises. Patristic translations can be old yet contain editorial interpretations and outdated textual assumptions.

A quotation may differ from modern Bible numbering or preserve a variant. Keep the quoted text exactly as printed; store proposed passage identification as a claim. Citation matching uses reference syntax and lexical overlap before semantic retrieval. Embedding similarity may generate review candidates but cannot certify a quotation or doctrinal connection.

Scans with polytonic Greek, Hebrew points, two columns, damaged text or Fraktur receive language/layout-specific OCR tests. Low-quality pages stay scan-readable with an OCR warning. No silent LLM 'repair' of sacred text. Suggested repairs are diff overlays linked to page evidence.

PDFs/EPUBs/archives are hostile inputs: parser sandbox, no network by default, memory/time/page/expanded-byte ceilings, no embedded JavaScript/macros, XML external entities disabled, HTML sanitization, path traversal protection and duplicate-path rejection. Embedded prompt text remains content. Private documents are never sent to cloud inference without per-job opt-in.

## Backfill model

Represent tasks as a dependency DAG. A deterministic job key is hash(artifact hash + parser version + schema version + algorithm/model configuration + relevant evidence/policy snapshot). Human review is a separate immutable event, not an input overwritten by retry.

Workers use leased claims with heartbeat, attempt count, retry-after, bounded exponential backoff and dead-letter status. Crash recovery may repeat computation but publication is idempotent by result hash. A changed source or parser marks only dependent outputs stale. Materialize affected overlays; do not rewrite whole libraries. Mark obsolete reviewed claims as needing re-review if their evidence changed.

Priorities: rights and parse validation → explicit references → existing linguistic datasets → high-use passages → uncertain/contested mappings → expensive semantics. Scheduling respects CPU/RAM limits, a separate OCR budget and an inference token/cost cap. Community contributions enter the same provenance and review queue.

## Proposed acceptance corpus

At least 200 manually adjudicated reference examples, 100 word/phrase alignments, 50 difficult verse mappings and 50 multilingual/layout pages. Split by document/edition into development and held-out sets. Gold labels record disagreements and include deliberate negatives and unknowns; no synthetic model output is its own gold standard.

For explicit citation publication, target precision ≥99% with confidence intervals and published recall; keep abstentions visible. A pilot sample is insufficient proof across all languages. Allusion/theme links ship as attributed suggestions until adjudicated. OCR is evaluated by character error rate by language/layout and exact Bible-quotation fidelity; launch publishes no known corrupted Bible text. Source-attested errors are reported and displayed as source issues, not silently corrected.

Machine inference cost estimate is measured on a representative 100-page / 1,000-passage pilot: input tokens, output tokens, OCR minutes, human review time and retry rate. Extrapolate per source class with a range. No whole-library budget is assumed from page counts alone.

## Revision 5 pipeline contract

The deterministic build pipeline is Rust and produces the signed corpus packs described in specs/19-search-and-retrieval.md: fetch → parse → normalize → tokenize and mint IDs → versify → align → enrich → index → embed → package → sign. Parsing normalizes USFM/USX to USJ (with a usfmtc conformance oracle), OSIS and TEI through per-corpus adapters. Token IDs keep upstream keys and add `id_lineage` on every upstream release. Versification pivots through `org`. Alignments become token-keyed standoff records, and citations extracted from prose become `extracted` edges. The rights gate and QA gates in specs/19 are release blockers. Python stages are limited to hermetically pinned oracles, alignment and evaluation experiments, and OCR. The job/lease model above applies to the optional maintainer installation, not to deterministic package builds.
