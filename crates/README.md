# crates

The Rust workspace: one core shared by every platform (docs/adr/0001-rust-core-native-ui.md).
Each crate's `lib.rs` opens with a paragraph stating its responsibility; specs/06-nix-operations.md
lists them together.

| Crate | Responsibility |
|---|---|
| `core` | references, versification mappings, anchors, typed claims; builds for wasm |
| `text` | render-ready attributed runs, normalization, offset adapters; builds for wasm |
| `search` | corpus query engine and prose full text (specs/19) |
| `retrieval` | vector and hybrid retrieval; the ADR 0002 storage traits |
| `package` | signed content-addressed packs, hashes, index layers, link index |
| `collections` | study collections, export receipts, Markdown/JSON export |
| `workspaces` | pane trees, link groups, layout profiles |
| `ink` | portable stroke geometry and snapshot contracts |
| `notes` | app/vault note authority and merge contracts |
| `sync` | encrypted operation log and conflict resolution |
| `ffi` | UniFFI and wasm-bindgen surfaces, versioned API (B061, B064) |
| `conformance` | cross-platform fixtures and expected results |

## Core API (B061)

`openlogos_core::API_VERSION` names the API the bindings are generated against, separately from
the crate version; its doc comment states the bump rule. The modules so far:

| Module | Types | Functions |
|---|---|---|
| `core::reference` | `BookId`, `Reference`, `ReferenceRange`, `ReferenceSet`, `ReviewFlag`, `AliasTable`, `BookAliases`, `ReferenceError` | `parse(input, &AliasTable)`, `AliasTable::english()`, `to_osis()`/`to_human(&table)` on each |
| `core::versification` | `SchemeId`, `VerseMapKind`, `VerseMapEntry`, `VerseMap`, `MappingStatus`, `MappingResult`, `ReferenceMapping`, `SegmentTable`, `SegmentSpan`, `SegmentResolution` (B006) | `VerseMap::map(&reference, &from, &to)`, pivoting through `org`; `VerseMap::map_many(&refs, &from, &to)`; `SegmentTable::resolve(&reference)` |
| `core::anchor` | `TokenId`, `ScalarRange`, `TextQuoteSelector`, `Anchor`, `TokenSpan`, `Revision`, `Segment`, `ReanchorResult`, `AnchorError` | `TokenId::parse`/`Display`, `TextQuoteSelector::from_text`, `split_selection(...)`, `reanchor(&anchor, &old, &new)`, `reanchor_across(&anchor, &old, &new, Option<&IdLineage>)` (B006) |
| `core::identity` (B006) | `IdLineage`, `LineageEntry`, `LineageKind`, `LineageResult`, `LineageError`, `AlignmentSet`, `AlignmentRecord`, `AlignmentStatus`, `OriginKind`, `AlignmentError`, `MintError` | `mint_verse(edition, &reference, &morphemes)`, `suffix_for(index)`, `IdLineage::validate`/`map_tokens`, `AlignmentSet::validate`/`check_tokenization`/`forward`/`reverse`/`renderings`/`is_explicit_null` |
| `core::versioning` (B006) | `SchemaVersion`, `Compatibility`, `ApiVersion`, `VersionSyntax` | `SchemaVersion::parse`/`compatibility`, `ApiVersion::parse`/`current`, `api_compatible(core, host)` |
| `collections` (B006) | `StudyCollection`, `CollectionItem`, `ItemBody`, `SourceProvenance`, `RightsSnapshot`, `Locator`, `VaultExportReceipt`, `ReceiptEntry`, `Disposition`, `CollectionError`, `ReceiptError` | `StudyCollection::validate`, `CollectionItem::block_id`/`quotation_permitted`, `VaultExportReceipt::validate`/`validate_against`/`stale_items`/`orphaned_entries` |
| `text::offsets` | `Utf16Range`, `GraphemeRange`, `OffsetUnit`, `OffsetError` | `scalar_to_utf16`, `utf16_to_scalar`, `scalar_to_grapheme`, `grapheme_to_scalar`, `snap_to_graphemes`, `slice_scalars`, `scalar_len`/`utf16_len`/`grapheme_len` |
| `conformance` | `Fixture` and its parts (text, versification, references, and from B006 lineage, alignment, reanchor), `ConformanceReport`, `CaseResult` | `run_all()`, `fixtures()`, `fixtures_json()`, `FIXTURE_FILES` |
| `workspaces` (B062) | `Workspace`, `Pane`, `ScrollAnchor`, `LayoutProfile`, `LayoutProfiles`, `DeviceClass`, `TextSize`, `DerivedLayout`, `Column`, `NotesDockPlacement`, `ProfileSource`, `LayoutError` | `derive_profile(&workspace, class, width_pt, text_size)`, `with_profile(&workspace, class, profile)`, `check_profile(...)`, `DeviceClass::for_width_pt` |

Everything public is plain data (owned `String`s and `Vec`s, no lifetimes or generics),
`serde`-serializable, and each module has one `thiserror` enum, so the UniFFI and wasm-bindgen
surfaces in `ffi` can mirror it without adapters. Fixtures are JSON files under
`conformance/fixtures/` compiled in with `include_str!`; a host calls `run_all()` and checks
`ConformanceReport::all_passed()`.

## Build and test

Inside `nix develop .#core` (or after `direnv allow`):

```sh
just test            # cargo nextest + doc tests, whole workspace
just lint            # rustfmt, clippy -D warnings, nixfmt, shellcheck, actionlint, blueprint
just check-wasm      # cargo check --target wasm32-unknown-unknown for core, text, collections, conformance and ffi
just ci              # lint + test + check-wasm, as CI runs them in the shell
just check           # nix flake check: the same checks as hermetic derivations
cargo test -p openlogos-core          # one crate
cargo doc --workspace --no-deps --open
```

Toolchain: Rust pinned in `flake.nix` (`rustVersion`) and mirrored by `rust-version` in
`Cargo.toml`. Cross targets: `wasm32-unknown-unknown` and `aarch64-linux-android` everywhere;
`aarch64-apple-darwin`, `aarch64-apple-ios`, `aarch64-apple-ios-sim` in the Darwin shells.
Workspace lints live in the root `Cargo.toml`; release profile is tuned for size.
