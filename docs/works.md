# Works (B016)

A non-Bible work as one pack: Calvin's *Institutes of the Christian Religion* in John Allen's
translation is the first (rights record S75). Built on this machine from the approved artifact on
the NAS, never in the repository: the output is outside it, and the importer refuses an output
inside the repository or the corpora.

```sh
just institutes-pack              # from the core shell; about 2 s plus the first compile
just institutes-pack-verify       # a second reader against the NAS and the print edition's Scripture index; --show prints places
just institutes-pack-bench        # chapter, section, citation, reverse-index and search latency natively
just institutes-pack-bench-wasm   # the same in a Chrome worker (web shell)
```

Inputs, read-only: `/Volumes/office-hub/openlogos/corpora/S75-gutenberg-calvin-institutes/`
(`--corpora` names the corpora root) and `rights/` (`--rights`). Before any text is parsed the
B071 gate checks the record (`approved`, `local_read` and `public_index` allowed), that the
source's `MANIFEST.sha256` hashes to the record's `artifact_sha256` and equals
`rights/S75/manifest.sha256`, and every listed file's hash.

Output, default `~/.cache/openlogos/works` (`OPENLOGOS_WORKS` or `--out`); the pack is renamed
into place only when the whole build passed and every section read back equal:

```
institutes-allen.sqlite   the work pack
institutes-allen.json     counts per book and chapter, every anomaly of the source, what was
                          left out, citation counts and unresolved samples by class, sizes
```

## The pack

One B068 corpus pack (schema `b068-1`, unchanged) with `work = institutes` and
`locator_scheme = institutes` in its manifest, so the engine, the device catalog, the link index
and the edition search read it like any pack, plus what reading a work needs
(`crates/retrieval-sqlite/src/work_pack.rs`): manifest keys `work_format = openlogos-work-1`,
`work_info` and `work_structure`; tables `work_section`, `work_citation` and `cite_span`. The text
pack of specs/23 §4 is keyed by Bible book, chapter and verse; a work's places are locators
(`3.2.7`, `dedication`), its sections are several paragraphs long and carry notes, so it does
not fit, and specs/19 already gives a work exactly this shape.

Places: `title.1`, `advertisement`, `translator-preface`, `author-preface`, `dedication`,
`syllabus`, `title.2` (front matter); `1.argument` … `4.argument`; chapters `1.1` … `4.20`;
sections `1.1.1` … `4.20.32`, the B068 grammar's canonical locators (`Inst. 3.2.7`,
`Institutes III.ii.7` and `institutes:3.2.7` all read as `3.2.7`).

## Hosts

- Natively: `WorkHandle.open(path)` reads the catalog entry, the structure, a chapter, a section,
  a section's citations and `cited_by(passage, scheme)`, the sections that cite a passage (what a
  verse shows as "cited in Calvin"). On the web: `new WorkHandle(bytes)` with the same calls.
- The palette: register the edition's scheme with `set_resource_schemes`
  (`institutes-allen` → `institutes`) and a short title with `set_resource_titles`; then
  `Inst. 3.2.7` is a Passage row and text hits in the work are Text rows, both opening the place
  with `tab.open_edition`. Text search: `ShellHandle.use_search_packs([...])` natively with the
  editions' `search/*.sqlite` and the work pack; `useEditionSearch([...])` on the web with their
  bytes.
- Show `WorkInfo.attribution`.

What the text is and why, the anomalies of the source and the citation results:
docs/evidence/B016.md.
