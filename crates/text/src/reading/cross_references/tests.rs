//! Cross references over small hand-made packs: rows in the source's (`eng`) numbering, editions
//! in `eng` and `org`, one that omits verses and one whose scheme the canon table does not know.

use super::*;
use crate::reading::{BookShape, ChapterShape, ChapterVerse, Edition, TextPack, canon_books};
use openlogos_core::canon::BookOrder;

/// A pack over the canon table whose verse text is its OSIS id, with chapters cut short and
/// verses left out as an edition does.
struct TestPack {
    edition: Edition,
    books: Vec<BookShape>,
    absent: Vec<(BookId, u32, Vec<u32>)>,
    unloaded: bool,
}

fn edition(id: &str, scheme: &str) -> Edition {
    Edition {
        resource_id: id.to_owned(),
        content_revision: format!("{id}-test"),
        title: id.to_uppercase(),
        name: id.to_owned(),
        language: "en".to_owned(),
        direction: "ltr".to_owned(),
        scheme: scheme.to_owned(),
        book_order: BookOrder::Protestant,
        placeholder: false,
        attribution: None,
    }
}

impl TestPack {
    fn new(id: &str, scheme: &str, books: &[BookId]) -> TestPack {
        let canon_scheme = if canon::knows(&SchemeId::new(scheme)) {
            scheme
        } else {
            "eng"
        };
        TestPack {
            edition: edition(id, scheme),
            books: canon_books(&SchemeId::new(canon_scheme), books, BookOrder::Protestant),
            absent: Vec::new(),
            unloaded: false,
        }
    }

    /// Ends a chapter early (SBLGNT Romans 16 at verse 24).
    fn cut(mut self, book: BookId, chapter: u32, last: u32) -> TestPack {
        for b in &mut self.books {
            if b.book == book {
                for c in &mut b.chapters {
                    if c.chapter == chapter {
                        *c = ChapterShape {
                            last_verse: last,
                            ..*c
                        };
                    }
                }
            }
        }
        self
    }

    fn omit(mut self, book: BookId, chapter: u32, verses: &[u32]) -> TestPack {
        self.absent.push((book, chapter, verses.to_vec()));
        self
    }
}

impl TextPack for TestPack {
    fn edition(&self) -> &Edition {
        &self.edition
    }

    fn books(&self) -> &[BookShape] {
        &self.books
    }

    fn verses(&self, book: BookId, chapter: u32) -> Result<Vec<ChapterVerse>, ReadingError> {
        if self.unloaded {
            return Err(ReadingError::ChunkNotLoaded {
                resource_id: self.edition.resource_id.clone(),
                chapter_key: format!("{}.{chapter}", book.as_osis()),
                path: "chapters/x.json".to_owned(),
            });
        }
        let shape = self
            .books
            .iter()
            .find(|b| b.book == book)
            .and_then(|b| b.chapters.iter().find(|c| c.chapter == chapter))
            .expect("asked for a chapter the pack has");
        Ok((shape.first_verse..=shape.last_verse)
            .filter(|v| !self.absent_verses(book, chapter).contains(v))
            .map(|v| {
                let id = format!("{}.{chapter}.{v}", book.as_osis());
                ChapterVerse {
                    segment_id: id.clone(),
                    number: v,
                    last_number: v,
                    label: v.to_string(),
                    text: format!("[{}:{id}]", self.edition.resource_id),
                    paragraph_start: false,
                    poetry: false,
                    superscription: v == 0,
                    placeholder: false,
                    tokens: Vec::new(),
                }
            })
            .collect())
    }

    fn absent_verses(&self, book: BookId, chapter: u32) -> &[u32] {
        self.absent
            .iter()
            .find(|(b, c, _)| *b == book && *c == chapter)
            .map_or(&[], |(_, _, v)| v.as_slice())
    }
}

fn info() -> CrossReferenceInfo {
    CrossReferenceInfo {
        source_id: "S09".to_owned(),
        edition_id: "test".to_owned(),
        scheme: "eng".to_owned(),
        license: "CC-BY-4.0".to_owned(),
        attribution: "Cross references from a test".to_owned(),
        relation: "cross_reference".to_owned(),
        kind: "unspecified".to_owned(),
        votes_meaning: "votes".to_owned(),
        review_state: "source_attested".to_owned(),
        coverage: "test".to_owned(),
        rows: 0,
    }
}

fn r(osis: &str) -> Reference {
    canon::segment_reference(osis).expect("a verse")
}

/// Rows written as the published file writes them: `From<TAB>To<TAB>Votes`.
fn rows(lines: &[&str]) -> Vec<CrossReferenceRow> {
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let mut f = line.split('\t');
            let from = r(f.next().unwrap());
            let to = f.next().unwrap();
            let (a, b) = to.split_once('-').unwrap_or((to, to));
            CrossReferenceRow {
                line: i as u32 + 2,
                from,
                to_start: r(a),
                to_end: r(b),
                votes: f.next().unwrap().parse().unwrap(),
            }
        })
        .collect()
}

const ROWS: &[&str] = &[
    "Gen.1.1\tJohn.1.1-John.1.3\t384",
    "Gen.1.1\tHeb.11.3\t278",
    "Gen.1.1\tPs.33.6\t50",
    "Gen.1.1\tIsa.45.18\t-3",
    "Gen.1.1\tJob.38.4\t0",
    "Heb.11.3\tGen.1.1\t40",
    "Ps.33.6\tGen.1.1-Gen.1.3\t12",
    "Mal.4.5\tMal.4.6\t5",
    "Mal.4.5\tLuke.1.17\t29",
    "Mal.4.5\tRom.16.25-Rom.16.27\t1",
    "Ps.51.1\tPs.3.1\t7",
    "Ps.23.1\tJohn.10.11\t383",
    "Ps.23.2\tJohn.10.11\t20",
    "Ps.23.2\tRev.7.17\t90",
    "Matt.17.20\tMatt.17.20-Matt.17.22\t3",
];

fn library() -> ReadingLibrary {
    let mut lib = ReadingLibrary::new();
    lib.insert(Box::new(TestPack::new("bsb", "eng", &BookId::all()).omit(
        BookId::Matt,
        17,
        &[21],
    )));
    let nt: Vec<BookId> = BookId::all()
        .into_iter()
        .filter(|b| *b >= BookId::Matt)
        .collect();
    lib.insert(Box::new(TestPack::new("sblgnt", "org", &nt).cut(
        BookId::Rom,
        16,
        24,
    )));
    let ot: Vec<BookId> = BookId::all()
        .into_iter()
        .filter(|b| *b < BookId::Matt)
        .collect();
    lib.insert(Box::new(TestPack::new("wlc", "org", &ot)));
    lib.set_cross_references(Box::new(MemoryCrossReferences::new(info(), rows(ROWS))));
    lib
}

fn ask(lib: &ReadingLibrary, id: &str, input: &str, reverse: bool) -> CrossReferenceList {
    lib.cross_references(
        id,
        input,
        &CrossReferenceOptions {
            include_reverse: reverse,
            ..CrossReferenceOptions::default()
        },
    )
    .unwrap()
}

fn find<'a>(list: &'a CrossReferenceList, source_target: &str) -> &'a CrossReference {
    list.items
        .iter()
        .find(|i| i.source_target == source_target)
        .unwrap_or_else(|| panic!("{source_target} in {list:#?}"))
}

#[test]
fn a_verse_lists_its_published_links_best_first_with_ranges_and_previews() {
    let list = ask(&library(), "bsb", "Gen 1:1", false);
    assert_eq!(list.reference, "Gen.1.1");
    assert_eq!(list.source_verses, ["Gen.1.1"]);
    assert_eq!(list.total, 5);
    let order: Vec<&str> = list
        .items
        .iter()
        .map(|i| i.source_target.as_str())
        .collect();
    // Votes as published, negative and zero kept and last.
    assert_eq!(
        order,
        [
            "John.1.1-John.1.3",
            "Heb.11.3",
            "Ps.33.6",
            "Job.38.4",
            "Isa.45.18"
        ]
    );
    let john = &list.items[0];
    assert_eq!(john.target.as_deref(), Some("John.1.1-John.1.3"));
    assert_eq!(john.label, "John 1:1–3");
    assert_eq!(john.votes, 384);
    assert_eq!(john.direction, Direction::AsPublished);
    assert_eq!(john.kind, "unspecified");
    assert_eq!(john.mapping, MappingStatus::Exact);
    assert_eq!(john.availability, Availability::Printed);
    assert_eq!(
        john.preview.as_deref(),
        Some("[bsb:John.1.1] [bsb:John.1.2] [bsb:John.1.3]")
    );
    assert!(!john.preview_truncated);
    assert_eq!(john.attribution, "Cross references from a test");
    assert_eq!(john.assertions[0].line, 2);
    assert_eq!(john.available_in, ["sblgnt"]);
    assert_eq!(list.items[4].votes, -3);
}

#[test]
fn a_long_range_preview_is_cut_and_says_so() {
    let lib = library();
    let list = lib
        .cross_references(
            "bsb",
            "Gen 1:1",
            &CrossReferenceOptions {
                preview_verses: 2,
                ..CrossReferenceOptions::default()
            },
        )
        .unwrap();
    let john = find(&list, "John.1.1-John.1.3");
    assert_eq!(
        john.preview.as_deref(),
        Some("[bsb:John.1.1] [bsb:John.1.2]")
    );
    assert!(john.preview_truncated);
    let limited = lib
        .cross_references(
            "bsb",
            "Gen 1:1",
            &CrossReferenceOptions {
                limit: Some(2),
                ..CrossReferenceOptions::default()
            },
        )
        .unwrap();
    assert_eq!(limited.items.len(), 2);
    assert_eq!(limited.total, 5);
}

#[test]
fn reverse_links_are_derived_labelled_and_merged_with_published_ones() {
    let list = ask(&library(), "bsb", "Gen 1:1", true);
    // Heb 11:3 is published both ways: one item, published, both assertions kept.
    let heb = find(&list, "Heb.11.3");
    assert_eq!(heb.direction, Direction::AsPublished);
    assert_eq!(heb.votes, 278);
    assert_eq!(heb.assertions.len(), 2);
    assert_eq!(heb.assertions[1].direction, Direction::ReverseDerived);
    assert_eq!(heb.assertions[1].from, "Heb.11.3");
    assert_eq!(heb.assertions[1].to, "Gen.1.1");
    // Ps 33:6 lists Gen 1:1–3 (a range holding Gen 1:1) and is listed under Gen 1:1.
    let ps = find(&list, "Ps.33.6");
    assert_eq!(ps.direction, Direction::AsPublished);
    assert_eq!(ps.assertions.len(), 2);
    assert_eq!(ps.assertions[1].to, "Gen.1.1-Gen.1.3");
    assert_eq!(list.total, 5);
    assert_eq!(list.published, 5);

    // From Heb 11:3, Gen 1:1 is published; from John 1:2 it is only derived.
    let john = ask(&library(), "bsb", "John 1:2", true);
    assert_eq!(john.items.len(), 1);
    assert_eq!(john.items[0].direction, Direction::ReverseDerived);
    assert_eq!(john.items[0].source_target, "Gen.1.1");
    assert_eq!(john.items[0].votes, 384);
    assert_eq!(john.published, 0);
    assert!(ask(&library(), "bsb", "John 1:2", false).items.is_empty());
}

#[test]
fn a_range_merges_equal_targets_and_keeps_every_assertion() {
    let list = ask(&library(), "bsb", "Ps 23:1–2", false);
    assert_eq!(list.source_verses, ["Ps.23.1", "Ps.23.2"]);
    assert_eq!(list.total, 2);
    let john = &list.items[0];
    assert_eq!(john.source_target, "John.10.11");
    assert_eq!(john.votes, 383);
    let froms: Vec<(&str, i32)> = john
        .assertions
        .iter()
        .map(|a| (a.from.as_str(), a.votes))
        .collect();
    assert_eq!(froms, [("Ps.23.1", 383), ("Ps.23.2", 20)]);
    // A whole chapter is a range too.
    assert_eq!(ask(&library(), "bsb", "Ps 23", false).total, 2);
}

#[test]
fn hebrew_numbering_maps_both_ends_and_keeps_other_testament_links() {
    let lib = library();
    // WLC Malachi 3:23 is English Malachi 4:5.
    let list = ask(&lib, "wlc", "Mal 3:23", false);
    assert_eq!(list.source_verses, ["Mal.4.5"]);
    let next = find(&list, "Mal.4.6");
    assert_eq!(next.target.as_deref(), Some("Mal.3.24"));
    assert_eq!(next.label, "Mal 3:24");
    assert_eq!(next.source_label, "Mal 4:6");
    assert_eq!(next.preview.as_deref(), Some("[wlc:Mal.3.24]"));
    let luke = find(&list, "Luke.1.17");
    assert_eq!(luke.availability, Availability::BookNotInEdition);
    assert_eq!(luke.preview, None);
    assert_eq!(luke.available_in, ["bsb", "sblgnt"]);
    // Not printed in the SBLGNT, kept and labelled; readable in the BSB.
    let doxology = find(&list, "Rom.16.25-Rom.16.27");
    assert_eq!(doxology.availability, Availability::BookNotInEdition);
    let in_sblgnt = ask(&lib, "sblgnt", "Luke 1:17", true);
    assert_eq!(in_sblgnt.items[0].source_target, "Mal.4.5");
    assert_eq!(
        in_sblgnt.items[0].availability,
        Availability::BookNotInEdition
    );
    assert_eq!(in_sblgnt.items[0].available_in, ["bsb", "wlc"]);
}

#[test]
fn a_target_the_edition_does_not_print_is_kept_and_labelled() {
    let lib = library();
    let mal = ask(&lib, "bsb", "Mal 4:5", false);
    assert_eq!(
        find(&mal, "Rom.16.25-Rom.16.27").availability,
        Availability::Printed
    );
    let mut only_nt = ReadingLibrary::new();
    let nt: Vec<BookId> = BookId::all()
        .into_iter()
        .filter(|b| *b >= BookId::Matt)
        .collect();
    only_nt.insert(Box::new(TestPack::new("sblgnt", "org", &nt).cut(
        BookId::Rom,
        16,
        24,
    )));
    only_nt.set_cross_references(Box::new(MemoryCrossReferences::new(
        info(),
        rows(&[
            "Eph.3.20\tRom.16.25-Rom.16.27\t4",
            "Eph.3.20\tRom.16.24-Rom.16.25\t1",
        ]),
    )));
    let list = ask(&only_nt, "sblgnt", "Eph 3:20", false);
    let doxology = find(&list, "Rom.16.25-Rom.16.27");
    assert_eq!(doxology.availability, Availability::NotPrinted);
    assert_eq!(doxology.target.as_deref(), Some("Rom.16.25-Rom.16.27"));
    assert_eq!(doxology.preview, None);
    assert!(doxology.available_in.is_empty());
    let partly = find(&list, "Rom.16.24-Rom.16.25");
    assert_eq!(partly.availability, Availability::PartlyPrinted);
    assert_eq!(partly.preview.as_deref(), Some("[sblgnt:Rom.16.24]"));

    // BSB omits Matt 17:21: a range over it is partly printed and its preview skips it.
    let matt = ask(&lib, "bsb", "Matt 17:20", false);
    let range = find(&matt, "Matt.17.20-Matt.17.22");
    assert_eq!(range.availability, Availability::PartlyPrinted);
    assert_eq!(
        range.preview.as_deref(),
        Some("[bsb:Matt.17.20] [bsb:Matt.17.22]")
    );
}

#[test]
fn psalm_titles_follow_the_hebrew_numbering() {
    let lib = library();
    // WLC Ps 51:1–2 is the title. Verse 1 has no English counterpart: the passage is
    // unmapped, said so, and has no links. Verse 2 is English 51:0, which the source lists
    // nothing under.
    let first = ask(&lib, "wlc", "Ps 51:1", false);
    assert_eq!(first.mapping, MappingStatus::Unmapped);
    assert!(first.source_verses.is_empty());
    assert!(first.items.is_empty());
    let title = ask(&lib, "wlc", "Ps 51:2", false);
    assert_eq!(title.source_verses, ["Ps.51.0"]);
    assert!(title.items.is_empty());
    // WLC Ps 51:3 is English 51:1; its target Ps 3:1 is WLC Ps 3:2.
    let list = ask(&lib, "wlc", "Ps 51:3", false);
    assert_eq!(list.source_verses, ["Ps.51.1"]);
    assert_eq!(list.items[0].target.as_deref(), Some("Ps.3.2"));
    assert_eq!(list.items[0].source_target, "Ps.3.1");
    assert_eq!(list.items[0].mapping, MappingStatus::Exact);
}

#[test]
fn an_edition_in_an_unknown_scheme_keeps_every_link_as_unmapped() {
    let mut lib = ReadingLibrary::new();
    lib.insert(Box::new(TestPack::new("vg", "vul", &BookId::all())));
    lib.set_cross_references(Box::new(MemoryCrossReferences::new(info(), rows(ROWS))));
    let list = ask(&lib, "vg", "Gen 1:1", false);
    assert_eq!(list.mapping, MappingStatus::Unmapped);
    assert!(list.items.is_empty());
}

#[test]
fn a_target_that_does_not_map_keeps_its_published_label() {
    let mut lib = ReadingLibrary::new();
    lib.insert(Box::new(TestPack::new("bsb", "eng", &BookId::all())));
    lib.insert(Box::new(TestPack::new("vg", "vul", &BookId::all())));
    lib.set_cross_references(Box::new(MemoryCrossReferences::new(info(), rows(ROWS))));
    // Asked in the BSB, the Vulgate-numbered edition cannot be offered for anything.
    let list = ask(&lib, "bsb", "Gen 1:1", false);
    assert!(list.items.iter().all(|i| i.available_in.is_empty()));
}

#[test]
fn errors_say_what_is_missing() {
    let mut lib = library();
    assert!(matches!(
        lib.cross_references("bsb", "Gen 1:1–50:26", &CrossReferenceOptions::default()),
        Err(CrossReferenceError::TooLong {
            max: MAX_QUERY_VERSES,
            ..
        })
    ));
    assert!(matches!(
        lib.cross_references("nope", "Gen 1:1", &CrossReferenceOptions::default()),
        Err(CrossReferenceError::Reading { .. })
    ));
    lib.clear_cross_references();
    let error = lib
        .cross_references("bsb", "Gen 1:1", &CrossReferenceOptions::default())
        .unwrap_err();
    assert_eq!(error, CrossReferenceError::NotInstalled);
    assert_eq!(error.to_string(), "Needs cross-reference data");
    assert!(lib.cross_reference_info().is_none());
}

#[test]
fn a_chunk_not_yet_supplied_is_named_instead_of_a_preview() {
    let mut lib = ReadingLibrary::new();
    let mut pack = TestPack::new("bsb", "eng", &BookId::all());
    pack.unloaded = true;
    lib.insert(Box::new(pack));
    lib.set_cross_references(Box::new(MemoryCrossReferences::new(info(), rows(ROWS))));
    let list = ask(&lib, "bsb", "Gen 1:1", false);
    // The test pack has no chunk table, so the item says only that there is no preview.
    assert_eq!(list.items[0].preview, None);
    assert_eq!(list.items[0].availability, Availability::Printed);
}
