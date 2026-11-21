//! B071: the small search packs the Swift hosts test `use_edition_search` with
//! (fixtures/search/). They hold made-up sentences, no corpus byte. This test builds them again
//! and says when the committed bytes no longer match the pack format;
//! `OPENLOGOS_UPDATE_SEARCH_FIXTURE=1 cargo test -p openlogos-preview-pack --test search_fixture`
//! writes them.

use std::path::{Path, PathBuf};

use openlogos_retrieval::VerseId;
use openlogos_retrieval_sqlite::{PackBuilder, PackManifest};

/// Edition, scheme, and verses (book number, chapter, verse, text). Not Scripture.
type Edition = (
    &'static str,
    &'static str,
    &'static [(u8, u16, u16, &'static str)],
);
const EDITIONS: &[Edition] = &[
    (
        "bsb",
        "eng",
        &[
            (44, 3, 16, "A made-up sentence about a zebra, for tests."),
            (42, 9, 43, "Another made-up sentence with a quagga in it."),
        ],
    ),
    (
        "sblgnt",
        "org",
        &[(44, 3, 16, "Πρότασις πεπλασμένη περὶ ζέβρας.")],
    ),
];

fn fixture_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/search")
}

fn build(edition: &Edition, path: &Path) {
    let manifest = PackManifest {
        edition: edition.0.to_owned(),
        scheme: edition.1.to_owned(),
        source: "made up for the host tests (crates/preview-pack/tests/search_fixture.rs)"
            .to_owned(),
        license: "CC0-1.0".to_owned(),
        attribution: "Test fixture, not Scripture.".to_owned(),
        work: None,
        locator_scheme: None,
    };
    let mut builder = PackBuilder::create(path, &manifest, &[]).unwrap();
    for (book, chapter, verse, text) in edition.2 {
        builder
            .add_verse(VerseId::new(*book, *chapter, *verse), text)
            .unwrap();
    }
    builder.finish().unwrap();
}

#[test]
fn the_committed_search_fixtures_are_what_the_builder_writes() {
    let scratch =
        std::env::temp_dir().join(format!("openlogos-search-fixture-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();
    for edition in EDITIONS {
        let name = format!("{}.sqlite", edition.0);
        let built = scratch.join(&name);
        build(edition, &built);
        let committed = fixture_directory().join(&name);
        if std::env::var_os("OPENLOGOS_UPDATE_SEARCH_FIXTURE").is_some() {
            std::fs::copy(&built, &committed).unwrap();
        }
        assert!(
            std::fs::read(&built).unwrap() == std::fs::read(&committed).unwrap(),
            "{} is stale: see this file's header",
            committed.display()
        );
    }
    std::fs::remove_dir_all(&scratch).unwrap();
}
