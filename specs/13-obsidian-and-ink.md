# Personal notes, Obsidian and handwriting

## Ownership and scope

The built-in notes feature remains complete without Obsidian. Obsidian is an optional external application; the integration plugin, interchange formats and our adapters are open source. There is no requirement for Obsidian Sync or an Obsidian account. Vault files remain ordinary Markdown and attachments readable by other tools.

Two explicitly selected note modes:

1. App-managed: local annotation database is authoritative; export/import Markdown and attachments or explicitly publish selected notes to a vault.
2. Vault-backed: a user-selected Markdown file is authoritative for the note body; the app maintains a derived index and stable attachment/anchor metadata. A stale app cache never overwrites a newer vault file.

Changing modes is an explicit migration with preview/backup. There is one body authority per note. The bridge does not become a second independent sync engine that competes with iCloud, Obsidian Sync, Git or another vault synchronizer.

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

This is handwriting capture, not a promise of handwriting recognition. OCR/recognition is a separate optional derived layer with confidence and explicit consent for any remote processing. Original strokes are never replaced by recognized text.

## Ink interaction requirements

Pen, translucent highlighter, stroke eraser, selection/lasso, move/resize, undo/redo, pan/zoom, finger/stylus modes and palm-rejection testing are required. A drawn highlighter composites once per stroke to avoid darker self-overlaps; independent strokes can accumulate intentionally. Pixel erasing may be deferred, but its absence must be explicit. Gestures separate drawing from scrolling; palm touches must not navigate another reading pane.

Persist during use and after stroke completion; recover all acknowledged strokes after simulated interruption. Record a proposed p95 input-to-visible target ≤25 ms on named supported stylus devices, and measure it; the selected framework cannot be declared adequate from a desktop mouse demo. Store completed stroke geometry so brush-version changes do not silently alter older notes.

## Margin snapshots and coordinates

A `ReadingSnapshot` contains immutable rendered page images or a vector/PDF representation, pixel/logical dimensions, scale, content hashes, pane rectangles, source edition/revisions/references and the workspace recipe. Private-note/chat inclusion flags are explicit. Capture after fonts and content settle; placeholders cannot be certified as captured source text. For long captures split into bounded pages with global limits. All images/fonts are local or rights-cleared.

`InkDocument` owns an immutable coordinate space over that snapshot. Zoom/pan is an affine viewport transform applied equally to background and strokes. Orientation changes never reflow the background; there is no automatic attempt to infer text anchors from stroke location. An annotation can separately link the sheet to live passage IDs. New margins change the document canvas through an explicit saved transform, not by rescaling existing text or strokes invisibly.

Share/export previews include the exact frozen content, notes and attribution. Snapshotting copyrighted material does not grant redistribution rights. Offer portable editable bundle plus PDF/PNG export; a flattened export alone is not a backup. On-device capture remains distinct from remote LLM image upload. Deleting a source package does not silently remove private snapshots or their provenance, subject to the user's chosen retention policy.

Concurrent ink edits use append-only stroke operations and tombstones with conflict-safe merge rules. Two users are not promised live collaborative drawing. Local notes sync includes ink/snapshot attachments encrypted, chunked and resumable; size quotas and attachment availability are visible. Obsidian sync/export is outside the app's E2EE boundary and follows the vault's own protection settings.
