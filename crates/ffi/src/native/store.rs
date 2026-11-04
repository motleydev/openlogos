//! The user store over `crates/workspaces::store` (B074, specs/24-persistence.md): methods on
//! [`ShellHandle`] that export and import the one document a host saves, a change counter
//! for debounced saves, and the notes, highlights and ink the core now keeps, so neither
//! host holds its own copy. The core does no file I/O; the host writes `export_store()`
//! atomically and keeps a backup of any file `import_store` refuses or reports issues for.

use std::sync::Arc;

use openlogos_workspaces::store::{
    Highlight, InkMark, Note, NoteBlock, NoteBlockKind, NoteSpan, RestoreReport, SAVE_DEBOUNCE_MS,
    STORE_FORMAT, STORE_SCHEMA_VERSION, SegmentSpan, StoreError,
};

use super::ink::InkStroke;
use super::reading::ReadingHandle;
use super::shell::ShellHandle;

/// Why a store call was refused. No field is called `message` or `cause`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum StoreCallError {
    /// The core refused the call.
    #[error("{reason}")]
    Refused {
        /// The core's tag: `empty`, `not_json`, `not_a_store`, `unsupported_version`,
        /// `invalid_note`, `invalid_highlight`, `placeholder_text`, `invalid_ink`,
        /// `not_found`.
        kind: String,
        /// Plain words.
        reason: String,
    },
}

impl From<StoreError> for StoreCallError {
    fn from(error: StoreError) -> Self {
        StoreCallError::Refused {
            kind: serde_json::to_value(&error)
                .ok()
                .and_then(|v| v.get("kind").and_then(|k| k.as_str()).map(str::to_owned))
                .unwrap_or_default(),
            reason: error.to_string(),
        }
    }
}

/// What the store's form and the save policy are.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct StoreConstants {
    /// `openlogos-user-store`.
    pub format: String,
    /// The version this core writes (`1.0`).
    pub schema_version: String,
    /// How long after the last change a host saves, in milliseconds.
    pub save_debounce_ms: u32,
}

/// The store's format, version and save debounce.
#[uniffi::export]
#[must_use]
pub fn store_constants() -> StoreConstants {
    StoreConstants {
        format: STORE_FORMAT.to_owned(),
        schema_version: STORE_SCHEMA_VERSION.to_owned(),
        save_debounce_ms: SAVE_DEBOUNCE_MS,
    }
}

/// One run of text in a note block.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct StoreNoteSpan {
    /// The text.
    pub text: String,
    /// Bold.
    #[uniffi(default = false)]
    pub bold: bool,
    /// Italic.
    #[uniffi(default = false)]
    pub italic: bool,
    /// A passage link, OSIS.
    #[uniffi(default = None)]
    pub link: Option<String>,
}

/// What a note block is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum StoreNoteBlockKind {
    /// A paragraph.
    Paragraph,
    /// A heading line.
    Heading,
    /// A bulleted list item.
    ListItem,
    /// A quote.
    Quote,
}

/// One block of a note.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct StoreNoteBlock {
    /// The kind.
    pub kind: StoreNoteBlockKind,
    /// Its runs.
    pub spans: Vec<StoreNoteSpan>,
}

/// A note in the stored form.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct StoreNote {
    /// The note id (the notes tab's `content_id`).
    pub id: String,
    /// The blocks.
    pub blocks: Vec<StoreNoteBlock>,
}

impl From<StoreNote> for Note {
    fn from(n: StoreNote) -> Self {
        Note {
            id: n.id,
            blocks: n
                .blocks
                .into_iter()
                .map(|b| NoteBlock {
                    kind: match b.kind {
                        StoreNoteBlockKind::Paragraph => NoteBlockKind::Paragraph,
                        StoreNoteBlockKind::Heading => NoteBlockKind::Heading,
                        StoreNoteBlockKind::ListItem => NoteBlockKind::ListItem,
                        StoreNoteBlockKind::Quote => NoteBlockKind::Quote,
                    },
                    spans: b
                        .spans
                        .into_iter()
                        .map(|s| NoteSpan {
                            text: s.text,
                            bold: s.bold,
                            italic: s.italic,
                            link: s.link,
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

impl From<&Note> for StoreNote {
    fn from(n: &Note) -> Self {
        StoreNote {
            id: n.id.clone(),
            blocks: n
                .blocks
                .iter()
                .map(|b| StoreNoteBlock {
                    kind: match b.kind {
                        NoteBlockKind::Paragraph => StoreNoteBlockKind::Paragraph,
                        NoteBlockKind::Heading => StoreNoteBlockKind::Heading,
                        NoteBlockKind::ListItem => StoreNoteBlockKind::ListItem,
                        NoteBlockKind::Quote => StoreNoteBlockKind::Quote,
                    },
                    spans: b
                        .spans
                        .iter()
                        .map(|s| StoreNoteSpan {
                            text: s.text.clone(),
                            bold: s.bold,
                            italic: s.italic,
                            link: s.link.clone(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

/// A run of one segment in scalar values `[start, end)`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct StoreSegmentSpan {
    /// The segment.
    pub segment_id: String,
    /// First scalar.
    pub start: u32,
    /// One past the last.
    pub end: u32,
}

/// A highlight.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct StoreHighlight {
    /// Minted by the core; empty when adding.
    #[uniffi(default = "")]
    pub id: String,
    /// The edition.
    pub resource_id: String,
    /// Its revision.
    pub content_revision: String,
    /// The spans, in text order.
    pub spans: Vec<StoreSegmentSpan>,
    /// `#RRGGBB`.
    pub color: String,
}

impl From<StoreHighlight> for Highlight {
    fn from(h: StoreHighlight) -> Self {
        Highlight {
            id: h.id,
            resource_id: h.resource_id,
            content_revision: h.content_revision,
            spans: h
                .spans
                .into_iter()
                .map(|s| SegmentSpan {
                    segment_id: s.segment_id,
                    start: s.start,
                    end: s.end,
                })
                .collect(),
            color: h.color,
        }
    }
}

impl From<&Highlight> for StoreHighlight {
    fn from(h: &Highlight) -> Self {
        StoreHighlight {
            id: h.id.clone(),
            resource_id: h.resource_id.clone(),
            content_revision: h.content_revision.clone(),
            spans: h
                .spans
                .iter()
                .map(|s| StoreSegmentSpan {
                    segment_id: s.segment_id.clone(),
                    start: s.start,
                    end: s.end,
                })
                .collect(),
            color: h.color.clone(),
        }
    }
}

/// An ink stroke pinned to a verse: points with `y` from the verse's top and `x` from the
/// text's leading edge.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct StoreInkMark {
    /// Minted by the core; empty when adding.
    #[uniffi(default = "")]
    pub id: String,
    /// The edition.
    pub resource_id: String,
    /// The verse it is pinned to.
    pub segment_id: String,
    /// The stroke.
    pub stroke: InkStroke,
}

impl From<StoreInkMark> for InkMark {
    fn from(k: StoreInkMark) -> Self {
        InkMark {
            id: k.id,
            resource_id: k.resource_id,
            segment_id: k.segment_id,
            stroke: k.stroke.into(),
        }
    }
}

impl From<&InkMark> for StoreInkMark {
    fn from(k: &InkMark) -> Self {
        StoreInkMark {
            id: k.id.clone(),
            resource_id: k.resource_id.clone(),
            segment_id: k.segment_id.clone(),
            stroke: k.stroke.clone().into(),
        }
    }
}

/// A part of a store set aside on restore.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct StoreIssue {
    /// `session`, `note`, `highlight` or `ink`.
    pub part: String,
    /// Its id, when it has one.
    pub id: Option<String>,
    /// What was wrong.
    pub detail: String,
}

/// What `import_store` did.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct StoreRestoreReport {
    /// The version the document named.
    pub schema_version: String,
    /// Whether the session came from the store.
    pub session_restored: bool,
    /// Workspaces now open.
    pub workspaces: u32,
    /// Tabs across them.
    pub tabs: u32,
    /// Notes restored.
    pub notes: u32,
    /// Highlights restored.
    pub highlights: u32,
    /// Ink strokes restored.
    pub ink: u32,
    /// Parts set aside; non-empty means: keep a backup of the file.
    pub issues: Vec<StoreIssue>,
}

impl From<RestoreReport> for StoreRestoreReport {
    fn from(r: RestoreReport) -> Self {
        StoreRestoreReport {
            schema_version: r.schema_version,
            session_restored: r.session_restored,
            workspaces: r.workspaces,
            tabs: r.tabs,
            notes: r.notes,
            highlights: r.highlights,
            ink: r.ink,
            issues: r
                .issues
                .into_iter()
                .map(|i| StoreIssue {
                    part: serde_json::to_value(i.part)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .unwrap_or_default(),
                    id: i.id,
                    detail: i.detail,
                })
                .collect(),
        }
    }
}

#[uniffi::export]
impl ShellHandle {
    /// The user store as JSON (schemas/user-store.schema.json): what the host writes,
    /// atomically, when `change_count` moved.
    pub fn export_store(&self) -> String {
        self.lock().shell.export_store_json()
    }

    /// Restores a saved store. Refused whole (`empty`, `not_json`, `not_a_store`,
    /// `unsupported_version`) leaves the shell as it was; otherwise `issues` names any part
    /// set aside. Registered titles, languages and schemes are kept.
    pub fn import_store(&self, json: String) -> Result<StoreRestoreReport, StoreCallError> {
        Ok(self.lock().shell.import_store_json(&json)?.into())
    }

    /// Bumped by every change a save would write, quiet ones included (a scroll anchor):
    /// save when it differs from the count at the last save.
    pub fn change_count(&self) -> u64 {
        self.lock().shell.changes
    }

    /// Every note, by id.
    pub fn notes(&self) -> Vec<StoreNote> {
        self.lock()
            .shell
            .annotations
            .notes
            .values()
            .map(Into::into)
            .collect()
    }

    /// One note.
    pub fn note(&self, id: String) -> Option<StoreNote> {
        self.lock().shell.note(&id).map(Into::into)
    }

    /// Saves a note, replacing the one with its id (an editor calls it as the text settles).
    pub fn put_note(&self, note: StoreNote) -> Result<(), StoreCallError> {
        Ok(self.lock().shell.put_note(note.into())?)
    }

    /// Deletes a note.
    pub fn delete_note(&self, id: String) -> Result<(), StoreCallError> {
        Ok(self.lock().shell.delete_note(&id)?)
    }

    /// Adds a highlight and returns its id. With `reading`, a span on placeholder text is
    /// refused (`placeholder_text`).
    pub fn add_highlight(
        &self,
        highlight: StoreHighlight,
        reading: Option<Arc<ReadingHandle>>,
    ) -> Result<String, StoreCallError> {
        let check = |resource: &str, segment: &str| {
            reading
                .as_ref()
                .is_some_and(|r| r.is_placeholder(resource, segment))
        };
        Ok(self
            .lock()
            .shell
            .add_highlight(highlight.into(), Some(&check))?)
    }

    /// Removes a highlight.
    pub fn remove_highlight(&self, id: String) -> Result<(), StoreCallError> {
        Ok(self.lock().shell.remove_highlight(&id)?)
    }

    /// The highlights of an edition with a span in a chapter (`John.3`), oldest first.
    pub fn highlights_in(&self, resource_id: String, chapter_key: String) -> Vec<StoreHighlight> {
        self.lock()
            .shell
            .highlights_in(&resource_id, &chapter_key)
            .into_iter()
            .map(Into::into)
            .collect()
    }

    /// Adds an ink stroke pinned to a verse and returns its id.
    pub fn add_ink(&self, mark: StoreInkMark) -> Result<String, StoreCallError> {
        Ok(self.lock().shell.add_ink(mark.into())?)
    }

    /// Removes an ink stroke.
    pub fn remove_ink(&self, id: String) -> Result<(), StoreCallError> {
        Ok(self.lock().shell.remove_ink(&id)?)
    }

    /// The strokes of an edition pinned to verses of a chapter, oldest first.
    pub fn ink_in(&self, resource_id: String, chapter_key: String) -> Vec<StoreInkMark> {
        self.lock()
            .shell
            .ink_in(&resource_id, &chapter_key)
            .into_iter()
            .map(Into::into)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::ink::{InkPoint, InkTool};

    fn point(x: f64, y: f64, t: f64) -> InkPoint {
        InkPoint {
            x,
            y,
            pressure: None,
            elapsed_ms: t,
            altitude: None,
            azimuth: None,
        }
    }

    #[test]
    fn a_host_saves_and_restores_through_the_handle() {
        let a = ShellHandle::fixture();
        let reading = ReadingHandle::placeholder();
        assert_eq!(a.change_count(), 0);
        a.put_note(StoreNote {
            id: "fixture:note:1".into(),
            blocks: vec![StoreNoteBlock {
                kind: StoreNoteBlockKind::Paragraph,
                spans: vec![StoreNoteSpan {
                    text: "See".into(),
                    bold: false,
                    italic: false,
                    link: Some("John.3.16".into()),
                }],
            }],
        })
        .unwrap();
        let highlight = |segment: &str| StoreHighlight {
            id: String::new(),
            resource_id: "bsb".into(),
            content_revision: "bsb-2024".into(),
            spans: vec![StoreSegmentSpan {
                segment_id: segment.into(),
                start: 0,
                end: 3,
            }],
            color: "#F5D90A".into(),
        };
        let refused = a
            .add_highlight(highlight("John.3.2"), Some(reading.clone()))
            .unwrap_err();
        assert!(
            matches!(refused, StoreCallError::Refused { ref kind, .. } if kind == "placeholder_text")
        );
        assert_eq!(
            a.add_highlight(highlight("John.3.16"), Some(reading))
                .unwrap(),
            "h1"
        );
        let id = a
            .add_ink(StoreInkMark {
                id: String::new(),
                resource_id: "bsb".into(),
                segment_id: "John.3.16".into(),
                stroke: InkStroke {
                    id: String::new(),
                    tool: InkTool::Pen,
                    brush_version: "pf-1.2.3/pen-1".into(),
                    width: 4.0,
                    color: "#000000".into(),
                    opacity: 1.0,
                    points: vec![point(0.0, 0.0, 0.0), point(5.0, 2.0, 8.0)],
                },
            })
            .unwrap();
        assert_eq!(id, "k2");
        assert_eq!(a.change_count(), 3);
        let json = a.export_store();

        let b = ShellHandle::fixture();
        let report = b.import_store(json.clone()).unwrap();
        assert!(report.issues.is_empty());
        assert_eq!((report.notes, report.highlights, report.ink), (1, 1, 1));
        assert_eq!(b.export_store(), json);
        assert_eq!(b.highlights_in("bsb".into(), "John.3".into()).len(), 1);
        assert_eq!(b.ink_in("bsb".into(), "John.3".into())[0].stroke.id, "k2");
        assert_eq!(b.notes(), a.notes());
        let error = b.import_store("{".into()).unwrap_err();
        assert!(matches!(error, StoreCallError::Refused { ref kind, .. } if kind == "not_json"));
        assert_eq!(store_constants().save_debounce_ms, 1000);
    }
}
