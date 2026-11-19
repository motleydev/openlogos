//! The B016 importer: Calvin's *Institutes of the Christian Religion* in John Allen's
//! translation, from the two plain-text volumes of the digitizer (rights record S75), into one
//! work pack ([`openlogos_retrieval_sqlite::work_pack`]). `just institutes-pack` runs it.
//!
//! - this module: the two volumes into front matter, books, arguments, chapters, numbered
//!   sections and notes, keeping the text as printed; every gap, duplicate or misnumbering is
//!   reported, never repaired;
//! - [`citations`]: Scripture citations in the notes, through the core reference parser;
//! - [`build`]: the rights gate, the pack, the report.
//!
//! What the text is, in one place (docs/evidence/B016.md has the counts):
//!
//! 1. **Kept as printed**: every scalar of the work between the digitizer's start and end
//!    lines, except what 2 to 6 list. Italics stay as the digitizer marked them (`_cultus_`),
//!    the translator's bracketed words stay, Greek and Latin stay, nothing is normalized.
//! 2. **Line wrapping** is the digitizer's: a paragraph is a run of non-blank lines; its lines
//!    are joined with one space, except after a line ending in U+2010 HYPHEN (a compound broken
//!    at the line end, `self‐` + `existence`), which joins with nothing. A paragraph whose every
//!    line is indented is a block (verse, a centred line, a salutation): its lines are trimmed
//!    and joined with a line break, since a verse's line breaks cannot be told from a wrap.
//! 3. **Note markers** (`(61)` in volume I, `[703]` in volume II) leave the text and become the
//!    note's anchor: the paragraph and scalar offset where the marker stood. Only the next
//!    expected number is a marker; anything else stays text and is reported.
//! 4. **Notes** are kept whole as notes of the section their marker is in (a citation note or
//!    the translator's or editor's comment alike); their Scripture citations become links.
//! 5. **Left out**: the digitizer's header and licence (before the start line and after the end
//!    line), its table of contents (volume I), the cover-art line and the transcriber's notes
//!    (listed in the report), volume II's repeated `BOOK III.` heading, and the two indexes at
//!    the end of volume II, which cite pages of the print edition that the pack does not carry.
//! 6. **Headings inside a chapter** (`The First Commandment.`, `CONFIRMATION.`) and what follows
//!    them up to the next numbered section belong to that next section.

pub mod build;
pub mod citations;

use std::collections::BTreeMap;

use openlogos_retrieval_sqlite::work_pack::{
    ChapterKind, DivisionKind, ParagraphStyle, WorkChapterInfo, WorkDivision, WorkNote,
    WorkParagraph, WorkSection,
};
use serde::Serialize;

use crate::model::{ImportError, parse_error};

/// The digitizer's start and end lines begin with these.
const START_LINE: &str = "*** START OF THE PROJECT GUTENBERG EBOOK";
const END_LINE: &str = "*** END OF THE PROJECT GUTENBERG EBOOK";

/// U+2010 HYPHEN, which the digitizer keeps at a line end inside a compound.
const HYPHEN: char = '\u{2010}';

/// Volume I's front matter: the heading as printed (lines joined) and the locator it gets.
const FRONT_V1: [(&str, &str); 5] = [
    ("ADVERTISEMENT.", "advertisement"),
    ("THE TRANSLATOR’S PREFACE.", "translator-preface"),
    (
        "THE AUTHOR’S PREFACE TO AN EDITION PUBLISHED IN THE YEAR 1559, WITH HIS LAST CORRECTIONS AND ADDITIONS.",
        "author-preface",
    ),
    ("DEDICATION.", "dedication"),
    ("GENERAL SYLLABUS.", "syllabus"),
];

/// One line of a file: its 1-based number and text without the line ending.
#[derive(Debug, Clone)]
struct Line {
    number: usize,
    text: String,
}

/// A run of non-blank lines.
#[derive(Debug, Clone)]
struct Para {
    blank_before: usize,
    lines: Vec<Line>,
}

impl Para {
    fn first(&self) -> &str {
        &self.lines[0].text
    }

    fn line_number(&self) -> usize {
        self.lines[0].number
    }

    fn trimmed(&self) -> Vec<&str> {
        self.lines.iter().map(|l| l.text.trim()).collect()
    }

    /// Lines trimmed and joined with spaces: headings.
    fn joined(&self) -> String {
        self.trimmed().join(" ")
    }

    fn indented(&self) -> bool {
        self.lines.iter().all(|l| l.text.starts_with(' '))
    }
}

/// One file split at the digitizer's start and end lines.
#[derive(Debug, Clone)]
pub struct DigitizedFile {
    /// `pg45001.txt`.
    pub name: String,
    /// `Title:`, `Author:`, `Translator:`, `Release date:`, `Most recently updated:`,
    /// `Credits:`, `Language:` of the header.
    pub header: BTreeMap<String, String>,
    /// Lines before the start line, which are left out.
    pub header_lines: usize,
    /// Lines from the end line on, which are left out.
    pub trailer_lines: usize,
    /// Lines that ended in CR LF.
    pub crlf_lines: usize,
    body: Vec<Line>,
}

/// Splits a file into header, body and licence trailer.
pub fn split_digitized(name: &str, text: &str) -> Result<DigitizedFile, ImportError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = Vec::new();
    let mut crlf_lines = 0;
    for (i, raw) in text.split('\n').enumerate() {
        let line = match raw.strip_suffix('\r') {
            Some(l) => {
                crlf_lines += 1;
                l
            }
            None => raw,
        };
        lines.push(Line {
            number: i + 1,
            text: line.to_owned(),
        });
    }
    let start = lines
        .iter()
        .position(|l| l.text.starts_with(START_LINE))
        .ok_or_else(|| parse_error(name, "no start line"))?;
    let end = lines
        .iter()
        .position(|l| l.text.starts_with(END_LINE))
        .ok_or_else(|| parse_error(name, "no end line"))?;
    if end <= start {
        return Err(parse_error(
            name,
            "the end line comes before the start line",
        ));
    }
    let mut header = BTreeMap::new();
    for line in &lines[..start] {
        if let Some((key, value)) = line.text.trim().split_once(": ")
            && [
                "Title",
                "Author",
                "Translator",
                "Release date",
                "Most recently updated",
                "Credits",
                "Language",
            ]
            .contains(&key)
        {
            header.insert(key.to_owned(), value.trim().to_owned());
        }
    }
    Ok(DigitizedFile {
        name: name.to_owned(),
        header,
        header_lines: start + 1,
        trailer_lines: lines.len() - end,
        crlf_lines,
        body: lines[start + 1..end].to_vec(),
    })
}

fn paragraphs(lines: &[Line]) -> Vec<Para> {
    let mut out = Vec::new();
    let mut current: Vec<Line> = Vec::new();
    let mut blank = 0;
    let mut blank_before = 0;
    for line in lines {
        if line.text.trim().is_empty() {
            if !current.is_empty() {
                out.push(Para {
                    blank_before,
                    lines: std::mem::take(&mut current),
                });
            }
            blank += 1;
        } else {
            if current.is_empty() {
                blank_before = blank;
            }
            current.push(line.clone());
            blank = 0;
        }
    }
    if !current.is_empty() {
        out.push(Para {
            blank_before,
            lines: current,
        });
    }
    out
}

/// The value of a canonical upper-case roman numeral; `None` for anything else (`IIII`).
#[must_use]
pub fn roman(text: &str) -> Option<u32> {
    if text.is_empty() || text.len() > 12 {
        return None;
    }
    let value = |c: char| match c.to_ascii_uppercase() {
        'I' => Some(1),
        'V' => Some(5),
        'X' => Some(10),
        'L' => Some(50),
        'C' => Some(100),
        _ => None,
    };
    let digits: Vec<u32> = text.chars().map(value).collect::<Option<_>>()?;
    let mut total = 0;
    for (i, d) in digits.iter().enumerate() {
        if digits.get(i + 1).is_some_and(|next| next > d) {
            total -= *d as i64;
        } else {
            total += *d as i64;
        }
    }
    let total = u32::try_from(total).ok().filter(|t| *t > 0)?;
    (to_roman(total) == text.to_ascii_uppercase()).then_some(total)
}

fn to_roman(mut n: u32) -> String {
    let table = [
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut out = String::new();
    for (value, letters) in table {
        while n >= value {
            out.push_str(letters);
            n -= value;
        }
    }
    out
}

/// `II. On the other hand` → `(2, "II.")`: a section number at the start of a paragraph.
fn section_number(first_line: &str) -> Option<(u32, String)> {
    let (numeral, rest) = first_line.split_once(". ")?;
    if numeral.is_empty() || !numeral.chars().all(|c| "IVXLC".contains(c)) || rest.is_empty() {
        return None;
    }
    roman(numeral).map(|n| (n, format!("{numeral}.")))
}

/// Something the report lists: an anomaly of the source or a place left out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    /// `pg45001.txt`.
    pub file: String,
    /// 1-based line.
    pub line: usize,
    /// `gap`, `duplicate`, `out_of_order`, `marker_not_next`, …
    pub kind: String,
    /// What was seen.
    pub detail: String,
}

/// What reconstructing the paragraphs did, counted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct TextCounters {
    /// Lines joined with one space.
    pub joined_with_space: usize,
    /// Lines joined with nothing after U+2010 HYPHEN.
    pub joined_after_hyphen: usize,
    /// Prose paragraphs.
    pub prose_paragraphs: usize,
    /// Block paragraphs (every line indented), their line breaks kept.
    pub block_paragraphs: usize,
    /// Headings inside chapters.
    pub headings: usize,
    /// Note markers removed from the text and kept as anchors.
    pub markers: usize,
    /// Paragraphs that held a marker-shaped number that was not the next note.
    pub stray_markers: usize,
}

/// Lines and regions left out, by reason.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LeftOut {
    /// Reason → lines.
    pub lines: BTreeMap<String, usize>,
    /// The transcriber's notes, word for word.
    pub transcriber_notes: Vec<String>,
}

/// The parsed work, before citations.
#[derive(Debug, Clone)]
pub struct ParsedWork {
    /// Divisions and chapters.
    pub structure: Vec<WorkDivision>,
    /// Sections in reading order with notes anchored (note kinds are set by the citation step).
    pub sections: Vec<WorkSection>,
    /// Anomalies of the source, none repaired.
    pub findings: Vec<Finding>,
    /// Reconstruction counters.
    pub counters: TextCounters,
    /// What was left out.
    pub left_out: LeftOut,
    /// Notes per volume, from the notes' own numbering.
    pub notes_per_volume: [usize; 2],
}

/// A marker taken out of a paragraph: volume, note number, the marker as printed, scalar offset.
type Anchor = (u32, u32, String, u32);

/// A note's text as collected, before anchoring.
#[derive(Debug, Clone)]
struct RawNote {
    text: String,
    line: usize,
}

/// One section being assembled.
#[derive(Debug, Clone)]
struct OpenSection {
    locator: String,
    chapter: String,
    number: Option<u32>,
    label: Option<String>,
    paragraphs: Vec<WorkParagraph>,
    /// (volume, number, marker as printed, paragraph, scalar)
    anchors: Vec<(u32, u32, String, u32, u32)>,
}

struct Builder {
    structure: Vec<WorkDivision>,
    sections: Vec<OpenSection>,
    findings: Vec<Finding>,
    counters: TextCounters,
    left_out: LeftOut,
    /// Paragraphs after a heading inside a chapter, waiting for the next numbered section.
    pending: Vec<(WorkParagraph, Vec<Anchor>)>,
    pending_line: usize,
    /// The next note number per volume.
    next_note: [u32; 2],
    /// The section number expected next in the open chapter.
    expected_section: u32,
}

impl Builder {
    fn finding(&mut self, file: &str, line: usize, kind: &str, detail: impl Into<String>) {
        self.findings.push(Finding {
            file: file.to_owned(),
            line,
            kind: kind.to_owned(),
            detail: detail.into(),
        });
    }

    fn leave_out(&mut self, reason: &str, lines: usize) {
        *self.left_out.lines.entry(reason.to_owned()).or_default() += lines;
    }

    fn division(
        &mut self,
        locator: &str,
        kind: DivisionKind,
        label: Option<String>,
        title: Option<String>,
    ) {
        self.structure.push(WorkDivision {
            locator: locator.to_owned(),
            kind,
            label,
            title,
            chapters: Vec::new(),
        });
    }

    fn current_division(&self) -> &str {
        self.structure.last().map_or("", |d| d.locator.as_str())
    }

    /// Opens a chapter-level unit in the current division.
    fn chapter(
        &mut self,
        locator: &str,
        kind: ChapterKind,
        label: Option<String>,
        title: Option<String>,
    ) {
        self.flush_pending(None);
        if let Some(division) = self.structure.last_mut() {
            division.chapters.push(WorkChapterInfo {
                locator: locator.to_owned(),
                kind,
                label,
                title,
                sections: Vec::new(),
            });
        }
        self.expected_section = 1;
    }

    fn current_chapter(&self) -> Option<&WorkChapterInfo> {
        self.structure.last().and_then(|d| d.chapters.last())
    }

    fn open_section(&mut self, locator: String, number: Option<u32>, label: Option<String>) {
        let chapter = self
            .current_chapter()
            .map(|c| c.locator.clone())
            .unwrap_or_default();
        if let Some(division) = self.structure.last_mut()
            && let Some(c) = division.chapters.last_mut()
        {
            c.sections.push(locator.clone());
        }
        self.sections.push(OpenSection {
            locator,
            chapter,
            number,
            label,
            paragraphs: Vec::new(),
            anchors: Vec::new(),
        });
    }

    /// Moves waiting paragraphs into `into` (the next section), or into the last section when
    /// the chapter ended first (reported).
    fn flush_pending(&mut self, file_line: Option<(&str, usize)>) {
        if self.pending.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.pending);
        if file_line.is_none() {
            let line = self.pending_line;
            self.finding(
                "",
                line,
                "heading_without_section",
                "a heading inside a chapter with no numbered section after it; kept in the last section",
            );
        }
        let Some(section) = self.sections.last_mut() else {
            return;
        };
        for (paragraph, anchors) in pending {
            let index = section.paragraphs.len() as u32;
            section.paragraphs.push(paragraph);
            for (volume, number, marker, scalar) in anchors {
                section
                    .anchors
                    .push((volume, number, marker, index, scalar));
            }
        }
    }

    /// Builds a paragraph's text and takes out the volume's next note markers.
    fn paragraph(
        &mut self,
        file: &str,
        volume: u32,
        para: &Para,
        style: ParagraphStyle,
    ) -> (WorkParagraph, Vec<Anchor>) {
        let text = match style {
            ParagraphStyle::Block => {
                self.counters.block_paragraphs += 1;
                para.trimmed().join("\n")
            }
            ParagraphStyle::Prose | ParagraphStyle::Heading => {
                if style == ParagraphStyle::Prose {
                    self.counters.prose_paragraphs += 1;
                } else {
                    self.counters.headings += 1;
                }
                let mut out = String::new();
                for (i, line) in para.lines.iter().enumerate() {
                    let piece = line.text.trim();
                    if i > 0 {
                        if out.ends_with(HYPHEN) {
                            self.counters.joined_after_hyphen += 1;
                        } else {
                            out.push(' ');
                            self.counters.joined_with_space += 1;
                        }
                    }
                    out.push_str(piece);
                }
                out
            }
        };
        let (open, close) = if volume == 1 { ('(', ')') } else { ('[', ']') };
        let mut kept = String::with_capacity(text.len());
        let mut anchors = Vec::new();
        let mut rest = text.as_str();
        let mut scalars = 0u32;
        let mut stray = false;
        while let Some(at) = rest.find(open) {
            let (before, from) = rest.split_at(at);
            kept.push_str(before);
            scalars += before.chars().count() as u32;
            let inner_end = from[1..].find(close).map(|i| i + 1);
            let digits = inner_end.map(|e| &from[1..e]);
            match digits {
                Some(d)
                    if !d.is_empty() && d.len() <= 5 && d.chars().all(|c| c.is_ascii_digit()) =>
                {
                    let number: u32 = d.parse().unwrap_or(0);
                    let next = &mut self.next_note[volume as usize - 1];
                    let marker = &from[..inner_end.unwrap_or(0) + 1];
                    if number == *next {
                        *next += 1;
                        anchors.push((volume, number, marker.to_owned(), scalars));
                        self.counters.markers += 1;
                        rest = &from[marker.len()..];
                        continue;
                    }
                    stray = true;
                    kept.push_str(marker);
                    scalars += marker.chars().count() as u32;
                    rest = &from[marker.len()..];
                }
                _ => {
                    kept.push(open);
                    scalars += 1;
                    rest = &from[open.len_utf8()..];
                }
            }
        }
        kept.push_str(rest);
        if stray {
            self.counters.stray_markers += 1;
            self.finding(
                file,
                para.line_number(),
                "marker_not_next",
                format!(
                    "a number in {open}{close} that is not note {} stays in the text",
                    self.next_note[volume as usize - 1]
                ),
            );
        }
        if kept.contains("  ") {
            self.finding(
                file,
                para.line_number(),
                "double_space",
                "two spaces in a row after taking out a marker or joining lines",
            );
        }
        (WorkParagraph { style, text: kept }, anchors)
    }

    /// Adds a content paragraph to the open section, or opens a numbered section.
    fn content(
        &mut self,
        file: &str,
        volume: u32,
        para: &Para,
        in_chapter: bool,
    ) -> Result<(), ImportError> {
        let style = if para.indented() {
            ParagraphStyle::Block
        } else {
            ParagraphStyle::Prose
        };
        if in_chapter
            && style == ParagraphStyle::Prose
            && let Some((number, label)) = section_number(para.first())
        {
            let chapter = self
                .current_chapter()
                .map(|c| c.locator.clone())
                .unwrap_or_default();
            let expected = self.expected_section;
            if number != expected {
                let kind = if number < expected {
                    if self
                        .sections
                        .iter()
                        .any(|s| s.chapter == chapter && s.number == Some(number))
                    {
                        "duplicate"
                    } else {
                        "out_of_order"
                    }
                } else {
                    "gap"
                };
                self.finding(
                    file,
                    para.line_number(),
                    kind,
                    format!("{chapter}: section {label} where {expected} was expected"),
                );
            }
            let locator = format!("{chapter}.{number}");
            if self.sections.iter().any(|s| s.locator == locator) {
                return Err(parse_error(
                    file,
                    format!(
                        "line {}: a second section {locator}; the importer will not renumber it",
                        para.line_number()
                    ),
                ));
            }
            self.expected_section = number + 1;
            self.open_section(locator, Some(number), Some(label));
            self.flush_pending(Some((file, para.line_number())));
        } else if in_chapter && self.expected_section == 1 {
            // The chapter's first section is not numbered in the source.
            let chapter = self
                .current_chapter()
                .map(|c| c.locator.clone())
                .unwrap_or_default();
            self.expected_section = 2;
            self.open_section(format!("{chapter}.1"), Some(1), None);
            self.flush_pending(Some((file, para.line_number())));
        } else if !self.pending.is_empty() {
            let built = self.paragraph(file, volume, para, style);
            self.pending.push(built);
            return Ok(());
        }
        let (paragraph, anchors) = self.paragraph(file, volume, para, style);
        let section = self.sections.last_mut().ok_or_else(|| {
            parse_error(
                file,
                format!("line {}: text before any section", para.line_number()),
            )
        })?;
        let index = section.paragraphs.len() as u32;
        section.paragraphs.push(paragraph);
        for (volume, number, marker, scalar) in anchors {
            section
                .anchors
                .push((volume, number, marker, index, scalar));
        }
        Ok(())
    }

    /// A heading inside a chapter: it and what follows wait for the next numbered section.
    fn heading(&mut self, file: &str, volume: u32, para: &Para) {
        let built = self.paragraph(file, volume, para, ParagraphStyle::Heading);
        if self.pending.is_empty() {
            self.pending_line = para.line_number();
        }
        self.pending.push(built);
    }

    /// A single-section unit (front matter, an argument) whose paragraphs follow.
    fn unit(
        &mut self,
        locator: &str,
        kind: ChapterKind,
        label: Option<String>,
        title: Option<String>,
    ) {
        self.chapter(locator, kind, label, title);
        self.open_section(locator.to_owned(), None, None);
    }
}

fn book_heading(text: &str) -> Option<(u32, String, Option<String>)> {
    let rest = text.strip_prefix("BOOK ")?;
    let (numeral, title) = rest.split_once('.')?;
    let number = roman(numeral)?;
    let title = title.trim();
    Some((
        number,
        format!("BOOK {numeral}."),
        (!title.is_empty()).then(|| title.to_owned()),
    ))
}

fn chapter_heading(text: &str, word: &str) -> Option<(u32, String, Option<String>)> {
    let rest = text.strip_prefix(word)?.strip_prefix(' ')?;
    let (numeral, title) = rest.split_once('.')?;
    let number = roman(numeral)?;
    let title = title.trim();
    Some((
        number,
        format!("{word} {numeral}."),
        (!title.is_empty()).then(|| title.to_owned()),
    ))
}

/// Volume I's notes: `    61 Acts xvii. 28.` with continuation lines indented.
fn notes_v1(
    file: &str,
    paras: &[Para],
    builder: &mut Builder,
) -> Result<Vec<RawNote>, ImportError> {
    let mut notes: Vec<RawNote> = Vec::new();
    for para in paras {
        let first = para.first().trim_start();
        let (number, rest) = first
            .split_once(' ')
            .filter(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
            .ok_or_else(|| {
                parse_error(
                    file,
                    format!("line {}: a note without a number", para.line_number()),
                )
            })?;
        let number: usize = number.parse().unwrap_or(0);
        if number != notes.len() + 1 {
            builder.finding(
                file,
                para.line_number(),
                "note_numbering",
                format!("note {number} where {} was expected", notes.len() + 1),
            );
        }
        let mut lines = vec![rest.trim().to_owned()];
        lines.extend(para.lines[1..].iter().map(|l| l.text.trim().to_owned()));
        notes.push(RawNote {
            text: join_note_lines(&lines),
            line: para.line_number(),
        });
    }
    Ok(notes)
}

fn join_note_lines(lines: &[String]) -> String {
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 && !out.ends_with(HYPHEN) {
            out.push(' ');
        }
        out.push_str(line);
    }
    out
}

/// Parses both volumes.
pub fn parse_volumes(v1: &DigitizedFile, v2: &DigitizedFile) -> Result<ParsedWork, ImportError> {
    let mut b = Builder {
        structure: Vec::new(),
        sections: Vec::new(),
        findings: Vec::new(),
        counters: TextCounters::default(),
        left_out: LeftOut::default(),
        pending: Vec::new(),
        pending_line: 0,
        next_note: [1, 1],
        expected_section: 1,
    };
    b.leave_out(
        "digitizer header (both files)",
        v1.header_lines + v2.header_lines,
    );
    b.leave_out(
        "digitizer licence and trailer (both files)",
        v1.trailer_lines + v2.trailer_lines,
    );
    let mut notes: [Vec<RawNote>; 2] = [Vec::new(), Vec::new()];

    // ---- Volume I.
    let file = v1.name.as_str();
    let paras = paragraphs(&v1.body);
    b.division("front", DivisionKind::Front, None, None);
    let contents = paras
        .iter()
        .position(|p| p.joined() == "CONTENTS")
        .ok_or_else(|| parse_error(file, "no CONTENTS"))?;
    b.unit(
        "title.1",
        ChapterKind::Front,
        None,
        Some("Title page, volume I".to_owned()),
    );
    for para in &paras[..contents] {
        b.content(file, 1, para, false)?;
    }
    let mut i = contents;
    // The digitizer's table of contents, cover line and note, to the first front heading.
    while i < paras.len() && paras[i].joined() != FRONT_V1[0].0 {
        let para = &paras[i];
        let text = para.joined();
        if text.starts_with("[Transcriber") {
            b.left_out.transcriber_notes.push(text.clone());
            b.leave_out("transcriber's note", para.lines.len());
        } else if text == "[Cover Art]" {
            b.leave_out("cover-art line", para.lines.len());
        } else {
            b.leave_out("digitizer's table of contents (volume I)", para.lines.len());
        }
        i += 1;
    }
    let mut front = 0;
    let mut in_chapter = false;
    let mut book_open = false;
    while i < paras.len() {
        let para = &paras[i];
        let text = para.joined();
        i += 1;
        if text == "FOOTNOTES" {
            notes[0] = notes_v1(file, &paras[i..], &mut b)?;
            i = paras.len();
            continue;
        }
        if !book_open && front < FRONT_V1.len() && text == FRONT_V1[front].0 {
            b.unit(
                FRONT_V1[front].1,
                ChapterKind::Front,
                None,
                Some(text.clone()),
            );
            front += 1;
            continue;
        }
        if let Some((number, label, title)) = book_heading(&text) {
            if front != FRONT_V1.len() {
                return Err(parse_error(
                    file,
                    format!(
                        "line {}: a book before the front matter ended",
                        para.line_number()
                    ),
                ));
            }
            b.flush_pending(None);
            b.division(&number.to_string(), DivisionKind::Book, Some(label), title);
            book_open = true;
            in_chapter = false;
            continue;
        }
        if book_open && text == "Argument." {
            let book = b.current_division().to_owned();
            b.unit(
                &format!("{book}.argument"),
                ChapterKind::Argument,
                Some(text),
                None,
            );
            in_chapter = false;
            continue;
        }
        if book_open && let Some((number, label, title)) = chapter_heading(&text, "Chapter") {
            let book = b.current_division().to_owned();
            b.chapter(
                &format!("{book}.{number}"),
                ChapterKind::Chapter,
                Some(label),
                title,
            );
            in_chapter = true;
            continue;
        }
        if in_chapter && para.blank_before == 3 && !para.indented() {
            b.heading(file, 1, para);
            continue;
        }
        if !book_open && front == 0 {
            return Err(parse_error(
                file,
                format!(
                    "line {}: text before the front matter: {text:.60}",
                    para.line_number()
                ),
            ));
        }
        b.content(file, 1, para, in_chapter)?;
    }
    b.flush_pending(None);
    if front != FRONT_V1.len() {
        return Err(parse_error(
            file,
            format!("front matter: {front} of {} headings found", FRONT_V1.len()),
        ));
    }

    // ---- Volume II.
    let file = v2.name.as_str();
    let paras = paragraphs(&v2.body);
    let book3 = paras
        .iter()
        .position(|p| p.joined() == "BOOK III.")
        .ok_or_else(|| parse_error(file, "no BOOK III. heading"))?;
    // Volume II's title page joins the front matter, after volume I's.
    let front_division = b
        .structure
        .iter()
        .position(|d| d.kind == DivisionKind::Front)
        .ok_or_else(|| parse_error(file, "no front matter"))?;
    {
        // Title page of volume II: a unit appended to the front division, read in place.
        let mut title_paras = Vec::new();
        for para in &paras[..book3] {
            title_paras.push(para.clone());
        }
        let ordinal_slot = b
            .sections
            .iter()
            .position(|s| s.chapter == "syllabus")
            .map_or(b.sections.len(), |p| p + 1);
        let mut unit = OpenSection {
            locator: "title.2".to_owned(),
            chapter: "title.2".to_owned(),
            number: None,
            label: None,
            paragraphs: Vec::new(),
            anchors: Vec::new(),
        };
        for para in &title_paras {
            let style = if para.indented() {
                ParagraphStyle::Block
            } else {
                ParagraphStyle::Prose
            };
            let (paragraph, anchors) = b.paragraph(file, 2, para, style);
            if !anchors.is_empty() {
                return Err(parse_error(file, "a note marker on the title page"));
            }
            unit.paragraphs.push(paragraph);
        }
        b.sections.insert(ordinal_slot, unit);
        b.structure[front_division].chapters.push(WorkChapterInfo {
            locator: "title.2".to_owned(),
            kind: ChapterKind::Front,
            label: None,
            title: Some("Title page, volume II".to_owned()),
            sections: vec!["title.2".to_owned()],
        });
    }
    b.leave_out(
        "repeated BOOK III. heading (volume II)",
        paras[book3].lines.len(),
    );
    let book3_division = b
        .structure
        .iter()
        .position(|d| d.locator == "3")
        .ok_or_else(|| parse_error(file, "volume I has no book III to continue"))?;
    if book3_division + 1 != b.structure.len() {
        return Err(parse_error(
            file,
            "book III is not the last division of volume I",
        ));
    }
    let mut i = book3 + 1;
    let mut in_chapter = false;
    let mut in_argument = false;
    let mut note: Option<(u32, usize, Vec<String>)> = None;
    let finish_note = |note: &mut Option<(u32, usize, Vec<String>)>,
                       notes: &mut Vec<RawNote>,
                       b: &mut Builder| {
        if let Some((number, line, lines)) = note.take() {
            if number as usize != notes.len() + 1 {
                b.finding(
                    file,
                    line,
                    "note_numbering",
                    format!("note {number} where {} was expected", notes.len() + 1),
                );
            }
            if lines.is_empty() {
                b.finding(
                    file,
                    line,
                    "empty_note",
                    format!("note {number} has no text"),
                );
            }
            notes.push(RawNote {
                text: join_note_lines(&lines),
                line,
            });
        }
    };
    while i < paras.len() {
        let para = &paras[i];
        let text = para.joined();
        i += 1;
        if let Some(number) = text
            .strip_prefix("Footnote ")
            .and_then(|r| r.strip_suffix(':'))
            .and_then(|n| n.parse::<u32>().ok())
        {
            finish_note(&mut note, &mut notes[1], &mut b);
            note = Some((number, para.line_number(), Vec::new()));
            continue;
        }
        if let Some((_, _, lines)) = note.as_mut() {
            if para
                .lines
                .iter()
                .all(|l| l.text.starts_with("  ") && !l.text.starts_with("   "))
            {
                if !lines.is_empty() {
                    // A second paragraph of one note: kept, joined with a space.
                    b.finding(
                        file,
                        para.line_number(),
                        "note_paragraphs",
                        "a note of more than one paragraph; joined with a space",
                    );
                }
                lines.extend(para.lines.iter().map(|l| l.text.trim().to_owned()));
                continue;
            }
            finish_note(&mut note, &mut notes[1], &mut b);
        }
        if text == "INDEX OF THE PRINCIPAL MATTERS." {
            // Both indexes and the transcriber's notes: left out.
            for rest in &paras[i - 1..] {
                let joined = rest.joined();
                if joined.starts_with("● Transcriber") {
                    b.left_out.transcriber_notes.push(
                        rest.lines
                            .iter()
                            .map(|l| l.text.trim())
                            .collect::<Vec<_>>()
                            .join("\n"),
                    );
                    b.leave_out("transcriber's notes (volume II)", rest.lines.len());
                } else {
                    b.leave_out(
                        "indexes of matters and of Scripture (volume II, cite print pages)",
                        rest.lines.len(),
                    );
                }
            }
            i = paras.len();
            continue;
        }
        let first = para.first().trim();
        if let Some((number, label, _)) = book_heading(first) {
            b.flush_pending(None);
            let title = para.lines[1..]
                .iter()
                .map(|l| l.text.trim())
                .collect::<Vec<_>>()
                .join(" ");
            b.division(
                &number.to_string(),
                DivisionKind::Book,
                Some(label),
                (!title.is_empty()).then_some(title),
            );
            in_chapter = false;
            in_argument = false;
            continue;
        }
        if text == "ARGUMENT." {
            let book = b.current_division().to_owned();
            b.unit(
                &format!("{book}.argument"),
                ChapterKind::Argument,
                Some(text),
                None,
            );
            in_chapter = false;
            in_argument = true;
            continue;
        }
        if let Some((number, label, _)) = chapter_heading(first, "CHAPTER")
            && first == label
        {
            let book = b.current_division().to_owned();
            let title = para.lines[1..]
                .iter()
                .map(|l| l.text.trim())
                .collect::<Vec<_>>()
                .join(" ");
            b.chapter(
                &format!("{book}.{number}"),
                ChapterKind::Chapter,
                Some(label),
                (!title.is_empty()).then_some(title),
            );
            in_chapter = true;
            in_argument = false;
            continue;
        }
        let centred_heading = para.lines.len() == 1
            && para.first().starts_with("          ")
            && text.ends_with('.')
            && text.chars().any(char::is_alphabetic)
            && !text.chars().any(char::is_lowercase);
        if in_chapter && centred_heading {
            b.heading(file, 2, para);
            continue;
        }
        if !in_chapter && !in_argument {
            return Err(parse_error(
                file,
                format!(
                    "line {}: text outside any chapter: {text:.60}",
                    para.line_number()
                ),
            ));
        }
        b.content(file, 2, para, in_chapter)?;
    }
    finish_note(&mut note, &mut notes[1], &mut b);
    b.flush_pending(None);

    // ---- Notes onto their anchors.
    for (v, list) in notes.iter().enumerate() {
        let used = b.next_note[v] - 1;
        if used as usize != list.len() {
            b.finding(
                if v == 0 { &v1.name } else { &v2.name },
                0,
                "markers_and_notes",
                format!("{used} markers in the text, {} notes", list.len()),
            );
        }
    }
    let notes_per_volume = [notes[0].len(), notes[1].len()];
    let mut sections = Vec::with_capacity(b.sections.len());
    for (i, open) in b.sections.iter().enumerate() {
        if open.paragraphs.is_empty() {
            return Err(parse_error(
                "",
                format!("section {} has no text", open.locator),
            ));
        }
        let mut section_notes = Vec::new();
        for (volume, number, marker, paragraph, scalar) in &open.anchors {
            let raw = notes[*volume as usize - 1]
                .get(*number as usize - 1)
                .ok_or_else(|| {
                    parse_error(
                        "",
                        format!("note {volume}:{number} has a marker and no text"),
                    )
                })?;
            if raw.text.is_empty() {
                return Err(parse_error(
                    "",
                    format!("note {volume}:{number} (line {}) is empty", raw.line),
                ));
            }
            section_notes.push(WorkNote {
                id: format!("{volume}:{number}"),
                marker: marker.clone(),
                paragraph: *paragraph,
                scalar: *scalar,
                text: raw.text.clone(),
                kind: openlogos_retrieval_sqlite::work_pack::NoteKind::Comment,
            });
        }
        let citation = open.number.map(|_| {
            openlogos_core::locator::WorkCitation {
                work: "institutes".to_owned(),
                scheme: "institutes".to_owned(),
                grammar_version: openlogos_core::locator::GRAMMAR_VERSION.to_owned(),
                ranges: vec![{
                    let l = openlogos_core::locator::WorkLocator::parse(&open.locator)
                        .unwrap_or(openlogos_core::locator::WorkLocator { parts: Vec::new() });
                    openlogos_core::locator::LocatorRange {
                        start: l.clone(),
                        end: l,
                    }
                }],
            }
            .to_human(&openlogos_core::locator::GrammarTable::builtin())
        });
        sections.push(WorkSection {
            locator: open.locator.clone(),
            ordinal: i as u32 + 1,
            chapter: open.chapter.clone(),
            number: open.number,
            label: open.label.clone(),
            citation,
            paragraphs: open.paragraphs.clone(),
            notes: section_notes,
        });
    }
    // The structure lists title.2 after the syllabus; sections were inserted to match.
    reorder_front(&mut b.structure);
    Ok(ParsedWork {
        structure: b.structure,
        sections,
        findings: b.findings,
        counters: b.counters,
        left_out: b.left_out,
        notes_per_volume,
    })
}

/// Front-matter chapters in the order their sections were placed: volume II's title page
/// directly after the syllabus, the last front unit of volume I.
fn reorder_front(structure: &mut [WorkDivision]) {
    if let Some(front) = structure.iter_mut().find(|d| d.kind == DivisionKind::Front)
        && let Some(at) = front.chapters.iter().position(|c| c.locator == "title.2")
    {
        let unit = front.chapters.remove(at);
        let after = front
            .chapters
            .iter()
            .position(|c| c.locator == "syllabus")
            .map_or(front.chapters.len(), |p| p + 1);
        front.chapters.insert(after, unit);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn roman_numerals_are_strict() {
        assert_eq!(roman("XIV"), Some(14));
        assert_eq!(roman("xliii"), Some(43));
        assert_eq!(roman("LIX"), Some(59));
        assert_eq!(roman("IIII"), None);
        assert_eq!(roman("IC"), None);
        assert_eq!(roman(""), None);
        assert_eq!(
            section_number("II. On the other hand"),
            Some((2, "II.".to_owned()))
        );
        assert_eq!(section_number("I. "), None);
        assert_eq!(section_number("In the first place"), None);
    }

    /// Two hand-written volumes in the source's format; none of this is the work's text.
    pub(crate) fn volumes() -> (DigitizedFile, DigitizedFile) {
        let v1 = "Header\r\nTitle: Test (Vol. 1 of 2)\r\nTranslator: Someone\r\n\r\n*** START OF THE PROJECT GUTENBERG EBOOK TEST ***\r\n\r\n                 A Title\r\n\r\n\r\nCONTENTS\r\n\r\nBook I.\r\n\r\n                [Cover Art]\r\n\r\n[Transcriber’s Note: a note.]\r\n\r\n\r\nADVERTISEMENT.\r\n\r\n\r\nAn advert with a note.(1)\r\n\r\n\r\nTHE TRANSLATOR’S PREFACE.\r\n\r\nPreface.\r\n\r\n\r\nTHE AUTHOR’S PREFACE TO AN EDITION PUBLISHED IN THE YEAR 1559, WITH HIS\r\nLAST CORRECTIONS AND ADDITIONS.\r\n\r\nAuthor.\r\n\r\n\r\nDEDICATION.\r\n\r\n    _To the King_,\r\n    greeting.\r\n\r\nSire, a self‐\r\nexistent letter.\r\n\r\n\r\nGENERAL SYLLABUS.\r\n\r\nI. Not a section here.\r\n\r\n\r\nBOOK I. ON ONE THING.\r\n\r\n\r\nArgument.\r\n\r\nThe argument.\r\n\r\n\r\nChapter I. The First\r\nChapter.\r\n\r\nFirst section, which cites.(2)\r\n\r\nStill the first section.\r\n\r\nII. Second section.(3) And (7) stays.\r\n\r\n\r\n\r\nA Heading.\r\n\r\n    _A quoted block._\r\n\r\nIII. Third section.\r\n\r\n\r\nBOOK III. ON ANOTHER.\r\n\r\n\r\nArgument.\r\n\r\nThird argument.\r\n\r\n\r\nChapter I. Only.\r\n\r\nOne.\r\n\r\nIII. Three, skipping two.\r\n\r\n\r\nFOOTNOTES\r\n\r\n\r\n    1 An editor's comment.\r\n\r\n    2 Rom. viii. 32.\r\n\r\n    3 Jude 9. Ibid.\r\n\r\n*** END OF THE PROJECT GUTENBERG EBOOK TEST ***\r\nLicence.\r\n";
        let v2 = "Header\n*** START OF THE PROJECT GUTENBERG EBOOK TEST 2 ***\n\n                  TITLE TWO.\n\n\n                               BOOK III.\n\n\n                              CHAPTER II.\n       A CENTRED\nTITLE.\n\n\nFirst of two.[1]\n\nII. Second of two.\n\nFootnote 1:\n\n  Psalm cxix. 105.\n\n\n                                BOOK IV.\nON THE LAST.\n\n\n                               ARGUMENT.\n\n\nI. Not a section either.\n\n\n                               CHAPTER I.\n                    ONE.\n\n\nIn chapter one.[2]\n\n                             CONFIRMATION.\n\nII. After the heading.\n\nFootnote 2:\n\n  Like viii. 5.\n\n\n                 INDEX OF THE PRINCIPAL MATTERS.\n\nAn index.\n\n ● Transcriber’s Notes:\n    ○ Italics are underscored.\n\n*** END OF THE PROJECT GUTENBERG EBOOK TEST 2 ***\nLicence.\n";
        (
            split_digitized("v1.txt", v1).unwrap(),
            split_digitized("v2.txt", v2).unwrap(),
        )
    }

    #[test]
    fn the_header_and_licence_are_cut_and_read() {
        let (v1, _) = volumes();
        assert_eq!(v1.header["Translator"], "Someone");
        assert_eq!(v1.header_lines, 5);
        assert_eq!(v1.trailer_lines, 3);
        assert!(v1.crlf_lines > 10);
        assert!(v1.body.iter().all(|l| !l.text.contains("GUTENBERG")));
    }

    #[test]
    fn the_structure_sections_and_notes_come_out_in_order() {
        let (v1, v2) = volumes();
        let work = parse_volumes(&v1, &v2).unwrap();
        let locators: Vec<&str> = work.sections.iter().map(|s| s.locator.as_str()).collect();
        assert_eq!(
            locators,
            [
                "title.1",
                "advertisement",
                "translator-preface",
                "author-preface",
                "dedication",
                "syllabus",
                "title.2",
                "1.argument",
                "1.1.1",
                "1.1.2",
                "1.1.3",
                "3.argument",
                "3.1.1",
                "3.1.3",
                "3.2.1",
                "3.2.2",
                "4.argument",
                "4.1.1",
                "4.1.2",
            ]
        );
        let ordinals: Vec<u32> = work.sections.iter().map(|s| s.ordinal).collect();
        assert_eq!(ordinals, (1..=19).collect::<Vec<_>>());
        let front: Vec<&str> = work.structure[0]
            .chapters
            .iter()
            .map(|c| c.locator.as_str())
            .collect();
        assert_eq!(front.last(), Some(&"title.2"));
        assert_eq!(front[5], "syllabus");
        // The gap is reported, not repaired.
        assert!(
            work.findings
                .iter()
                .any(|f| f.kind == "gap" && f.detail.contains("3.1: section III."))
        );
        // Volume II's chapter title and book title.
        let book3 = work.structure.iter().find(|d| d.locator == "3").unwrap();
        assert_eq!(book3.chapters[2].title.as_deref(), Some("A CENTRED TITLE."));
        let book4 = work.structure.iter().find(|d| d.locator == "4").unwrap();
        assert_eq!(book4.title.as_deref(), Some("ON THE LAST."));
        assert_eq!(book4.label.as_deref(), Some("BOOK IV."));
    }

    #[test]
    fn text_keeps_the_source_and_loses_only_wrapping_and_markers() {
        let (v1, v2) = volumes();
        let work = parse_volumes(&v1, &v2).unwrap();
        let section = |l: &str| work.sections.iter().find(|s| s.locator == l).unwrap();
        let dedication = section("dedication");
        assert_eq!(dedication.paragraphs[0].style, ParagraphStyle::Block);
        assert_eq!(dedication.paragraphs[0].text, "_To the King_,\ngreeting.");
        assert_eq!(
            dedication.paragraphs[1].text,
            "Sire, a self‐existent letter."
        );
        let first = section("1.1.1");
        assert_eq!(first.label, None);
        assert_eq!(first.paragraphs[0].text, "First section, which cites.");
        assert_eq!(first.notes[0].id, "1:2");
        assert_eq!(first.notes[0].marker, "(2)");
        assert_eq!(first.notes[0].scalar, 27);
        assert_eq!(first.notes[0].text, "Rom. viii. 32.");
        assert_eq!(first.paragraphs.len(), 2);
        let second = section("1.1.2");
        assert_eq!(second.label.as_deref(), Some("II."));
        assert_eq!(second.citation.as_deref(), Some("Institutes 1.1.2"));
        assert_eq!(
            second.paragraphs[0].text,
            "II. Second section. And (7) stays."
        );
        // The heading and its block wait for section III.
        let third = section("1.1.3");
        assert_eq!(third.paragraphs[0].style, ParagraphStyle::Heading);
        assert_eq!(third.paragraphs[0].text, "A Heading.");
        assert_eq!(third.paragraphs[1].style, ParagraphStyle::Block);
        assert_eq!(third.paragraphs[2].text, "III. Third section.");
        let after = section("4.1.2");
        assert_eq!(after.paragraphs[0].text, "CONFIRMATION.");
        assert_eq!(section("3.2.1").notes[0].text, "Psalm cxix. 105.");
        assert_eq!(section("3.2.1").notes[0].marker, "[1]");
        assert_eq!(work.counters.joined_after_hyphen, 1);
        assert_eq!(work.counters.stray_markers, 1);
        assert!(work.findings.iter().any(|f| f.kind == "marker_not_next"));
        assert_eq!(work.left_out.transcriber_notes.len(), 2);
        assert_eq!(work.notes_per_volume, [3, 2]);
        assert!(section("syllabus").paragraphs[0].text.starts_with("I. Not"));
    }
}
