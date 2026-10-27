# Data model: preserved text, explicit claims

## Identity and layers

| Entity | Meaning and durable key |
|---|---|
| Work | Abstract work, e.g. Gospel according to Mark; stable UUID/URI |
| Witness | Manuscript or attested textual witness with catalog citation; not inferred from an edition name |
| Edition | Specific editorial text or translation, date/revision/language and text basis |
| Artifact | Exact fetched bytes and SHA-256, acquisition record and rights evidence |
| Segment | Edition-scoped stable passage/division with native label and content revision |
| TokenOccurrence | Edition + segment + stable occurrence ID; surface text and offsets into preserved representation |
| Lexeme | Namespaced language/lexicon identity; never just an unqualified Strong's number |
| Sense | Lexical sense in a cited lexicon/version, optional reviewed local extension |
| Concept | Named theme with definition, editorial viewpoint and references; not a word's universal meaning |
| Claim | Evidence-bearing edge or assertion with lifecycle and provenance |
| Annotation | User-private record with target selectors, body revisions and device provenance |
| Package | Immutable collection of approved artifacts/derived layers, rights and hashes |
| LocatorScheme | Citable coordinate system for a non-Bible work (canonical hierarchy such as book.chapter.section or question/answer, or a named print edition's pages), with reference grammar and version |
| SectionMap | Edition-to-canonical-locator mapping (equal, split, merge, partial, absent), analogous to the verse map |
| LinkIndex | Device-local, rebuildable merge of outbound edges from installed packs plus user edges; serves backlinks and library-relative counts (specs/19-search-and-retrieval.md) |

Separate immutable source, deterministic parse, machine proposals, reviewed scholarly overlay, and user annotations. A source correction creates a new revision and mapping; it does not silently replace the text beneath old notes. Stable upstream IDs, such as OSHB token IDs, are preserved in a namespace [S07]. Synthetic IDs include edition/revision context and never pretend continuity after retokenization.

## Reference and versification design

Store an edition's book identifiers, canon membership/order, division system and native labels. The internal address is edition-scoped, not `book:chapter:verse` alone. OSIS/USFM identifiers are external identifiers where applicable, not universal proof of identical content.

`ReferenceMapping` supports many-to-many segment sets, partial spans, certainty, mapping source and notes. Mapping status is exact, partial, ambiguous or unmapped. Support bridge labels, omitted verses, additions, split/merged verses, Psalm superscriptions, Hebrew/Greek Psalm numbering and chapters differing across traditions. Titles, introductions and apparatus live outside verse text.

Fixtures include Psalm 9/10 versus Greek numbering, Joel/Malachi chapter differences, Esther/Daniel additions, Mark 16:9–20, John 7:53–8:11 and 1 John 5:7–8. Expected behavior is tied to each chosen edition. Never silently harmonize them to one text or canon.

## Token and morphology design

Hebrew AND Aramaic language tags are explicit. A display word, orthographic token and morpheme need not coincide. Support clitics/prefixes/suffixes, maqqef, ketiv/qere, alternative analyses, Greek elision, accents and punctuation. Store upstream morphology strings plus parser namespace/version; derived structured features retain pointers to originals.

Preserve exact source bytes and canonical display text. Normalized search keys are separate with their normalization algorithm version. Do not globally normalize Hebrew on ingest; OSHB specifically warns about NFC handling [S07]. Offsets are defined in Unicode scalar values for serialized selectors, with platform adapters for UTF-16 and grapheme clusters. Include tests for combining marks, emoji in notes and bidi isolation.

A word study starts at an occurrence, offers the relevant lemma analyses, then displays senses in context and distributions with corpus denominators. Etymology and roots do not establish contextual meaning. Homographs, polysemy and disputed analyses remain distinguishable. Translated words aligned to a lexeme are evidence of rendering choices, not definitions of the lexeme.

## Typed claims

| Relation | Allowed interpretation |
|---|---|
| edition_revises | Documented editorial relationship between editions |
| edition_uses_source | Documented textual basis, possibly partial/book-specific |
| witness_supports_reading | Cited manuscript/apparatus evidence for a particular reading |
| span_aligns_with | Translation correspondence, many-to-many or explicitly null |
| occurrence_has_analysis | Source-backed or reviewed lemma/morphology proposal |
| occurrence_uses_sense | Contextual lexical judgment with cited evidence |
| explicitly_cites | Identifiable citation to a passage or to a work locator |
| quotes / alludes_to | Verbatim/near quotation or interpretive allusion; distinct thresholds |
| shares_theme | Editorial/theological connection, attributed to a scheme or reviewer |
| lexeme_etymologically_related | Historical linguistic assertion requiring evidence |

Every claim carries subject/object IDs, scope, evidence artifact and locator, source policy, producer/version, creation time, review state and reviewer decisions. Machine outputs also record model, parameters, prompt hash and run ID. Confidence is a calibrated task-specific estimate or null, never an invented universal certainty. Imported scholarly claims may be `source_attested` without local independent review; the UI says so.

Lifecycle: `proposed → source_attested or reviewed → published`, with `rejected`, `disputed`, `superseded` and `retracted` alternatives. Published disputed claims must be visibly attributed, never rendered as unqualified facts. Each transition is an audit event. A generated proposal cannot move itself to reviewed. Review state is independent of numerical confidence.

## Translation heritage UX

A passage may have several textual bases and several editorial influences. Show a graph of **documented edition-level relationships** separately from **inferred token alignments**. A word's similarity to an earlier translation is not evidence the later translator copied it. A revised translation may still consult Hebrew/Greek directly.

Each visible edge has an evidence drawer. A chain can stop with 'basis not documented'. Do not draw an English → English → Greek chain unless that exact relationship is documented. Distinguish manuscript lineage, edition revisions, lexical correspondences and etymology with explicit labels. Alternate readings and unresolved source relationships remain accessible.

## Notes and synchronization

Anchor an annotation with edition ID, content revision, segment/token span, a W3C-style TextQuoteSelector (exact/prefix/suffix when rights permit), and position selectors [S17]. PDF anchors additionally use artifact hash, page index, bounding boxes and extracted-text revision. Text coordinates and visual boxes serve different purposes.

Reanchoring tries exact stable IDs, then revision mapping, then exact quote context; ambiguous matches become orphaned anchors awaiting user choice. Never silently attach a note to the nearest different passage. Preserve its original target in exports.

For v1 sync use an append-only operation log, globally unique operation IDs, causal parents and per-device sequence numbers. Concurrent note bodies become visible sibling revisions with a resolution UI; no silent last-write-wins text loss. Highlight/tag additions/removals and tombstones have specified deterministic merge rules and property tests. Out-of-order, duplicate and delayed operations converge. Tombstone compaction requires acknowledgement/retention policy; rejoining a stale device cannot resurrect deleted notes. A future CRDT is justified only if collaborative editing becomes a requirement.

## Revision 2 entities

- `Workspace`: versioned ordered pane layout with pane IDs, content types, edition/revision/reference, link group, pin state and independent navigation history. Notes and research panes reference app-owned note/thread IDs; credentials and copied private text are excluded.
- `NoteBinding`: stable note ID, authority (`app` or `vault`), opaque vault ID, relative file path, last-seen content hash and synchronization state. A path is not identity. Markdown user content remains independent of generated metadata and screenshots.
- `ReadingSnapshot`: immutable background asset hashes and dimensions, pane rectangles, captured source spans/revisions, workspace recipe, capture scope and explicit private-content flags. Never automatically reflow.
- `InkDocument`: snapshot or blank-canvas target, immutable coordinate-space ID, strokes/masks with per-operation IDs and brush versions, optional native fidelity blob and portable geometry. A fresh capture gets a new snapshot/coordinate-space ID.
- `ResearchThread`: local identity, message/branch DAG, provider-account reference without secrets, context selections and evidence bundles. Provider session handles are replaceable adapter metadata.
- `EvidenceBundle`: immutable retrieved spans, locators/edition hashes, rights-policy snapshot and tool provenance for a particular turn. A generated answer is not a published scholarly claim.

Optional encrypted sync covers workspaces, chats and ink assets but never provider credentials. Notes exported to an Obsidian vault follow the vault's own encryption/sync policy. Snapshot-to-live-text links aid navigation; they do not relocate strokes. Version migrations preserve both original coordinate geometry and canonical text anchors. New illustrative schemas/fixtures document these boundaries but are not complete production protocols.

## Collection and source-selection contracts

`StudyCollection` has stable identity, title, optional primary passage/theme, ordered item memberships, groups/tags, revisions and private ownership. Membership is independent of a source/annotation's lifetime. `CollectionItem` distinguishes a source reference/excerpt, personal observation, research excerpt and margin attachment. Source-bearing entries retain exact edition/artifact/revision/locator and rights-policy snapshot; machine output retains its generated label and supporting evidence without becoming a source quotation. These are contracts for B006/B058, not yet additional validated JSON fixtures.

`VaultExportReceipt` maps collection/item IDs to opaque vault ID, relative resource-note path, block ID, last-export content hash and source revision. File paths are not identity. Updates reconcile managed blocks, preserve user-authored manuscript text and retain conflicts. A portable Markdown representation and versioned JSON archive carry provenance even without the custom app URI.

Source-collection manifests record membership, provenance and acquisition criteria. Research turns retain selected collection IDs/revisions and the explicit user request. No `EditorialProfile` or implicit theological persona is part of the runtime contract. Confessional articles/questions have edition-scoped locators; their proof-text references are attributed source claims, not asserted entailments. A collection preference affects retrieval scope, never source bytes, token mappings or an assumed answer stance. See specs/15-reformed-study-and-collections.md.
