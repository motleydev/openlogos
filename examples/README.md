# Synthetic fixtures

The records here demonstrate shapes, not cleared sources, real alignments or production signatures. The package manifest is deliberately unsigned and `fixture_only`; a production installer must reject it. The rights record remains unknown. A passing schema test does not approve content or distribution.

The manifest signature is intended to cover canonical JSON bytes with the signature field omitted (implement and test one canonicalization standard, e.g. RFC 8785, before production). All payload paths are relative, non-traversing paths. The archive format, canonicalization, chunking and trust protocol still require the implementation/security tasks; this example is not a cryptographic implementation.


Revision 2 adds a synthetic three-pane workspace, a fixed-coordinate SVG background, one ink stroke and a local research thread. The SVG is only a test fixture, not a UI mockup. No real provider call, private note or biblical claim is represented. Schema validation rejects additional credential fields. Production validation must also enforce source authorization, quote fidelity, immutable snapshot revisions and platform limits.

B062 moves `workspace.json` to schema 1.1: five panes in two link groups as a Mac would save them (three translations in group 1, a Greek text and its lexicon in group 2), each pane with a scroll anchor, and `layout_profiles` for the wide class (all five visible, with widths) and the regular class (three visible, two tabs). Compact and ultra-wide are absent on purpose so the core derives them (specs/18-platform-capabilities.md, "Workspace layout profiles"). `archive/workspace-1.0.json` is the previous three-pane example, kept so the validator exercises the N reads N-1 rule and the 1.0 rejections. Editions, segments and the lexicon are fixture ids, not resources.
