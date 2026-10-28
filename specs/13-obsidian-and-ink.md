# Personal notes, Obsidian and handwriting

## Ownership and scope

The built-in notes feature remains complete without Obsidian. Obsidian is the sole first-class external writing integration, while remaining optional to install. The maintained plugin, interchange formats and our adapters are open source. P1 includes scripture block references and collection handoff; P2 adds fuller bidirectional vault editing. No other dedicated writing-app adapters are planned. There is no requirement for Obsidian Sync or an Obsidian account. Vault files remain ordinary Markdown and attachments readable by other tools.

Two explicitly selected note modes:

1. App-managed: local annotation database is authoritative; export/import Markdown and attachments or explicitly publish selected notes to a vault.
2. Vault-backed: a user-selected Markdown file is authoritative for the note body; the app maintains a derived index and stable attachment/anchor metadata. A stale app cache never overwrites a newer vault file.

Changing modes is an explicit migration with preview/backup. There is one body authority per note. The bridge does not become a second independent sync engine that competes with iCloud, Obsidian Sync, Git or another vault synchronizer.

### Binding states (B040)

The contract behind the two modes is a `NoteBinding` per note (`schemas/note-binding.schema.json`, `crates/notes`): the stable note id, the authority (`app` or `vault`), an opaque vault id, the file's relative path (not identity: a rename updates it without making a second note), the vault file's hash as the app last saw it, the hash of the body the app holds, and one of five states. `app_owned` and `vault_owned` are the two modes. `migrating` holds the preview hash and the backup id until the reader commits (the committed body must be the previewed one) or cancels; the authority moves on commit, and migrating back to the app leaves the vault file in place. `conflict` keeps both bodies by hash with its cause (a write whose last-seen hash is stale, or a published copy of an app-owned note edited outside) until the reader chooses the app body, chooses the vault body or supplies a merge; nothing is written meanwhile. `detached` is a vault-owned note whose file is gone: the app copy is read-only until the file is relinked (by reappearance or by hand) or the note is converted to app-owned. Every create, write and delete carries an operation id: replaying it is a no-op and the watcher's report of that operation is suppressed by the id, while a report under our id with a different body is treated as an external change. Under vault authority the app copy follows the file and is never written back unasked; under app authority an outside edit of the published copy is a conflict, not a silent import. Deleting a vault-owned note's file leaves the app copy as the recoverable body. The `note-authority-transitions` conformance fixture runs every transition on every host.

## Direct vault integration

On desktop, an optional local companion or the Obsidian plugin accesses only the chosen vault/folder. In the plugin use Obsidian's Vault API and atomic `process`-style updates, not arbitrary filesystem writes [S26]. Preserve user formatting, YAML frontmatter, comments, unknown fields and links outside the managed region. Store stable note UUIDs separately from paths; rename/move events update indexes without duplicating notes.

An update carries the last-seen file hash/revision. Conflicting changes preserve both bodies and show a merge/choose UI. Creation/deletion is idempotent; watcher feedback loops are suppressed by operation IDs, not by ignoring arbitrary future file events. Never silently delete unmanaged files, `.obsidian` settings or user attachments. Test concurrent external sync, offline edits, renames and case-sensitive/case-insensitive filename collisions.

The Obsidian mobile plugin uses the supported Vault API; Node/Electron dependencies are unavailable on mobile [S27]. The standalone iOS/Android Bible app does not automatically get access to another app's private vault. Native document-provider permissions, Android storage access or user-mediated import/export are tested capabilities. Browser filesystem access also varies by browser. Provide a platform capability matrix and an explicit file exchange or paired personal-companion fallback; no universal direct-vault promise.

## Scripture plugin and genuine block references

The plugin provides a passage picker, edition picker, insert-link, insert-quote, insert-embed and open-in-reader commands. It shares the core reference resolver and edition identifiers, reads approved local packages and works offline. Fetching an external source is opt-in. It checks quotation/redistribution rights before materializing text.

Support both forms:

- **Portable native Obsidian blocks:** materialize permitted verses into edition/revision-specific Markdown chapter notes with stable ASCII block IDs. Ordinary links and embeds work without the plugin after materialization. A proposed example is `[[Scripture/WEB-2020/John-003#^v-016]]` or `![[Scripture/WEB-2020/John-003#^v-016]]`; exact directory naming is finalized with collision tests. A multi-verse passage can reference multiple verse blocks or one deliberately created stable range block. Obsidian does not magically resolve a range of block IDs.
- **Dynamic scripture fences:** a `bible` code-block processor renders a structured passage request containing edition ID, revision, versification and reference. Reading view and Live Preview need separate tested support; Markdown post-processing alone does not supply a Live Preview editor extension. Without the plugin, the readable request stays in the file. 'Materialize' converts it to a portable quotation/block after a rights check.

Obsidian's documented `#^block-id` syntax targets real blocks in notes [S28]. A deep link such as `openbible://...` opens our reader but is not a substitute for a native block reference. Protocol names shown here are proposed until registered/implemented.

Default embeddings pin edition/revision. Changing the default Bible does not rewrite existing quotations. A deliberate refresh previews text/attribution changes and preserves old anchors or creates a new revision-specific note. Every generated scripture note carries edition, source and attribution metadata; generated sections are clearly bounded so personal marginal notes are untouched.

Vault attachment layout separates Markdown notes, portable ink JSON, immutable snapshots and exported PNG/PDF previews. Export paths are sanitized; no scripts, tracking images, active SVG or remote resources run when importing a note. Attachment names use IDs/hashes to avoid collisions. Opening a vault never grants its whole contents to AI.

## Handwriting engine decision

| Candidate | Fit | Decision |
|---|---|---|
| perfect-freehand | MIT pressure-aware stroke geometry; caller supplies rendering [S29] | First spike for a small portable ink layer; not a complete note editor |
| Excalidraw | MIT editor/component with a broader drawing workflow [S30] | Compare for faster editor delivery and optional interoperability; test stylus/RTL/export rather than assume parity |
| Native ink adapters | Apple PencilKit / Android Ink as optional platform capabilities | Evaluate only with interoperable raw-stroke/geometry export and OS-level dependency disclosure |
| tldraw SDK | Current SDK is source-available with production licensing [S31] | Excluded from the default fully open-source dependency set |

The baseline is an open stroke model with an open renderer, not a proprietary note format. A native adapter may improve platform input but cannot be the only way to edit/read saved notes. Preserve pressure, time, tilt where available, brush definition/version, color, opacity, width, eraser/mask operations, stroke IDs and z-order in versioned neutral records. Unsupported sensor data is null, not fabricated. Save native blobs only as optional fidelity caches; export portable geometry plus a flattened preview.

### B039 findings

The spike (docs/evidence/B039.md) settled what the table left open. perfect-freehand 1.2.3 is
ported to `crates/ink` (MIT, attribution in the file, golden vectors from the npm package), so
the open baseline is the core's own outline, rendered by each host as one non-zero fill per
stroke; brushes are versioned ids (`pf-1.2.3/pen-1`, `pf-1.2.3/highlighter-1`) and a stroke
whose brush a build does not know is shown by a labelled fallback, never silently reshaped.
Excalidraw is a complete editor with its own document model (no per-point time, tilt or brush
version; one stored point per frame; React and a 17 MB distribution) and is not the margin-ink
layer; it stays an optional interoperability target. Native adapters export what they expose:
PencilKit gives resampled control points with force, tilt and time offsets but not the raw
touches, its brush geometry or its blob format; the Android Ink API gives the raw input batch
with pressure and tilt. `schemas/ink-document.schema.json` 1.1 (B039, additive per ADR 0003)
carries nullable `altitude` and `azimuth` in radians per sample so tilt is preserved where a
device reports it and null elsewhere. The 25 ms p95 input-to-visible target is still unmeasured on hardware:
B039 measured synthesized and WebDriver-driven input on the Mac mini and the iPad simulator
only, and the palm rule's 20-point contact-radius threshold needs a device to calibrate.

This is handwriting capture, not a promise of handwriting recognition. OCR/recognition is a separate optional derived layer with confidence and explicit consent for any remote processing. Original strokes are never replaced by recognized text.

## Ink interaction requirements

Pen, translucent highlighter, stroke eraser, selection/lasso, move/resize, undo/redo, pan/zoom, finger/stylus modes and palm-rejection testing are required. A drawn highlighter composites once per stroke to avoid darker self-overlaps; independent strokes can accumulate intentionally. Pixel erasing may be deferred, but its absence must be explicit. Gestures separate drawing from scrolling; palm touches must not navigate another reading pane.

Persist during use and after stroke completion; recover all acknowledged strokes after simulated interruption. Record a proposed p95 input-to-visible target ≤25 ms on named supported stylus devices, and measure it; the selected framework cannot be declared adequate from a desktop mouse demo. Store completed stroke geometry so brush-version changes do not silently alter older notes.

## Margin snapshots and coordinates

A `ReadingSnapshot` contains immutable rendered page images or a vector/PDF representation, pixel/logical dimensions, scale, content hashes, pane rectangles, source edition/revisions/references and the workspace recipe. Private-note/chat inclusion flags are explicit. Capture after fonts and content settle; placeholders cannot be certified as captured source text. For long captures split into bounded pages with global limits. All images/fonts are local or rights-cleared.

`InkDocument` owns an immutable coordinate space over that snapshot. Zoom/pan is an affine viewport transform applied equally to background and strokes. Orientation changes never reflow the background; there is no automatic attempt to infer text anchors from stroke location. An annotation can separately link the sheet to live passage IDs. New margins change the document canvas through an explicit saved transform, not by rescaling existing text or strokes invisibly.

Share/export previews include the exact frozen content, notes and attribution. Snapshotting copyrighted material does not grant redistribution rights. Offer portable editable bundle plus PDF/PNG export; a flattened export alone is not a backup. On-device capture remains distinct from remote LLM image upload. Deleting a source package does not silently remove private snapshots or their provenance, subject to the user's chosen retention policy.

Concurrent ink edits use append-only stroke operations and tombstones with conflict-safe merge rules. Two users are not promised live collaborative drawing. Local notes sync includes ink/snapshot attachments encrypted, chunked and resumable; size quotas and attachment availability are visible. Obsidian sync/export is outside the app's E2EE boundary and follows the vault's own protection settings.

## Collection handoff and external writing protection

The canonical handoff workflow is defined in specs/15-reformed-study-and-collections.md. Use a selected app-managed resource folder and stable per-item block IDs. The user's manuscript stays a user-managed note that links to or embeds these resources. Never replace it with a generated collection document. Re-export reconciles by item ID and last-export hash, detects edited or deleted blocks and preserves unmanaged content. Repeated handoff is idempotent; a rejected/conflicting update leaves both versions available.

P1 can use plugin-mediated local package/file exchange to avoid pretending every mobile app can access the same vault directory. Materialized native blocks must function in Reading view and Live Preview without the plugin after creation. Dynamic scripture fences and full app-side vault editing are separate P2 capabilities. Record direct and fallback support per platform instead of advertising untested filesystem parity.
