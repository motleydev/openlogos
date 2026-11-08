//! The user store of specs/24-persistence.md (B074): one versioned JSON document that holds
//! everything a quit must not lose — the session (schema 1.0 record and its 2.0 workspaces,
//! with every tab's passage, scroll anchor and history, column widths, the tool and the
//! reading settings), the notes, the highlights and the ink.
//!
//! The core owns the form and the rules; it does no file I/O. A host saves what
//! [`Shell::export_store_json`] returns when [`Shell::changes`] moved (debounced, and on quit
//! or when the page is hidden), and on launch hands the file to
//! [`Shell::import_store_json`]. A document that is not a store, or one of a major version
//! this core does not read, is refused whole with a typed [`StoreError`] and the shell is
//! left as it was (the host's fresh session); the host keeps the file aside as a backup.
//! Within a readable store, a part that does not check (the session, or one note,
//! highlight or ink stroke) is set aside and named in [`RestoreReport::issues`], never
//! dropped silently: the host keeps a backup of that file too.
//!
//! [`Annotations`] are what the store keeps beside the session, held by the [`Shell`] so both
//! hosts read and write one truth:
//!
//! - [`Note`]: a small block and span form (paragraph, heading, list item, quote; bold,
//!   italic, a passage link as an OSIS reference). Hosts convert to and from their editors
//!   (HTML on the web, attributed strings on the Mac); neither keeps its own copy.
//! - [`Highlight`]: segment spans in scalar values of one edition revision and a `#RRGGBB`
//!   colour. Never on placeholder text ([`PlaceholderCheck`]); no quoted text is stored.
//! - [`InkMark`]: a B039 stroke pinned to the verse it was drawn at; its points are relative
//!   to that verse's top, so it moves with the text as chapters load and unload.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::commands::Shell;
use crate::migrate_workspace_to_2;
use crate::navigation::is_pane_id;
use crate::shell::{
    InkSettings, ReadingSettings, SESSION_SCHEMA_VERSION, Session, SessionRecord, Tool, Workspace,
    is_swatch,
};

/// The `format` every user store names.
pub const STORE_FORMAT: &str = "openlogos-user-store";

/// The store schema version this module writes. It reads every 1.x: a later minor only adds
/// fields, which an older core ignores.
pub const STORE_SCHEMA_VERSION: &str = "1.0";

/// The store's major version: a document of any other major is refused.
pub const STORE_MAJOR: u32 = 1;

/// How long a host waits after the last change before it saves (specs/24 §4).
pub const SAVE_DEBOUNCE_MS: u32 = 1000;

/// One run of text in a note block.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteSpan {
    /// The text, as typed.
    pub text: String,
    /// Bold.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    /// Italic.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    /// A passage link: the OSIS reference the span opens (`John.3.16`, `John.3.16-John.3.18`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

/// What a note block is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteBlockKind {
    /// A paragraph.
    #[default]
    Paragraph,
    /// A heading line (1.25× the reading size on both hosts).
    Heading,
    /// One bulleted list item.
    ListItem,
    /// A quote, drawn with a rule on the leading edge.
    Quote,
}

/// One block of a note: a kind and its spans.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteBlock {
    /// The kind.
    pub kind: NoteBlockKind,
    /// Its text runs, in order; empty for an empty line.
    #[serde(default)]
    pub spans: Vec<NoteSpan>,
}

/// A note as stored: its id (the `content_id` of the notes tabs and the dock that show it)
/// and its blocks.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    /// The note id.
    pub id: String,
    /// The blocks, in order.
    #[serde(default)]
    pub blocks: Vec<NoteBlock>,
}

impl Note {
    /// A note of plain paragraphs, one per line of `text`.
    #[must_use]
    pub fn from_text(id: &str, text: &str) -> Note {
        Note {
            id: id.to_owned(),
            blocks: text
                .lines()
                .map(|line| NoteBlock {
                    kind: NoteBlockKind::Paragraph,
                    spans: if line.is_empty() {
                        Vec::new()
                    } else {
                        vec![NoteSpan {
                            text: line.to_owned(),
                            ..NoteSpan::default()
                        }]
                    },
                })
                .collect(),
        }
    }

    /// The note's text, blocks joined by newlines.
    #[must_use]
    pub fn plain_text(&self) -> String {
        self.blocks
            .iter()
            .map(|b| b.spans.iter().map(|s| s.text.as_str()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The passages the note links to, in order, each once.
    #[must_use]
    pub fn links(&self) -> Vec<String> {
        let mut links: Vec<String> = Vec::new();
        for link in self
            .blocks
            .iter()
            .flat_map(|b| &b.spans)
            .filter_map(|s| s.link.as_ref())
        {
            if !links.contains(link) {
                links.push(link.clone());
            }
        }
        links
    }

    /// Checks the note: an id of the shape tab ids have, and every link a passage.
    pub fn check(&self) -> Result<(), StoreError> {
        if !is_pane_id(&self.id) {
            return Err(StoreError::InvalidNote {
                id: self.id.clone(),
                detail: "a note id is 1–64 letters, digits, `_`, `.`, `:` or `-`".to_owned(),
            });
        }
        for link in self
            .blocks
            .iter()
            .flat_map(|b| &b.spans)
            .filter_map(|s| s.link.as_deref())
        {
            if crate::palette::parse_tab_reference(link).is_none() {
                return Err(StoreError::InvalidNote {
                    id: self.id.clone(),
                    detail: format!("the link `{link}` is not a passage"),
                });
            }
        }
        Ok(())
    }
}

/// A run of one segment, in Unicode scalar values `[start, end)`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SegmentSpan {
    /// The segment (`John.3.16`).
    pub segment_id: String,
    /// First scalar.
    pub start: u32,
    /// One past the last scalar.
    pub end: u32,
}

/// A highlight: segment spans of one edition revision and a colour. A selection across
/// verses (or chapters) is one highlight of several spans.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Highlight {
    /// Stable id, minted by the core (`h1`).
    #[serde(default)]
    pub id: String,
    /// The edition.
    pub resource_id: String,
    /// The edition revision the offsets were taken against.
    pub content_revision: String,
    /// The spans, in text order; never empty.
    pub spans: Vec<SegmentSpan>,
    /// `#RRGGBB`, upper case.
    pub color: String,
}

impl Highlight {
    /// Checks the highlight's shape (not its text: see [`PlaceholderCheck`]).
    pub fn check(&self) -> Result<(), StoreError> {
        let bad = |detail: String| StoreError::InvalidHighlight {
            id: self.id.clone(),
            detail,
        };
        if self.resource_id.is_empty() || self.content_revision.is_empty() {
            return Err(bad("a highlight names its edition and revision".to_owned()));
        }
        if self.spans.is_empty() {
            return Err(bad("a highlight has at least one span".to_owned()));
        }
        for span in &self.spans {
            if span.segment_id.is_empty() || span.end <= span.start {
                return Err(bad(format!(
                    "the span {}[{}, {}) is empty or runs backwards",
                    span.segment_id, span.start, span.end
                )));
            }
        }
        if !is_swatch(&self.color) {
            return Err(bad(format!("`{}` is not a #RRGGBB colour", self.color)));
        }
        Ok(())
    }

    /// Whether any span lies in the chapter `chapter_key` (`John.3`).
    #[must_use]
    pub fn in_chapter(&self, chapter_key: &str) -> bool {
        self.spans
            .iter()
            .any(|s| segment_in_chapter(&s.segment_id, chapter_key))
    }
}

/// A stroke of ink pinned to the verse it was drawn at (B078's verse anchor, now stored).
/// The stroke's points are in the reading column's coordinates with `y` measured from the
/// top of `segment_id` and `x` from the text's leading edge, so the host draws it at the
/// verse wherever the verse is laid out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InkMark {
    /// Stable id, minted by the core (`k1`); the stroke's own id is the same.
    #[serde(default)]
    pub id: String,
    /// The edition the stroke was drawn over.
    pub resource_id: String,
    /// The verse it is pinned to.
    pub segment_id: String,
    /// The stroke (schemas/ink-document.schema.json's stroke).
    pub stroke: openlogos_ink::Stroke,
}

impl InkMark {
    /// Checks the mark and its stroke.
    pub fn check(&self) -> Result<(), StoreError> {
        let bad = |detail: String| StoreError::InvalidInk {
            id: self.id.clone(),
            detail,
        };
        if self.resource_id.is_empty() || self.segment_id.is_empty() {
            return Err(bad("a stroke names its edition and verse".to_owned()));
        }
        if self
            .stroke
            .points
            .iter()
            .any(|p| !p.x.is_finite() || !p.y.is_finite())
        {
            return Err(bad("a point is not a finite number".to_owned()));
        }
        self.stroke.validate().map_err(|e| bad(e.to_string()))
    }

    /// Whether the stroke is pinned to a verse of `chapter_key` (`John.3`).
    #[must_use]
    pub fn in_chapter(&self, chapter_key: &str) -> bool {
        segment_in_chapter(&self.segment_id, chapter_key)
    }
}

/// Ink coordinates are kept to a tenth of a point, sensor values (pressure, altitude,
/// azimuth) to a hundredth and time to a millisecond: finer than a stylus resolves on screen,
/// and what keeps a stroke small in the store (specs/24 §2.3). [`Shell::add_ink`] rounds a
/// stroke on the way in, so the shell holds exactly what a restore gives back.
const XY_SCALE: f64 = 10.0;
const SENSOR_SCALE: f64 = 100.0;

fn fixed(value: f64, scale: f64) -> i64 {
    (value * scale).round() as i64
}

fn unfixed(value: i64, scale: f64) -> f64 {
    value as f64 / scale
}

/// Rounds a stroke's points to the stored precision.
fn quantize(stroke: &mut openlogos_ink::Stroke) {
    for p in &mut stroke.points {
        p.x = unfixed(fixed(p.x, XY_SCALE), XY_SCALE);
        p.y = unfixed(fixed(p.y, XY_SCALE), XY_SCALE);
        p.elapsed_ms = unfixed(fixed(p.elapsed_ms, 1.0), 1.0);
        for v in [&mut p.pressure, &mut p.altitude, &mut p.azimuth] {
            *v = v.map(|v| unfixed(fixed(v, SENSOR_SCALE), SENSOR_SCALE));
        }
    }
}

/// An ink stroke as the store writes it: the mark's fields and the stroke's paint flat, the
/// points as columns of fixed-point integers, each column delta-encoded (the first value as
/// is, then the change from the value before; a sensor column skips its `null`s). `xy` is
/// tenths of a point, interleaved (`x0, y0, dx1, dy1, …`); `t` milliseconds; `pressure`,
/// `altitude` and `azimuth` hundredths, present only when some point reported them. Small
/// numbers make a stroke about a sixth of the size of point objects and quick to write.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredInk {
    /// The mark's id, also the stroke's.
    pub id: String,
    /// The edition.
    pub resource_id: String,
    /// The verse the stroke is pinned to.
    pub segment_id: String,
    /// `pen` or `highlighter`.
    pub tool: openlogos_ink::Tool,
    /// The brush and its version.
    pub brush_version: String,
    /// Base width in points, as drawn.
    pub width: f64,
    /// `#RRGGBB`.
    pub color: String,
    /// Opacity `0..=1`.
    pub opacity: f64,
    /// `x0, y0, dx1, dy1, …` in tenths of a point, `y` from the verse's top.
    pub xy: Vec<i64>,
    /// Milliseconds since the first point, delta-encoded, one per point.
    pub t: Vec<i64>,
    /// Pressure in hundredths, one per point; absent when no point reported one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure: Option<Vec<Option<i64>>>,
    /// Stylus altitude in hundredths of a radian; absent when none reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub altitude: Option<Vec<Option<i64>>>,
    /// Stylus azimuth in hundredths of a radian; absent when none reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub azimuth: Option<Vec<Option<i64>>>,
}

/// Delta-encodes a column: the first value, then each change from the one before.
fn deltas(values: impl Iterator<Item = i64>) -> Vec<i64> {
    let mut last = 0;
    values
        .map(|v| {
            let d = v - last;
            last = v;
            d
        })
        .collect()
}

/// Undoes [`deltas`].
fn undeltas(values: &[i64]) -> Vec<i64> {
    let mut last = 0;
    values
        .iter()
        .map(|d| {
            last += d;
            last
        })
        .collect()
}

/// [`deltas`] over a column with gaps: a `null` stays `null` and is skipped.
fn sparse_deltas(values: impl Iterator<Item = Option<i64>>) -> Vec<Option<i64>> {
    let mut last = 0;
    values
        .map(|v| {
            v.map(|v| {
                let d = v - last;
                last = v;
                d
            })
        })
        .collect()
}

/// Undoes [`sparse_deltas`].
fn sparse_undeltas(values: &[Option<i64>]) -> Vec<Option<i64>> {
    let mut last = 0;
    values
        .iter()
        .map(|d| {
            d.map(|d| {
                last += d;
                last
            })
        })
        .collect()
}

impl From<&InkMark> for StoredInk {
    fn from(k: &InkMark) -> StoredInk {
        let s = &k.stroke;
        let column = |get: fn(&openlogos_ink::InkPoint) -> Option<f64>| {
            s.points.iter().any(|p| get(p).is_some()).then(|| {
                sparse_deltas(
                    s.points
                        .iter()
                        .map(|p| get(p).map(|v| fixed(v, SENSOR_SCALE))),
                )
            })
        };
        StoredInk {
            id: k.id.clone(),
            resource_id: k.resource_id.clone(),
            segment_id: k.segment_id.clone(),
            tool: s.tool,
            brush_version: s.brush_version.clone(),
            width: s.width,
            color: s.color.clone(),
            opacity: s.opacity,
            xy: deltas(s.points.iter().map(|p| fixed(p.x, XY_SCALE)))
                .into_iter()
                .zip(deltas(s.points.iter().map(|p| fixed(p.y, XY_SCALE))))
                .flat_map(|(x, y)| [x, y])
                .collect(),
            t: deltas(s.points.iter().map(|p| fixed(p.elapsed_ms, 1.0))),
            pressure: column(|p| p.pressure),
            altitude: column(|p| p.altitude),
            azimuth: column(|p| p.azimuth),
        }
    }
}

impl TryFrom<StoredInk> for InkMark {
    type Error = String;

    fn try_from(k: StoredInk) -> Result<InkMark, String> {
        let n = k.t.len();
        if k.xy.len() != 2 * n {
            return Err(format!("{} coordinates for {n} points", k.xy.len()));
        }
        for (name, column) in [
            ("pressure", &k.pressure),
            ("altitude", &k.altitude),
            ("azimuth", &k.azimuth),
        ] {
            if column.as_ref().is_some_and(|c| c.len() != n) {
                return Err(format!("{name} does not have one value per point"));
            }
        }
        let xs = undeltas(&k.xy.iter().step_by(2).copied().collect::<Vec<_>>());
        let ys = undeltas(&k.xy.iter().skip(1).step_by(2).copied().collect::<Vec<_>>());
        let ts = undeltas(&k.t);
        let column = |c: &Option<Vec<Option<i64>>>| c.as_deref().map(sparse_undeltas);
        let (pressure, altitude, azimuth) =
            (column(&k.pressure), column(&k.altitude), column(&k.azimuth));
        let at = |column: &Option<Vec<Option<i64>>>, i: usize| {
            column
                .as_ref()
                .and_then(|c| c[i])
                .map(|v| unfixed(v, SENSOR_SCALE))
        };
        let points = (0..n)
            .map(|i| openlogos_ink::InkPoint {
                x: unfixed(xs[i], XY_SCALE),
                y: unfixed(ys[i], XY_SCALE),
                pressure: at(&pressure, i),
                elapsed_ms: unfixed(ts[i], 1.0),
                altitude: at(&altitude, i),
                azimuth: at(&azimuth, i),
            })
            .collect();
        Ok(InkMark {
            stroke: openlogos_ink::Stroke {
                id: k.id.clone(),
                tool: k.tool,
                brush_version: k.brush_version,
                width: k.width,
                color: k.color,
                opacity: k.opacity,
                points,
            },
            id: k.id,
            resource_id: k.resource_id,
            segment_id: k.segment_id,
        })
    }
}

/// The written JSON of the annotation parts, kept until the part changes, so a save after a
/// scroll re-serializes only the session (B074). Never compared or persisted.
#[derive(Debug, Clone, Default)]
pub struct StoreCache {
    notes: Option<String>,
    highlights: Option<String>,
    ink: Option<String>,
    /// Each stroke's JSON by id: a stroke never changes once drawn, so adding or erasing one
    /// rewrites only the join.
    strokes: BTreeMap<String, String>,
}

impl PartialEq for StoreCache {
    fn eq(&self, _: &StoreCache) -> bool {
        true
    }
}

/// Whether `segment_id` (`John.3.16`, `Ps.3.0`, `Num.26.3-4`) is in `chapter_key` (`John.3`).
#[must_use]
pub fn segment_in_chapter(segment_id: &str, chapter_key: &str) -> bool {
    segment_id
        .strip_prefix(chapter_key)
        .is_some_and(|rest| rest.starts_with('.'))
}

/// Tells generated placeholder text from an edition's own (specs/23 §5): a highlight is never
/// stored on a placeholder. The bindings answer it from the host's `ReadingHandle`
/// (`ReadingLibrary::segment_is_placeholder`); a segment it does not know is not one.
pub trait PlaceholderCheck {
    /// Whether `segment_id` of `resource_id` is placeholder text.
    fn is_placeholder(&self, resource_id: &str, segment_id: &str) -> bool;
}

impl<F: Fn(&str, &str) -> bool> PlaceholderCheck for F {
    fn is_placeholder(&self, resource_id: &str, segment_id: &str) -> bool {
        self(resource_id, segment_id)
    }
}

/// Notes, highlights and ink: what the store keeps beside the session.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Annotations {
    /// Notes by id.
    #[serde(default)]
    pub notes: BTreeMap<String, Note>,
    /// Highlights, oldest first (later ones paint over earlier ones).
    #[serde(default)]
    pub highlights: Vec<Highlight>,
    /// Ink strokes, oldest first.
    #[serde(default)]
    pub ink: Vec<InkMark>,
    /// The next serial for highlight and ink ids.
    #[serde(default)]
    pub next_serial: u64,
}

impl Annotations {
    fn mint(&mut self, prefix: &str) -> String {
        loop {
            self.next_serial += 1;
            let id = format!("{prefix}{}", self.next_serial);
            if !self.highlights.iter().any(|h| h.id == id) && !self.ink.iter().any(|k| k.id == id) {
                return id;
            }
        }
    }
}

/// Why a store, or a note, highlight or stroke handed to the core, was refused. Serialized
/// with a `kind` tag; no field is called `message` or `cause`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StoreError {
    /// Nothing to read: no file yet. The host starts fresh; there is nothing to back up.
    #[error("the store is empty")]
    Empty,
    /// Not JSON at all (a truncated or overwritten file).
    #[error("the store is not JSON: {detail}")]
    NotJson {
        /// The parser's words.
        detail: String,
    },
    /// JSON, but not a user store.
    #[error("not an OpenLogos user store: {detail}")]
    NotAStore {
        /// What was wrong.
        detail: String,
    },
    /// A store of a major version this core does not read (written by a newer app).
    #[error("store version {version} is newer than this app reads (1.x)")]
    UnsupportedVersion {
        /// The version the document names.
        version: String,
    },
    /// A note that does not check.
    #[error("note {id}: {detail}")]
    InvalidNote {
        /// The note id.
        id: String,
        /// What was wrong.
        detail: String,
    },
    /// A highlight that does not check.
    #[error("highlight {id}: {detail}")]
    InvalidHighlight {
        /// The highlight id (empty for a new one).
        id: String,
        /// What was wrong.
        detail: String,
    },
    /// A highlight on placeholder text (specs/23 §5).
    #[error("{segment_id} is placeholder text; placeholder text is never highlighted")]
    PlaceholderText {
        /// The segment.
        segment_id: String,
    },
    /// An ink stroke that does not check.
    #[error("ink {id}: {detail}")]
    InvalidInk {
        /// The stroke id (empty for a new one).
        id: String,
        /// What was wrong.
        detail: String,
    },
    /// No note, highlight or stroke has this id.
    #[error("nothing has the id {id}")]
    NotFound {
        /// The id.
        id: String,
    },
}

/// Which part of a store a [`StoreIssue`] is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorePart {
    /// The session record and its workspaces.
    Session,
    /// One note.
    Note,
    /// One highlight.
    Highlight,
    /// One ink stroke.
    Ink,
}

/// A part of a readable store that was set aside on restore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreIssue {
    /// The part.
    pub part: StorePart,
    /// Its id, when it has one.
    pub id: Option<String>,
    /// What was wrong, in words.
    pub detail: String,
}

/// What a restore did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreReport {
    /// The version the document named.
    pub schema_version: String,
    /// Whether the session came from the store (`false`: it was set aside and the shell
    /// keeps the session it had).
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
    /// Parts set aside; non-empty means the host keeps a backup of the file it read.
    pub issues: Vec<StoreIssue>,
}

/// The user store document (schemas/user-store.schema.json), as written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserStore {
    /// [`STORE_FORMAT`].
    pub format: String,
    /// [`STORE_SCHEMA_VERSION`].
    pub schema_version: String,
    /// Synthetic data, never a user's store.
    pub fixture_only: bool,
    /// The core API version that wrote it.
    pub core_api_version: String,
    /// The session record (schemas/session.schema.json).
    pub session: SessionRecord,
    /// Its workspaces, in the record's order (schemas/workspace-2.schema.json).
    pub workspaces: Vec<Workspace>,
    /// Notes, by id.
    pub notes: Vec<Note>,
    /// Highlights, oldest first.
    pub highlights: Vec<Highlight>,
    /// Ink, oldest first, in the stored form.
    pub ink: Vec<StoredInk>,
    /// The next serial for highlight and ink ids.
    pub next_serial: u64,
}

#[derive(Serialize)]
struct SessionRecordRef<'a> {
    schema_version: &'a str,
    fixture_only: bool,
    workspace_ids: Vec<&'a str>,
    active_workspace_id: &'a str,
    tool: Tool,
    next_serial: u64,
    reading: &'a ReadingSettings,
    ink: &'a InkSettings,
}

/// Migration steps between store minor versions, applied in order to a document that names
/// `from`. None yet: 1.0 is the first store. A future 1.x that renames or moves a field adds
/// its step here; a new field needs none (serde defaults).
/// One step: the version it reads, the version it writes and the rewrite.
type StoreStep = (&'static str, &'static str, fn(Value) -> Value);

const STORE_STEPS: &[StoreStep] = &[];

/// Brings a store document to the version this core reads: checks the format and major
/// version and runs [`STORE_STEPS`]. The forward-version hook of specs/24 §3.
pub fn migrate_store(mut value: Value) -> Result<Value, StoreError> {
    let Some(object) = value.as_object() else {
        return Err(StoreError::NotAStore {
            detail: "the document is not a JSON object".to_owned(),
        });
    };
    match object.get("format").and_then(Value::as_str) {
        Some(STORE_FORMAT) => {}
        Some(other) => {
            return Err(StoreError::NotAStore {
                detail: format!("its format is `{other}`"),
            });
        }
        None => {
            return Err(StoreError::NotAStore {
                detail: "it has no `format`".to_owned(),
            });
        }
    }
    let version = object
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| StoreError::NotAStore {
            detail: "it has no `schema_version`".to_owned(),
        })?
        .to_owned();
    let major = version
        .split('.')
        .next()
        .and_then(|m| m.parse::<u32>().ok())
        .ok_or_else(|| StoreError::NotAStore {
            detail: format!("`{version}` is not a version"),
        })?;
    if major != STORE_MAJOR {
        return Err(StoreError::UnsupportedVersion { version });
    }
    let mut current = version;
    for (from, to, step) in STORE_STEPS {
        if current == *from {
            value = step(value);
            current = (*to).to_owned();
        }
    }
    Ok(value)
}

fn take_array(value: &mut Value, key: &str) -> Vec<Value> {
    match value.get_mut(key).map(Value::take) {
        Some(Value::Array(items)) => items,
        _ => Vec::new(),
    }
}

/// The session part of a store: the record and its workspaces, 1.x workspaces migrated.
fn read_session(value: &mut Value) -> Result<Session, String> {
    let record = value
        .get_mut("session")
        .map(Value::take)
        .ok_or("the store has no session")?;
    let record: SessionRecord = serde_json::from_value(record).map_err(|e| e.to_string())?;
    if record.schema_version != SESSION_SCHEMA_VERSION {
        return Err(format!(
            "session version {} is not {SESSION_SCHEMA_VERSION}",
            record.schema_version
        ));
    }
    let mut workspaces = Vec::new();
    for w in take_array(value, "workspaces") {
        let w = migrate_workspace_to_2(w).map_err(|e| e.to_string())?;
        let w: Workspace = serde_json::from_value(w).map_err(|e| e.to_string())?;
        workspaces.push(w);
    }
    Session::from_records(record, workspaces).map_err(|e| e.to_string())
}

impl Shell {
    /// The user store as a document.
    #[must_use]
    pub fn export_store(&self) -> UserStore {
        let (session, workspaces) = self.session.to_records();
        UserStore {
            format: STORE_FORMAT.to_owned(),
            schema_version: STORE_SCHEMA_VERSION.to_owned(),
            fixture_only: self.session.fixture_only,
            core_api_version: openlogos_core::API_VERSION.to_owned(),
            session,
            workspaces,
            notes: self.annotations.notes.values().cloned().collect(),
            highlights: self.annotations.highlights.clone(),
            ink: self.annotations.ink.iter().map(StoredInk::from).collect(),
            next_serial: self.annotations.next_serial,
        }
    }

    /// The user store as JSON, what a host writes: [`Shell::export_store`] without copying
    /// the session, compact, with the notes, highlights and ink taken from their cached JSON
    /// when they have not changed since the last save (so a save after a scroll writes only
    /// the session anew).
    pub fn export_store_json(&mut self) -> String {
        self.fill_store_cache();
        let cache = &self.store_cache;
        let (notes, highlights, ink) = (
            cache.notes.as_deref().unwrap_or("[]"),
            cache.highlights.as_deref().unwrap_or("[]"),
            cache.ink.as_deref().unwrap_or("[]"),
        );
        let s = &self.session;
        let record = SessionRecordRef {
            schema_version: &s.schema_version,
            fixture_only: s.fixture_only,
            workspace_ids: s.workspaces.iter().map(|w| w.id.as_str()).collect(),
            active_workspace_id: &s.active_workspace_id,
            tool: s.tool,
            next_serial: s.next_serial,
            reading: &s.reading,
            ink: &s.ink,
        };
        let session = serde_json::to_string(&record).expect("a session serializes");
        let workspaces = serde_json::to_string(&s.workspaces).expect("workspaces serialize");
        let mut out = String::with_capacity(
            200 + session.len() + workspaces.len() + notes.len() + highlights.len() + ink.len(),
        );
        out.push_str("{\"format\":\"");
        out.push_str(STORE_FORMAT);
        out.push_str("\",\"schema_version\":\"");
        out.push_str(STORE_SCHEMA_VERSION);
        out.push_str("\",\"fixture_only\":");
        out.push_str(if s.fixture_only { "true" } else { "false" });
        out.push_str(",\"core_api_version\":\"");
        out.push_str(openlogos_core::API_VERSION);
        out.push_str("\",\"session\":");
        out.push_str(&session);
        out.push_str(",\"workspaces\":");
        out.push_str(&workspaces);
        out.push_str(",\"notes\":");
        out.push_str(notes);
        out.push_str(",\"highlights\":");
        out.push_str(highlights);
        out.push_str(",\"ink\":");
        out.push_str(ink);
        out.push_str(",\"next_serial\":");
        out.push_str(&self.annotations.next_serial.to_string());
        out.push('}');
        out
    }

    /// Writes the JSON of every annotation part that changed since it was last written.
    fn fill_store_cache(&mut self) {
        let annotations = &self.annotations;
        let cache = &mut self.store_cache;
        cache.notes.get_or_insert_with(|| {
            serde_json::to_string(&annotations.notes.values().collect::<Vec<_>>())
                .expect("notes serialize")
        });
        cache.highlights.get_or_insert_with(|| {
            serde_json::to_string(&annotations.highlights).expect("highlights serialize")
        });
        if cache.ink.is_none() {
            let strokes = &mut cache.strokes;
            strokes.retain(|id, _| annotations.ink.iter().any(|k| &k.id == id));
            let mut out = String::from("[");
            for (i, mark) in annotations.ink.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(strokes.entry(mark.id.clone()).or_insert_with(|| {
                    serde_json::to_string(&StoredInk::from(mark)).expect("ink serializes")
                }));
            }
            out.push(']');
            cache.ink = Some(out);
        }
    }

    /// Restores a saved store (specs/24 §5). A document that is empty, not JSON, not a store
    /// or of another major version fails whole and leaves the shell as it was. Otherwise
    /// the session replaces the shell's (keeping the host's registered titles, languages and
    /// schemes) and the notes, highlights and ink replace the shell's; a part that does not
    /// check is set aside and reported. Clears undo and redo and bumps the revision.
    pub fn import_store_json(&mut self, json: &str) -> Result<RestoreReport, StoreError> {
        if json.trim().is_empty() {
            return Err(StoreError::Empty);
        }
        let value: Value = serde_json::from_str(json).map_err(|e| StoreError::NotJson {
            detail: e.to_string(),
        })?;
        let mut value = migrate_store(value)?;
        let schema_version = value["schema_version"].as_str().unwrap_or("").to_owned();
        let mut issues = Vec::new();

        let session = match read_session(&mut value) {
            Ok(session) => Some(session),
            Err(detail) => {
                issues.push(StoreIssue {
                    part: StorePart::Session,
                    id: None,
                    detail,
                });
                None
            }
        };

        let mut annotations = Annotations {
            next_serial: value["next_serial"].as_u64().unwrap_or(0),
            ..Annotations::default()
        };
        let id_of = |v: &Value| v.get("id").and_then(Value::as_str).map(str::to_owned);
        for item in take_array(&mut value, "notes") {
            let id = id_of(&item);
            let checked = serde_json::from_value::<Note>(item)
                .map_err(|e| e.to_string())
                .and_then(|n| n.check().map(|()| n).map_err(|e| e.to_string()));
            match checked {
                Ok(note) if annotations.notes.contains_key(&note.id) => issues.push(StoreIssue {
                    part: StorePart::Note,
                    id,
                    detail: "a second note with this id".to_owned(),
                }),
                Ok(note) => {
                    annotations.notes.insert(note.id.clone(), note);
                }
                Err(detail) => issues.push(StoreIssue {
                    part: StorePart::Note,
                    id,
                    detail,
                }),
            }
        }
        for item in take_array(&mut value, "highlights") {
            let id = id_of(&item);
            let checked = serde_json::from_value::<Highlight>(item)
                .map_err(|e| e.to_string())
                .and_then(|h| h.check().map(|()| h).map_err(|e| e.to_string()));
            match checked {
                Ok(h) if h.id.is_empty() || annotations.highlights.iter().any(|o| o.id == h.id) => {
                    issues.push(StoreIssue {
                        part: StorePart::Highlight,
                        id,
                        detail: "a missing or repeated id".to_owned(),
                    });
                }
                Ok(h) => annotations.highlights.push(h),
                Err(detail) => issues.push(StoreIssue {
                    part: StorePart::Highlight,
                    id,
                    detail,
                }),
            }
        }
        for item in take_array(&mut value, "ink") {
            let id = id_of(&item);
            let checked = serde_json::from_value::<StoredInk>(item)
                .map_err(|e| e.to_string())
                .and_then(InkMark::try_from)
                .and_then(|k| k.check().map(|()| k).map_err(|e| e.to_string()));
            match checked {
                Ok(k) if k.id.is_empty() || annotations.ink.iter().any(|o| o.id == k.id) => {
                    issues.push(StoreIssue {
                        part: StorePart::Ink,
                        id,
                        detail: "a missing or repeated id".to_owned(),
                    });
                }
                Ok(k) => annotations.ink.push(k),
                Err(detail) => issues.push(StoreIssue {
                    part: StorePart::Ink,
                    id,
                    detail,
                }),
            }
        }

        let session_restored = session.is_some();
        if let Some(mut session) = session {
            let old = &mut self.session;
            session.resource_titles = std::mem::take(&mut old.resource_titles);
            session.resource_languages = std::mem::take(&mut old.resource_languages);
            session.resource_schemes = std::mem::take(&mut old.resource_schemes);
            session.morphology_editions = std::mem::take(&mut old.morphology_editions);
            self.session = session;
        }
        self.annotations = annotations;
        // Written once here, inside the restore's budget, so the saves that follow rewrite
        // only what changed.
        self.store_cache = StoreCache::default();
        self.fill_store_cache();
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.revision += 1;
        if !issues.is_empty() {
            // What was set aside is not in the shell: the next save writes what is.
            self.changes += 1;
        }
        let s = &self.session;
        Ok(RestoreReport {
            schema_version,
            session_restored,
            workspaces: s.workspaces.len() as u32,
            tabs: s.workspaces.iter().map(|w| w.tabs().count() as u32).sum(),
            notes: self.annotations.notes.len() as u32,
            highlights: self.annotations.highlights.len() as u32,
            ink: self.annotations.ink.len() as u32,
            issues,
        })
    }

    /// A note by id.
    #[must_use]
    pub fn note(&self, id: &str) -> Option<&Note> {
        self.annotations.notes.get(id)
    }

    /// Saves a note, replacing the one with its id.
    pub fn put_note(&mut self, note: Note) -> Result<(), StoreError> {
        note.check()?;
        if self.annotations.notes.get(&note.id) != Some(&note) {
            self.annotations.notes.insert(note.id.clone(), note);
            self.store_cache.notes = None;
            self.changes += 1;
        }
        Ok(())
    }

    /// Deletes a note.
    pub fn delete_note(&mut self, id: &str) -> Result<(), StoreError> {
        self.annotations
            .notes
            .remove(id)
            .ok_or_else(|| StoreError::NotFound { id: id.to_owned() })?;
        self.store_cache.notes = None;
        self.changes += 1;
        Ok(())
    }

    /// Adds a highlight and returns its new id. Refuses one whose spans touch placeholder
    /// text when `placeholders` says so. The colour is stored upper case.
    pub fn add_highlight(
        &mut self,
        mut highlight: Highlight,
        placeholders: Option<&dyn PlaceholderCheck>,
    ) -> Result<String, StoreError> {
        highlight.id = String::new();
        highlight.color = highlight.color.to_uppercase();
        highlight.check()?;
        if let Some(check) = placeholders
            && let Some(span) = highlight
                .spans
                .iter()
                .find(|s| check.is_placeholder(&highlight.resource_id, &s.segment_id))
        {
            return Err(StoreError::PlaceholderText {
                segment_id: span.segment_id.clone(),
            });
        }
        highlight.id = self.annotations.mint("h");
        let id = highlight.id.clone();
        self.annotations.highlights.push(highlight);
        self.store_cache.highlights = None;
        self.changes += 1;
        Ok(id)
    }

    /// Removes a highlight.
    pub fn remove_highlight(&mut self, id: &str) -> Result<(), StoreError> {
        let index = self
            .annotations
            .highlights
            .iter()
            .position(|h| h.id == id)
            .ok_or_else(|| StoreError::NotFound { id: id.to_owned() })?;
        self.annotations.highlights.remove(index);
        self.store_cache.highlights = None;
        self.changes += 1;
        Ok(())
    }

    /// The highlights of an edition with a span in one chapter, oldest first: what a host
    /// paints when it renders that chapter's block.
    #[must_use]
    pub fn highlights_in(&self, resource_id: &str, chapter_key: &str) -> Vec<&Highlight> {
        self.annotations
            .highlights
            .iter()
            .filter(|h| h.resource_id == resource_id && h.in_chapter(chapter_key))
            .collect()
    }

    /// Adds an ink stroke and returns its new id (also set as the stroke's id).
    pub fn add_ink(&mut self, mut mark: InkMark) -> Result<String, StoreError> {
        let id = self.annotations.mint("k");
        mark.id.clone_from(&id);
        mark.stroke.id.clone_from(&id);
        quantize(&mut mark.stroke);
        mark.check()?;
        self.annotations.ink.push(mark);
        self.store_cache.ink = None;
        self.changes += 1;
        Ok(id)
    }

    /// Removes an ink stroke (the eraser removes whole strokes).
    pub fn remove_ink(&mut self, id: &str) -> Result<(), StoreError> {
        let index = self
            .annotations
            .ink
            .iter()
            .position(|k| k.id == id)
            .ok_or_else(|| StoreError::NotFound { id: id.to_owned() })?;
        self.annotations.ink.remove(index);
        self.store_cache.ink = None;
        self.changes += 1;
        Ok(())
    }

    /// The strokes of an edition pinned to verses of one chapter, oldest first.
    #[must_use]
    pub fn ink_in(&self, resource_id: &str, chapter_key: &str) -> Vec<&InkMark> {
        self.annotations
            .ink
            .iter()
            .filter(|k| k.resource_id == resource_id && k.in_chapter(chapter_key))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::tests::fixture_session;
    use crate::commands::{CommandArgs, CommandContext, Invocation};

    fn shell() -> Shell {
        Shell::new(fixture_session()).unwrap()
    }

    fn stroke() -> openlogos_ink::Stroke {
        serde_json::from_value(serde_json::json!({
            "id": "",
            "tool": "pen",
            "brush_version": "pf-1.2.3/pen-1",
            "width": 4.0,
            "color": "#1F6FEB",
            "opacity": 1.0,
            "points": [
                {"x": 10.0, "y": -2.5, "pressure": null, "elapsed_ms": 0.0},
                {"x": 30.0, "y": 4.0, "pressure": 0.4, "elapsed_ms": 16.0}
            ]
        }))
        .unwrap()
    }

    fn highlight(segment: &str) -> Highlight {
        Highlight {
            id: String::new(),
            resource_id: "bsb".into(),
            content_revision: "bsb-2024".into(),
            spans: vec![SegmentSpan {
                segment_id: segment.into(),
                start: 20,
                end: 44,
            }],
            color: "#f5d90a".into(),
        }
    }

    #[test]
    fn a_store_round_trips_the_session_and_the_annotations() {
        let mut a = shell();
        let context = CommandContext::default();
        a.apply(
            &Invocation::with(
                "tab.record_scroll",
                CommandArgs {
                    tab_id: Some("t1".into()),
                    segment_id: Some("John.3.17".into()),
                    scalar: Some(9),
                    ..CommandArgs::default()
                },
            ),
            &context,
        )
        .unwrap();
        a.apply(&Invocation::new("tool.pen"), &context).unwrap();
        a.put_note(Note {
            id: "fixture:note:1".into(),
            blocks: vec![NoteBlock {
                kind: NoteBlockKind::Heading,
                spans: vec![
                    NoteSpan {
                        text: "Born again ".into(),
                        bold: true,
                        ..NoteSpan::default()
                    },
                    NoteSpan {
                        text: "Jn 3:3".into(),
                        link: Some("John.3.3".into()),
                        ..NoteSpan::default()
                    },
                ],
            }],
        })
        .unwrap();
        let h = a.add_highlight(highlight("John.3.16"), None).unwrap();
        let k = a
            .add_ink(InkMark {
                id: String::new(),
                resource_id: "bsb".into(),
                segment_id: "John.3.17".into(),
                stroke: stroke(),
            })
            .unwrap();
        assert_eq!((h.as_str(), k.as_str()), ("h1", "k2"));
        let json = a.export_store_json();
        assert_eq!(
            serde_json::from_str::<UserStore>(&json).unwrap(),
            a.export_store()
        );

        let mut b = shell();
        let report = b.import_store_json(&json).unwrap();
        assert!(report.issues.is_empty(), "{report:?}");
        assert_eq!(b.session, a.session);
        assert_eq!(b.annotations, a.annotations);
        assert_eq!(b.export_store_json(), json);
        assert_eq!(b.highlights_in("bsb", "John.3").len(), 1);
        assert!(b.highlights_in("bsb", "John.31").is_empty());
        assert_eq!(b.ink_in("bsb", "John.3")[0].stroke.id, "k2");
    }

    #[test]
    fn ink_is_stored_as_rounded_delta_columns_and_read_back_exactly() {
        let mut s = shell();
        let mut raw = stroke();
        raw.points.push(openlogos_ink::InkPoint {
            x: 12.34,
            y: 7.06,
            pressure: None,
            elapsed_ms: 20.6,
            altitude: Some(1.234),
            azimuth: None,
        });
        raw.points.push(openlogos_ink::InkPoint {
            x: -4.0,
            y: 7.0,
            pressure: Some(0.555),
            elapsed_ms: 31.0,
            altitude: None,
            azimuth: None,
        });
        s.add_ink(InkMark {
            id: String::new(),
            resource_id: "bsb".into(),
            segment_id: "John.3.16".into(),
            stroke: raw,
        })
        .unwrap();
        let stored = StoredInk::from(&s.annotations.ink[0]);
        assert_eq!(stored.xy, vec![100, -25, 200, 65, -177, 31, -163, -1]);
        assert_eq!(stored.t, vec![0, 16, 5, 10]);
        assert_eq!(stored.pressure, Some(vec![None, Some(40), None, Some(16)]));
        assert_eq!(stored.altitude, Some(vec![None, None, Some(123), None]));
        assert_eq!(stored.azimuth, None);
        let back = InkMark::try_from(stored).unwrap();
        assert_eq!(back, s.annotations.ink[0]);
        assert_eq!(back.stroke.points[2].x, 12.3);
        let json = s.export_store_json();
        let mut t = shell();
        t.import_store_json(&json).unwrap();
        assert_eq!(t.annotations, s.annotations);
    }

    #[test]
    fn refused_documents_leave_the_shell_alone() {
        let mut s = shell();
        let before = s.clone();
        for (input, kind) in [
            ("", "empty"),
            ("{\"format\": \"openlogos-user-store\", ", "not_json"),
            ("[1, 2]", "not_a_store"),
            ("{\"format\": \"something-else\"}", "not_a_store"),
            (
                "{\"format\": \"openlogos-user-store\", \"schema_version\": \"2.0\"}",
                "unsupported_version",
            ),
        ] {
            let error = s.import_store_json(input).unwrap_err();
            assert_eq!(
                serde_json::to_value(&error).unwrap()["kind"],
                kind,
                "{input}"
            );
        }
        assert_eq!(s, before);
    }

    #[test]
    fn bad_parts_are_set_aside_and_reported() {
        let mut a = shell();
        a.add_highlight(highlight("John.3.16"), None).unwrap();
        let mut doc: Value = serde_json::from_str(&a.export_store_json()).unwrap();
        doc["session"]["active_workspace_id"] = "nowhere".into();
        doc["highlights"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"id": "h9", "resource_id": "bsb", "content_revision": "x", "spans": [], "color": "#FFFFFF"}));
        let mut b = shell();
        b.session.workspaces.truncate(1);
        b.session.active_workspace_id = b.session.workspaces[0].id.clone();
        let kept = b.session.clone();
        let report = b.import_store_json(&doc.to_string()).unwrap();
        assert!(!report.session_restored);
        assert_eq!(b.session, kept);
        assert_eq!(report.highlights, 1);
        let parts: Vec<_> = report.issues.iter().map(|i| i.part).collect();
        assert_eq!(parts, vec![StorePart::Session, StorePart::Highlight]);
    }

    #[test]
    fn placeholder_text_is_never_highlighted() {
        let mut s = shell();
        let check = |_: &str, segment: &str| segment == "John.3.1";
        let error = s
            .add_highlight(highlight("John.3.1"), Some(&check))
            .unwrap_err();
        assert_eq!(
            error,
            StoreError::PlaceholderText {
                segment_id: "John.3.1".into()
            }
        );
        assert!(
            s.add_highlight(highlight("John.3.16"), Some(&check))
                .is_ok()
        );
    }

    #[test]
    fn changes_count_quiet_commands_and_annotations() {
        let mut s = shell();
        let context = CommandContext::default();
        s.apply(
            &Invocation::with(
                "tab.record_scroll",
                CommandArgs {
                    tab_id: Some("t1".into()),
                    segment_id: Some("John.3.16".into()),
                    scalar: Some(3),
                    ..CommandArgs::default()
                },
            ),
            &context,
        )
        .unwrap();
        assert_eq!((s.revision, s.changes), (0, 1));
        s.put_note(Note::from_text("n1", "hello")).unwrap();
        s.put_note(Note::from_text("n1", "hello")).unwrap();
        assert_eq!(s.changes, 2);
        assert_eq!(s.note("n1").unwrap().plain_text(), "hello");
        assert!(s.delete_note("n2").is_err());
    }
}
