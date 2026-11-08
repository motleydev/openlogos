//! Works (B016) on the native surface: `WorkHandle` over one work pack
//! (`openlogos_retrieval_sqlite::work_pack`, the `retrieval` feature), with records mirroring
//! the core's field for field. Additive; nothing existing is renamed.
//!
//! A host opens `~/.cache/openlogos/works/institutes-allen.sqlite` (`just institutes-pack`)
//! with `WorkHandle.open`, registers the edition's locator scheme with
//! `ShellHandle.set_resource_schemes` (`institutes-allen` → `institutes`) so the palette offers
//! `Inst. 3.2.7`, and hands the same file to `ShellHandle.use_search_packs` beside the editions'
//! search packs for the Text section. `cited_by` is the reverse link index: what a verse shows
//! as "cited in Calvin".

use std::sync::{Arc, Mutex};

use openlogos_retrieval_sqlite::work_pack::{
    self as core, ChapterKind, CitationStatus, DivisionKind, NoteKind, ParagraphStyle, WorkPack,
};

/// Why a work call failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum WorkCallError {
    /// The core refused the call.
    #[error("{reason}")]
    Refused {
        /// The core's tag: `not_a_work_pack`, `not_found`, `passage`, `storage`.
        kind: String,
        /// Plain words.
        reason: String,
    },
}

impl From<core::WorkPackError> for WorkCallError {
    fn from(error: core::WorkPackError) -> Self {
        WorkCallError::Refused {
            kind: serde_json::to_value(&error)
                .ok()
                .and_then(|v| v.get("kind").and_then(|k| k.as_str()).map(str::to_owned))
                .unwrap_or_default(),
            reason: error.to_string(),
        }
    }
}

/// One printed volume (`WorkVolume`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WorkVolumeRecord {
    /// `1`, `2`.
    pub volume: u32,
    /// The record's title.
    pub title: String,
    /// `eBook #45001`.
    pub identifier: String,
    /// Release date as the record gives it.
    pub release_date: String,
    /// Last update as the record gives it.
    pub last_update: String,
    /// Who produced the digital text; empty when the record names no one.
    pub credits: String,
}

/// The catalog entry of a work edition (`WorkInfo` with its bibliography flattened).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WorkCatalogEntry {
    /// `institutes-allen`.
    pub resource_id: String,
    /// `institutes`.
    pub work: String,
    /// `institutes`.
    pub locator_scheme: String,
    /// Changes with any byte of the text or structure.
    pub content_revision: String,
    /// Full title.
    pub title: String,
    /// For tabs and palette rows.
    pub short_title: String,
    /// BCP 47.
    pub language: String,
    /// The rights record.
    pub source_id: String,
    /// Licence identifier.
    pub license: String,
    /// What a reader must be shown.
    pub attribution: String,
    /// Where the text came from.
    pub source: String,
    /// Author as the record names them.
    pub author: String,
    /// Translator as the record names them.
    pub translator: String,
    /// Edition statement.
    pub edition: String,
    /// Place as printed.
    pub place: String,
    /// Publisher as printed.
    pub publisher: String,
    /// Date as the title page prints it.
    pub date_printed: String,
    /// Subjects.
    pub subjects: Vec<String>,
    /// Volumes.
    pub volumes: Vec<WorkVolumeRecord>,
    /// What to know about these fields.
    pub bibliography_note: String,
    /// Sections and units.
    pub sections: u32,
    /// Notes.
    pub notes: u32,
    /// Citations found.
    pub citations: u32,
    /// Of which resolved.
    pub resolved: u32,
}

impl From<core::WorkInfo> for WorkCatalogEntry {
    fn from(i: core::WorkInfo) -> Self {
        let b = i.bibliography;
        WorkCatalogEntry {
            resource_id: i.resource_id,
            work: i.work,
            locator_scheme: i.locator_scheme,
            content_revision: i.content_revision,
            title: i.title,
            short_title: i.short_title,
            language: i.language,
            source_id: i.source_id,
            license: i.license,
            attribution: i.attribution,
            source: i.source,
            author: b.author,
            translator: b.translator,
            edition: b.edition,
            place: b.place,
            publisher: b.publisher,
            date_printed: b.date_printed,
            subjects: b.subjects,
            volumes: b
                .volumes
                .into_iter()
                .map(|v| WorkVolumeRecord {
                    volume: v.volume,
                    title: v.title,
                    identifier: v.identifier,
                    release_date: v.release_date,
                    last_update: v.last_update,
                    credits: v.credits,
                })
                .collect(),
            bibliography_note: b.note,
            sections: i.sections,
            notes: i.notes,
            citations: i.citations,
            resolved: i.resolved,
        }
    }
}

/// `DivisionKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum WorkDivisionKind {
    /// Front matter.
    Front,
    /// A numbered book.
    Book,
}

/// `ChapterKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum WorkChapterKind {
    /// One front-matter piece.
    Front,
    /// A book's argument.
    Argument,
    /// A numbered chapter.
    Chapter,
}

/// A chapter-level unit in the structure (`WorkChapterInfo`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WorkChapterOutline {
    /// `3.2`, `1.argument`, `dedication`.
    pub locator: String,
    /// Its kind.
    pub kind: WorkChapterKind,
    /// Label as printed.
    pub label: Option<String>,
    /// Title as printed.
    pub title: Option<String>,
    /// Its sections' locators.
    pub sections: Vec<String>,
}

impl From<core::WorkChapterInfo> for WorkChapterOutline {
    fn from(c: core::WorkChapterInfo) -> Self {
        WorkChapterOutline {
            locator: c.locator,
            kind: match c.kind {
                ChapterKind::Front => WorkChapterKind::Front,
                ChapterKind::Argument => WorkChapterKind::Argument,
                ChapterKind::Chapter => WorkChapterKind::Chapter,
            },
            label: c.label,
            title: c.title,
            sections: c.sections,
        }
    }
}

/// A top-level division (`WorkDivision`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WorkDivisionOutline {
    /// `front`, `1` … `4`.
    pub locator: String,
    /// Its kind.
    pub kind: WorkDivisionKind,
    /// Label as printed.
    pub label: Option<String>,
    /// Title as printed.
    pub title: Option<String>,
    /// Its chapters.
    pub chapters: Vec<WorkChapterOutline>,
}

/// `ParagraphStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum WorkParagraphStyle {
    /// Running text.
    Prose,
    /// An indented block; its line breaks are kept.
    Block,
    /// A heading inside a chapter.
    Heading,
}

/// One paragraph (`WorkParagraph`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WorkParagraphText {
    /// Layout.
    pub style: WorkParagraphStyle,
    /// The text.
    pub text: String,
}

/// `NoteKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum WorkNoteKind {
    /// Only citations.
    Citations,
    /// Comment without citations.
    Comment,
    /// Comment with citations.
    Mixed,
}

/// One note (`WorkNote`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WorkNoteRecord {
    /// `1:61`.
    pub id: String,
    /// The marker as printed.
    pub marker: String,
    /// Paragraph of the section, 0-based.
    pub paragraph: u32,
    /// Scalar offset in it.
    pub scalar: u32,
    /// The note's text.
    pub text: String,
    /// Its kind.
    pub kind: WorkNoteKind,
}

/// One section (`WorkSection`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WorkSectionRecord {
    /// `3.2.7`.
    pub locator: String,
    /// Reading order, from 1.
    pub ordinal: u32,
    /// Its chapter-level unit.
    pub chapter: String,
    /// Printed number.
    pub number: Option<u32>,
    /// Printed label.
    pub label: Option<String>,
    /// How people cite it.
    pub citation: Option<String>,
    /// Paragraphs.
    pub paragraphs: Vec<WorkParagraphText>,
    /// Notes.
    pub notes: Vec<WorkNoteRecord>,
}

impl From<core::WorkSection> for WorkSectionRecord {
    fn from(s: core::WorkSection) -> Self {
        WorkSectionRecord {
            locator: s.locator,
            ordinal: s.ordinal,
            chapter: s.chapter,
            number: s.number,
            label: s.label,
            citation: s.citation,
            paragraphs: s
                .paragraphs
                .into_iter()
                .map(|p| WorkParagraphText {
                    style: match p.style {
                        ParagraphStyle::Prose => WorkParagraphStyle::Prose,
                        ParagraphStyle::Block => WorkParagraphStyle::Block,
                        ParagraphStyle::Heading => WorkParagraphStyle::Heading,
                    },
                    text: p.text,
                })
                .collect(),
            notes: s
                .notes
                .into_iter()
                .map(|n| WorkNoteRecord {
                    id: n.id,
                    marker: n.marker,
                    paragraph: n.paragraph,
                    scalar: n.scalar,
                    text: n.text,
                    kind: match n.kind {
                        NoteKind::Citations => WorkNoteKind::Citations,
                        NoteKind::Comment => WorkNoteKind::Comment,
                        NoteKind::Mixed => WorkNoteKind::Mixed,
                    },
                })
                .collect(),
        }
    }
}

/// A chapter with its sections (`WorkChapter`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WorkChapterRecord {
    /// Its division.
    pub division: String,
    /// The chapter.
    pub outline: WorkChapterOutline,
    /// Its sections.
    pub sections: Vec<WorkSectionRecord>,
}

/// `CitationStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum WorkCitationState {
    /// A passage of the scheme.
    Resolved,
    /// Kept with its class.
    Unresolved,
}

/// One citation in a section's notes (`WorkCitationLink`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WorkCitationRecord {
    /// The citing section.
    pub section: String,
    /// Its note.
    pub note: String,
    /// As printed.
    pub text: String,
    /// Scalar start in the note.
    pub start: u32,
    /// Scalar end in the note, exclusive.
    pub end: u32,
    /// Resolved or not.
    pub status: WorkCitationState,
    /// The passage, OSIS in `eng`, when resolved.
    pub target: Option<String>,
    /// Why not, when unresolved.
    pub class: Option<String>,
    /// A remark.
    pub detail: Option<String>,
}

/// A section that cites a passage (`CitingSection`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WorkCitingSection {
    /// The section.
    pub section: String,
    /// `Institutes 3.2.7`.
    pub label: String,
    /// Its chapter's title.
    pub chapter_title: Option<String>,
    /// The citation as printed.
    pub text: String,
    /// Its note.
    pub note: String,
    /// The whole cited passage, OSIS.
    pub target: String,
}

/// One work pack opened for reading.
#[derive(uniffi::Object)]
pub struct WorkHandle {
    pack: Mutex<WorkPack>,
}

impl WorkHandle {
    fn pack(&self) -> std::sync::MutexGuard<'_, WorkPack> {
        self.pack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[uniffi::export]
impl WorkHandle {
    /// Opens a work pack file read-only.
    #[uniffi::constructor]
    pub fn open(path: String) -> Result<Arc<WorkHandle>, WorkCallError> {
        Ok(Arc::new(WorkHandle {
            pack: Mutex::new(WorkPack::open(std::path::Path::new(&path))?),
        }))
    }

    /// Identity, rights, bibliography and counts: the catalog entry, offline.
    pub fn catalog_entry(&self) -> WorkCatalogEntry {
        self.pack().info().clone().into()
    }

    /// Divisions, chapters and section locators in reading order.
    pub fn structure(&self) -> Vec<WorkDivisionOutline> {
        self.pack()
            .structure()
            .iter()
            .cloned()
            .map(|d| WorkDivisionOutline {
                locator: d.locator,
                kind: match d.kind {
                    DivisionKind::Front => WorkDivisionKind::Front,
                    DivisionKind::Book => WorkDivisionKind::Book,
                },
                label: d.label,
                title: d.title,
                chapters: d.chapters.into_iter().map(Into::into).collect(),
            })
            .collect()
    }

    /// The canonical form of a place as people write it (`Inst. 3.2.7` → `3.2.7`).
    pub fn canonical_locator(&self, input: String) -> Result<String, WorkCallError> {
        Ok(self.pack().canonical_locator(&input)?)
    }

    /// One section.
    pub fn section(&self, locator: String) -> Result<WorkSectionRecord, WorkCallError> {
        Ok(self.pack().section(&locator)?.into())
    }

    /// A chapter-level unit with its sections; a section's locator opens its chapter.
    pub fn chapter(&self, locator: String) -> Result<WorkChapterRecord, WorkCallError> {
        let chapter = self.pack().chapter(&locator)?;
        Ok(WorkChapterRecord {
            division: chapter.division,
            outline: chapter.info.into(),
            sections: chapter.sections.into_iter().map(Into::into).collect(),
        })
    }

    /// A section's citations, unresolved ones included.
    pub fn citations(&self, locator: String) -> Result<Vec<WorkCitationRecord>, WorkCallError> {
        Ok(self
            .pack()
            .citations(&locator)?
            .into_iter()
            .map(|c| WorkCitationRecord {
                section: c.section,
                note: c.note,
                text: c.text,
                start: c.start,
                end: c.end,
                status: match c.status {
                    CitationStatus::Resolved => WorkCitationState::Resolved,
                    CitationStatus::Unresolved => WorkCitationState::Unresolved,
                },
                target: c.target,
                class: c.class,
                detail: c.detail,
            })
            .collect())
    }

    /// The sections citing a passage or a verse inside it; `scheme` is the passage's
    /// versification when it is not `eng` (`org` for the WLC).
    pub fn cited_by(
        &self,
        passage: String,
        scheme: Option<String>,
    ) -> Result<Vec<WorkCitingSection>, WorkCallError> {
        Ok(self
            .pack()
            .cited_by(&passage, scheme.as_deref())?
            .into_iter()
            .map(|c| WorkCitingSection {
                section: c.section,
                label: c.label,
                chapter_title: c.chapter_title,
                text: c.text,
                note: c.note,
                target: c.target,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::shell::ShellHandle;

    fn pack() -> String {
        let dir = std::env::temp_dir().join(format!("openlogos-ffi-works-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test-work.sqlite");
        core::write_work_pack(&path, &sample()).unwrap();
        path.display().to_string()
    }

    /// One hand-written chapter; none of it is the Institutes' text.
    fn sample() -> core::WorkContent {
        use openlogos_core::reference::{AliasTable, parse};
        let rom = parse("Rom 8:28", &AliasTable::english()).unwrap();
        core::WorkContent {
            info: core::WorkInfo {
                resource_id: "test-work".to_owned(),
                work: "institutes".to_owned(),
                locator_scheme: "institutes".to_owned(),
                content_revision: "r1".to_owned(),
                title: "A Test Work".to_owned(),
                short_title: "Test".to_owned(),
                language: "en".to_owned(),
                source_id: "S00".to_owned(),
                license: "CC0-1.0".to_owned(),
                attribution: "Test".to_owned(),
                source: "hand-written".to_owned(),
                bibliography: core::WorkBibliography::default(),
                sections: 1,
                notes: 0,
                citations: 1,
                resolved: 1,
            },
            structure: vec![core::WorkDivision {
                locator: "3".to_owned(),
                kind: DivisionKind::Book,
                label: Some("BOOK III.".to_owned()),
                title: None,
                chapters: vec![core::WorkChapterInfo {
                    locator: "3.2".to_owned(),
                    kind: ChapterKind::Chapter,
                    label: None,
                    title: Some("Faith.".to_owned()),
                    sections: vec!["3.2.7".to_owned()],
                }],
            }],
            sections: vec![core::WorkSection {
                locator: "3.2.7".to_owned(),
                ordinal: 1,
                chapter: "3.2".to_owned(),
                number: Some(7),
                label: Some("VII.".to_owned()),
                citation: Some("Institutes 3.2.7".to_owned()),
                paragraphs: vec![core::WorkParagraph {
                    style: ParagraphStyle::Prose,
                    text: "VII. A firm and certain knowledge.".to_owned(),
                }],
                notes: Vec::new(),
            }],
            citations: vec![core::CitationRow {
                link: core::WorkCitationLink {
                    section: "3.2.7".to_owned(),
                    note: "1:1".to_owned(),
                    text: "Rom. viii. 28".to_owned(),
                    start: 0,
                    end: 13,
                    status: CitationStatus::Resolved,
                    target: Some("Rom.8.28".to_owned()),
                    class: None,
                    detail: None,
                },
                spans: core::verse_spans(&rom).unwrap(),
            }],
            citation_method: "test@1".to_owned(),
        }
    }

    #[test]
    fn the_handle_reads_the_pack_and_refuses_with_a_kind() {
        let handle = WorkHandle::open(pack()).unwrap();
        assert_eq!(handle.catalog_entry().resource_id, "test-work");
        assert_eq!(handle.structure()[0].chapters[0].sections, ["3.2.7"]);
        assert_eq!(
            handle.canonical_locator("Inst. 3.2.7".to_owned()).unwrap(),
            "3.2.7"
        );
        let section = handle.section("Institutes III.ii.7".to_owned()).unwrap();
        assert_eq!(section.paragraphs[0].style, WorkParagraphStyle::Prose);
        assert_eq!(handle.chapter("3.2".to_owned()).unwrap().sections.len(), 1);
        assert_eq!(
            handle.citations("3.2.7".to_owned()).unwrap()[0].status,
            WorkCitationState::Resolved
        );
        let cited = handle.cited_by("Romans 8:28".to_owned(), None).unwrap();
        assert_eq!(cited[0].label, "Institutes 3.2.7");
        let Err(WorkCallError::Refused { kind, .. }) = handle.section("9.9.9".to_owned()) else {
            panic!("a missing section is refused");
        };
        assert_eq!(kind, "not_found");
        assert!(WorkHandle::open("/nonexistent/pack.sqlite".to_owned()).is_err());
    }

    #[test]
    fn the_shell_answers_text_and_the_palette_opens_the_work() {
        use crate::native::shell::{ShellPaletteContext, ShellPlatform};
        let context = || ShellPaletteContext {
            has_selection: false,
            platform: ShellPlatform::Mac,
            expanded: None,
            focus: None,
        };
        let shell = ShellHandle::fixture();
        shell.set_resource_schemes([("test-work".to_owned(), "institutes".to_owned())].into());
        shell.set_resource_titles([("test-work".to_owned(), "Test".to_owned())].into());
        let passage = format!(
            "{:?}",
            shell.palette_query("Inst. 3.2.7".to_owned(), context())
        );
        assert!(
            passage.contains("Institutes 3.2.7") && passage.contains("tab.open_edition"),
            "{passage}"
        );
        shell.use_search_packs(vec![pack()]).unwrap();
        let text = format!(
            "{:?}",
            shell.palette_query("certain know".to_owned(), context())
        );
        assert!(text.contains("Test · Institutes 3.2.7"), "{text}");
        assert!(text.contains("institutes:3.2.7"), "{text}");
        assert!(
            shell
                .use_search_packs(vec!["/nonexistent.sqlite".to_owned()])
                .is_err()
        );
    }
}
