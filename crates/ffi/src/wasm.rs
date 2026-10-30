//! The wasm-bindgen surface: what the web reader's worker and the Obsidian plugin call
//! (docs/adr/0001-rust-core-native-ui.md). Compiled only for `wasm32`; the native surface
//! for Swift and Kotlin is the sibling module.
//!
//! Values cross the boundary as JS objects produced by `serde-wasm-bindgen` from the same
//! serde derives the core uses for JSON, so a JS object carries the core's field names
//! (`segment_id`, `content_revision`, ...) and a fixture from [`conformance_fixtures_json`]
//! can be passed straight back in. `Option::None` becomes `null`, as in JSON, and nothing is
//! stringified twice. Offsets given to or returned by this module as UTF-16 code units are
//! JS string indexes; everything inside an anchor stays in Unicode scalar values.
//!
//! A failing call throws a JS `Error` whose `message` is the core's `thiserror` text, whose
//! `kind` is the serde tag of the variant (`unknown_book`, `splits_surrogate_pair`, ...) and
//! whose `details` holds the whole error as an object.

use std::sync::OnceLock;

#[cfg(feature = "retrieval")]
pub mod retrieval;

use openlogos_conformance::{ConformanceReport, Fixture};
use openlogos_core::anchor::{Anchor, Revision, Segment, TokenId, split_selection};
use openlogos_core::reference::{AliasTable, ReferenceSet, parse};
use openlogos_core::versification::{SchemeId, VerseMap};
use openlogos_ink::{Easing, EndOptions, InputPoint, StrokeOptions, Taper};
use openlogos_text::{Utf16Range, scalar_to_utf16, utf16_to_scalar};
use openlogos_workspaces::commands::{CommandContext, Invocation, Shell, registry};
use openlogos_workspaces::migrate_workspace_to_2;
use openlogos_workspaces::palette::{
    PaletteContext, PaletteIndex, TextProvider, palette_query_indexed,
};
use openlogos_workspaces::shell::{Session, SessionRecord, ShellError, Workspace};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Errors raised by this surface itself, around the core's own error enums.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WasmError {
    /// A JS value that does not have the shape the parameter needs.
    #[error("{parameter}: {message}")]
    InvalidArgument {
        /// Name of the parameter.
        parameter: String,
        /// What serde-wasm-bindgen said.
        message: String,
    },
    /// A citation given where exactly one reference (not a range or a list) was needed.
    #[error("`{text}` must name exactly one reference")]
    NotOneReference {
        /// The citation as given.
        text: String,
    },
    /// The fixture table could not be loaded, which a passing conformance run rules out.
    #[error("fixture `{fixture}` is unusable: {message}")]
    FixtureUnusable {
        /// The fixture id.
        fixture: String,
        /// Why.
        message: String,
    },
}

/// Serializes a core value to a JS object with JSON semantics (`None` is `null`).
fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(JsValue::from)
}

/// Deserializes a parameter, naming it in the error.
fn from_js<T: for<'de> Deserialize<'de>>(parameter: &str, value: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(value).map_err(|e| {
        throw(WasmError::InvalidArgument {
            parameter: parameter.to_owned(),
            message: e.to_string(),
        })
    })
}

/// A core error as a JS `Error` with `kind` and `details`.
fn throw<E: std::error::Error + Serialize>(error: E) -> JsValue {
    let js = js_sys::Error::new(&error.to_string());
    let details = to_js(&error).unwrap_or(JsValue::UNDEFINED);
    let kind =
        js_sys::Reflect::get(&details, &JsValue::from_str("kind")).unwrap_or(JsValue::UNDEFINED);
    // Setting a property on a fresh Error object cannot fail; the results are ignored.
    let _ = js_sys::Reflect::set(&js, &JsValue::from_str("kind"), &kind);
    let _ = js_sys::Reflect::set(&js, &JsValue::from_str("details"), &details);
    js.into()
}

#[wasm_bindgen(typescript_custom_section)]
const TYPES: &str = r#"
/** A scheme-neutral reference; `verse` is null for a whole chapter. */
export interface Reference { book: string; chapter: number; verse: number | null; subverse: string | null }
export interface ReferenceRange { start: Reference; end: Reference }
export type ReviewFlag =
  | { kind: "chapter_only"; range: number }
  | { kind: "ambiguous_book"; range: number; alias: string; candidates: string[] };
export interface ReferenceSet { ranges: ReferenceRange[]; flags: ReviewFlag[] }
export type MappingStatus = "exact" | "partial" | "ambiguous" | "unmapped";
export interface MappingResult { status: MappingStatus; targets: Reference[]; path: string[] }
/** A half-open range of Unicode scalar values. */
export interface ScalarRange { start: number; end: number }
/** A half-open range of UTF-16 code units: JS string indexes. */
export interface Utf16Range { start: number; end: number }
/** A token id as an object; `parseTokenId` and `formatTokenId` convert the `edition:BBCCCVVVWWW[s]` string form. */
export interface TokenId { edition: string; book: string; chapter: number; verse: number; word: number; suffix: string | null }
export interface TokenSpan { id: TokenId; range: ScalarRange }
export interface Segment { segment_id: string; text: string; tokens: TokenSpan[] }
/** Segments as laid out for selection, joined by `joiner`. */
export interface Paragraph { edition_id: string; content_revision: string; joiner: string; segments: Segment[] }
export interface TextQuoteSelector { exact: string; prefix: string; suffix: string }
export interface Anchor { edition_id: string; content_revision: string; segment_id: string; range: ScalarRange; tokens: TokenId[]; quote: TextQuoteSelector | null }
export interface Revision { content_revision: string; text: string; tokens: TokenSpan[] }
export type ReanchorResult =
  | { status: "exact"; value: Anchor }
  | { status: "moved"; value: Anchor }
  | { status: "ambiguous"; value: Anchor[] }
  | { status: "orphaned" };
export interface CaseResult { fixture: string; name: string; passed: boolean; expected: string; actual: string }
export interface ConformanceReport { api_version: string; passed: number; failed: number; cases: CaseResult[] }
/** What a failing call throws: an Error with the variant tag and the whole error object. */
export interface CoreError extends Error { kind: string; details: unknown }
/** perfect-freehand options for `inkOutline` (B039); every field optional, upstream defaults. `taper*`: false/absent none, true full, a number a distance. */
export interface InkOutlineOptions { size?: number; thinning?: number; smoothing?: number; streamline?: number; simulatePressure?: boolean; last?: boolean; capStart?: boolean; capEnd?: boolean; taperStart?: boolean | number; taperEnd?: boolean | number }
export type InkTool = "pen" | "highlighter";
/** One sampled point of `schemas/ink-document.schema.json`; `pressure` is null when the device reported none; `altitude`/`azimuth` (1.1, radians) likewise, and absent in a 1.0 file. */
export interface InkPoint { x: number; y: number; pressure: number | null; elapsed_ms: number; altitude?: number | null; azimuth?: number | null }
export interface InkStroke { id: string; tool: InkTool; brush_version: string; width: number; color: string; opacity: number; points: InkPoint[] }
export interface InkViewportTransform { scale: number; translate_x: number; translate_y: number }
/** The portable ink record, field names as the schema. */
/** B070 workspace shell (specs/22): a tab is specs/21's pane; column numbers are positions. */
export interface ShellTab { id: string; kind: "reading" | "notes" | "research" | "lexicon"; content_id: string; content_revision: string; reference: string | null; scroll_anchor?: { segment_id: string; scalar: number }; link_group: string | null; pinned: boolean; history?: { back: unknown[]; current: unknown | null; forward: unknown[] } }
export interface ShellColumn { id: string; tabs: ShellTab[]; shown_tab_id: string; width_fraction: number }
export interface ShellWorkspace { schema_version: "2.0"; fixture_only: boolean; id: string; title: string; columns: ShellColumn[]; active_column_id: string; notes_dock: { note_id: string | null; open: boolean }; layout_profiles?: Record<string, { visible_column_ids: string[]; widths?: Record<string, number>; notes_dock_open: boolean }>; next_serial: number }
export type ShellTool = "select" | "highlight" | "pen" | "highlighter_pen" | "eraser" | "collect";
/** The in-memory session: the session record with its workspaces inline. */
/** Text size step from the platform default, line spacing (1.35, 1.5 or 1.7) and theme. */
export interface ShellReading { text_size_step: number; line_spacing: number; theme: "system" | "light" | "dark" | "sepia" }
/** Brush width and the two swatch wells (`#RRGGBB`). */
export interface ShellInk { width: number; swatch: string; alternate_swatch: string }
export interface ShellSession { schema_version: "1.0"; fixture_only: boolean; workspaces: ShellWorkspace[]; active_workspace_id: string; tool: ShellTool; next_serial: number; reading: ShellReading; ink: ShellInk }
export interface ShellSessionRecord { schema_version: "1.0"; fixture_only: boolean; workspace_ids: string[]; active_workspace_id: string; tool: ShellTool; next_serial: number; reading?: ShellReading; ink?: ShellInk }
export interface ShellCommandArgs { column?: number; new_column?: boolean; text?: string; reference?: string; workspace_id?: string; tab_id?: string; group?: string; number?: number; segment_id?: string; scalar?: number }
/** The core's pane minimums and measure caps, in points. */
export interface LayoutConstants { pane_minimum_standard_pt: number; pane_minimum_accessibility_pt: number; measure_cap_standard_pt: number; measure_cap_accessibility_pt: number }
export interface ShellInvocation { command_id: string; args?: ShellCommandArgs }
export interface ShellCommandContext { has_selection?: boolean }
export type ShellPlatform = "web" | "mac" | "ipad";
export interface ShellCommandSpec { id: string; title: string; aliases: string[]; keywords: string[]; scope: "session" | "workspace" | "column" | "tab" | "selection"; argument: "none" | "column_number" | "text" | "reference" | "workspace" | "tab" | "link_group" | "number" | "column_width" | "scroll"; shortcuts: { web: string[]; mac: string[]; ipad: string[] }; bare_key: boolean; undoable: boolean; handled_by_core: boolean; quiet: boolean }
/** `disabled` is null when the command can run; otherwise `{ kind, ... }`. */
export interface ShellCommandState { spec: ShellCommandSpec; disabled: { kind: string } | null }
export interface ShellOutcome { command_id: string; handled: boolean; recorded: boolean; effects: { followed_tab_ids: string[] }; revision: number }
export type ShellPaletteSection = "passage" | "commands" | "tabs" | "text";
export interface ShellPaletteContext { has_selection?: boolean; platform?: ShellPlatform; expanded?: ShellPaletteSection | null }
export interface ShellPaletteItem { section: ShellPaletteSection; title: string; subtitle: string | null; score: number; invocation: ShellInvocation; disabled: { kind: string } | null; shortcut: string | null }
export interface ShellPaletteResult { query: { mode: "all" | "commands" | "tabs" | "text" | "passage"; text: string; target: { kind: "column"; number: number } | { kind: "new" } | null }; groups: { section: ShellPaletteSection; items: ShellPaletteItem[]; total: number }[]; notice: string | null }
export interface ShellRecords { session: ShellSessionRecord; workspaces: ShellWorkspace[] }
export interface InkDocument { schema_version: "1.0" | "1.1"; fixture_only: boolean; id: string; snapshot_id: string; coordinate_space_id: string; strokes: InkStroke[]; deleted_stroke_ids: string[]; viewport_transform: InkViewportTransform }
"#;

/// `InkOutlineOptions` as a page passes it: upstream's camelCase names, every field optional.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct InkOutlineOptionsJs {
    size: Option<f64>,
    thinning: Option<f64>,
    smoothing: Option<f64>,
    streamline: Option<f64>,
    simulate_pressure: Option<bool>,
    last: Option<bool>,
    cap_start: Option<bool>,
    cap_end: Option<bool>,
    taper_start: Option<serde_json::Value>,
    taper_end: Option<serde_json::Value>,
}

fn taper_from_js(value: Option<&serde_json::Value>) -> Taper {
    match value {
        Some(serde_json::Value::Bool(true)) => Taper::Full,
        Some(serde_json::Value::Number(n)) => n.as_f64().map_or(Taper::None, Taper::Distance),
        _ => Taper::None,
    }
}

impl From<InkOutlineOptionsJs> for StrokeOptions {
    fn from(o: InkOutlineOptionsJs) -> Self {
        let d = StrokeOptions::default();
        StrokeOptions {
            size: o.size.unwrap_or(d.size),
            thinning: o.thinning.unwrap_or(d.thinning),
            smoothing: o.smoothing.unwrap_or(d.smoothing),
            streamline: o.streamline.unwrap_or(d.streamline),
            easing: Easing::Linear,
            simulate_pressure: o.simulate_pressure.unwrap_or(d.simulate_pressure),
            start: EndOptions {
                cap: o.cap_start.unwrap_or(true),
                taper: taper_from_js(o.taper_start.as_ref()),
                easing: Easing::EaseOutQuad,
            },
            end: EndOptions {
                cap: o.cap_end.unwrap_or(true),
                taper: taper_from_js(o.taper_end.as_ref()),
                easing: Easing::EaseOutCubic,
            },
            last: o.last.unwrap_or(d.last),
        }
    }
}

/// B039: the perfect-freehand outline of a stroke from `crates/ink`. `points` is a
/// `Float64Array` of `x, y, pressure` triples (a pressure below zero means the device reported
/// none; a trailing partial triple is ignored); the result is a `Float64Array` of `x, y` pairs
/// of the closed polygon, to be filled once with the non-zero rule.
#[wasm_bindgen(js_name = inkOutline)]
pub fn ink_outline(
    points: &[f64],
    #[wasm_bindgen(unchecked_param_type = "InkOutlineOptions | undefined")] options: JsValue,
) -> Result<Vec<f64>, JsValue> {
    let options: InkOutlineOptionsJs = if options.is_undefined() || options.is_null() {
        InkOutlineOptionsJs::default()
    } else {
        from_js("options", options)?
    };
    let input: Vec<InputPoint> = points
        .as_chunks::<3>()
        .0
        .iter()
        .map(|c| InputPoint::new(c[0], c[1], (c[2] >= 0.0).then_some(c[2])))
        .collect();
    Ok(openlogos_ink::get_stroke(&input, &options.into())
        .into_iter()
        .flat_map(|p| [p[0], p[1]])
        .collect())
}

/// B039: validates an `InkDocument` object against the rules of
/// `schemas/ink-document.schema.json` and `crates/ink` and returns it as JSON (pretty-printed,
/// `null` for an unreported pressure). Throws a `CoreError` whose `kind` names the broken rule.
#[wasm_bindgen(js_name = inkDocumentJson)]
pub fn ink_document_json(
    #[wasm_bindgen(unchecked_param_type = "InkDocument")] document: JsValue,
) -> Result<String, JsValue> {
    let document: openlogos_ink::InkDocument = from_js("document", document)?;
    document.to_json().map_err(throw)
}

/// The core API version this module was built against ([`openlogos_core::API_VERSION`]).
/// A host compares it with the version it was written for before calling anything else.
#[wasm_bindgen(js_name = apiVersion)]
#[must_use]
pub fn api_version() -> String {
    openlogos_core::api_version().to_owned()
}

/// Parses a citation such as `Jn 3:16, 18; Rom 8:28-30` with the English alias table.
#[wasm_bindgen(js_name = parseReference, unchecked_return_type = "ReferenceSet")]
pub fn parse_reference(text: &str) -> Result<JsValue, JsValue> {
    let set = parse(text, &AliasTable::english()).map_err(throw)?;
    to_js(&set)
}

/// The canonical OSIS form of a parsed set, e.g. `John.3.16;John.3.18;Rom.8.28-Rom.8.30`.
#[wasm_bindgen(js_name = formatOsis)]
pub fn format_osis(
    #[wasm_bindgen(unchecked_param_type = "ReferenceSet")] reference_set: JsValue,
) -> Result<String, JsValue> {
    let set: ReferenceSet = from_js("referenceSet", reference_set)?;
    Ok(set.to_osis())
}

/// The human form of a parsed set with English names, e.g. `John 3:16, 18; Romans 8:28–30`.
#[wasm_bindgen(js_name = formatHuman)]
pub fn format_human(
    #[wasm_bindgen(unchecked_param_type = "ReferenceSet")] reference_set: JsValue,
) -> Result<String, JsValue> {
    let set: ReferenceSet = from_js("referenceSet", reference_set)?;
    Ok(set.to_human(&AliasTable::english()))
}

/// B070: the short form of a citation for tab labels and palette rows, from text (a tab's
/// OSIS reference or anything `parseReference` reads): `Jn 3`, `Jn 3:16`, `Gen 1:1–2`.
#[wasm_bindgen(js_name = formatShort)]
pub fn format_short(reference: &str) -> Result<String, JsValue> {
    let set = parse(reference, &AliasTable::english()).map_err(throw)?;
    Ok(set.to_short())
}

/// The Psalm 9/10 fixture table, built once. Only this table exists until the package
/// crate loads real maps from packs, so a reference outside it maps as `unmapped`.
fn psalm_table() -> Result<&'static VerseMap, JsValue> {
    static TABLE: OnceLock<Result<VerseMap, String>> = OnceLock::new();
    const FIXTURE: &str = "psalm-9-10-numbering";
    TABLE
        .get_or_init(|| {
            openlogos_conformance::fixtures()
                .into_iter()
                .find_map(|fixture| match fixture {
                    Fixture::Versification(f) if f.id == FIXTURE => Some(f.verse_map()),
                    _ => None,
                })
                .unwrap_or_else(|| Err("fixture not found".to_owned()))
        })
        .as_ref()
        .map_err(|message| {
            throw(WasmError::FixtureUnusable {
                fixture: FIXTURE.to_owned(),
                message: message.clone(),
            })
        })
}

/// Maps one reference between versification schemes (`eng`, `org`, `lxx`) using the Psalm
/// 9/10 fixture table; `reference` is one verse (or chapter, which maps as `unmapped`) in
/// OSIS or human form.
#[wasm_bindgen(js_name = mapVerse, unchecked_return_type = "MappingResult")]
pub fn map_verse(reference: &str, from_scheme: &str, to_scheme: &str) -> Result<JsValue, JsValue> {
    let set = parse(reference, &AliasTable::english()).map_err(throw)?;
    let verse = match set.ranges.as_slice() {
        [range] if range.is_single() => &range.start,
        _ => {
            return Err(throw(WasmError::NotOneReference {
                text: reference.to_owned(),
            }));
        }
    };
    let result = psalm_table()?.map(
        verse,
        &SchemeId::new(from_scheme),
        &SchemeId::new(to_scheme),
    );
    to_js(&result)
}

/// Parses a token id in the `edition:BBCCCVVVWWW[s]` form the pipeline mints and the fixture
/// files use, into the object form anchors and segments carry.
#[wasm_bindgen(js_name = parseTokenId, unchecked_return_type = "TokenId")]
pub fn parse_token_id(text: &str) -> Result<JsValue, JsValue> {
    let id = TokenId::parse(text).map_err(throw)?;
    to_js(&id)
}

/// The `edition:BBCCCVVVWWW[s]` form of a token id object.
#[wasm_bindgen(js_name = formatTokenId)]
pub fn format_token_id(
    #[wasm_bindgen(unchecked_param_type = "TokenId")] token_id: JsValue,
) -> Result<String, JsValue> {
    let id: TokenId = from_js("tokenId", token_id)?;
    Ok(id.to_string())
}

/// Segments rendered as one paragraph, as a text view lays them out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Paragraph {
    /// The edition.
    pub edition_id: String,
    /// Content revision of the edition the segments come from.
    pub content_revision: String,
    /// Text placed between segments.
    pub joiner: String,
    /// The segments in paragraph order.
    pub segments: Vec<Segment>,
}

impl Paragraph {
    fn text(&self) -> String {
        self.segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(&self.joiner)
    }
}

/// Turns a selection, given as JS string indexes over the paragraph text (segments joined
/// by the paragraph's joiner), into one anchor per segment it touches.
#[wasm_bindgen(js_name = selectionToAnchor, unchecked_return_type = "Anchor[]")]
pub fn selection_to_anchor(
    #[wasm_bindgen(unchecked_param_type = "Paragraph")] paragraph: JsValue,
    utf16_start: u32,
    utf16_end: u32,
) -> Result<JsValue, JsValue> {
    let paragraph: Paragraph = from_js("paragraph", paragraph)?;
    let text = paragraph.text();
    let range = utf16_to_scalar(
        &text,
        Utf16Range {
            start: utf16_start,
            end: utf16_end,
        },
    )
    .map_err(throw)?;
    let anchors = split_selection(
        &paragraph.edition_id,
        &paragraph.content_revision,
        &paragraph.segments,
        &paragraph.joiner,
        range,
    )
    .map_err(throw)?;
    to_js(&anchors)
}

/// The anchor's range as JS string indexes into `text`, the text of the anchor's segment,
/// for painting a highlight.
#[wasm_bindgen(js_name = anchorToUtf16, unchecked_return_type = "Utf16Range")]
pub fn anchor_to_utf16(
    #[wasm_bindgen(unchecked_param_type = "Anchor")] anchor: JsValue,
    text: &str,
) -> Result<JsValue, JsValue> {
    let anchor: Anchor = from_js("anchor", anchor)?;
    let range = scalar_to_utf16(text, anchor.range).map_err(throw)?;
    to_js(&range)
}

/// Finds `anchor` again in revision `new` of its segment, given revision `old` it was taken
/// against: token ids first, then the quoted text with context, never the nearest guess.
#[wasm_bindgen(js_name = reanchor, unchecked_return_type = "ReanchorResult")]
pub fn reanchor(
    #[wasm_bindgen(unchecked_param_type = "Anchor")] anchor: JsValue,
    #[wasm_bindgen(unchecked_param_type = "Revision")] old: JsValue,
    #[wasm_bindgen(unchecked_param_type = "Revision")] new: JsValue,
) -> Result<JsValue, JsValue> {
    let anchor: Anchor = from_js("anchor", anchor)?;
    let old: Revision = from_js("old", old)?;
    let new: Revision = from_js("new", new)?;
    to_js(&openlogos_core::anchor::reanchor(&anchor, &old, &new))
}

/// Runs every conformance fixture through the core compiled into this module and reports
/// each expected and actual value; a host asserts `failed === 0`.
#[wasm_bindgen(js_name = runConformance, unchecked_return_type = "ConformanceReport")]
pub fn run_conformance() -> Result<JsValue, JsValue> {
    let report: ConformanceReport = openlogos_conformance::run_all();
    to_js(&report)
}

/// Every conformance fixture as one JSON array, for reading or display.
#[wasm_bindgen(js_name = conformanceFixturesJson)]
#[must_use]
pub fn conformance_fixtures_json() -> String {
    openlogos_conformance::fixtures_json().to_owned()
}

/// B070: a session with its undo history, owned by the core (specs/22 §2–3). Commands,
/// undo, redo and the palette run against it; the palette's index is rebuilt only when the
/// revision changed, so `paletteQuery` is cheap on every keystroke.
#[wasm_bindgen(js_name = ShellHandle)]
pub struct ShellHandle {
    shell: Shell,
    index: Option<(u64, PaletteIndex)>,
    text: Option<openlogos_conformance::FixtureTextProvider>,
}

fn shell_of(session: Session) -> Result<ShellHandle, JsValue> {
    Ok(ShellHandle {
        shell: Shell::new(session).map_err(throw)?,
        index: None,
        text: None,
    })
}

#[wasm_bindgen(js_class = ShellHandle)]
impl ShellHandle {
    /// A handle over an in-memory session (`ShellSession`, workspaces inline).
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "ShellSession")] session: JsValue,
    ) -> Result<ShellHandle, JsValue> {
        shell_of(from_js("session", session)?)
    }

    /// A handle over the persisted records; 1.x workspace records are migrated to 2.0.
    #[wasm_bindgen(js_name = fromRecords)]
    pub fn from_records(
        #[wasm_bindgen(unchecked_param_type = "ShellSessionRecord")] record: JsValue,
        #[wasm_bindgen(unchecked_param_type = "unknown[]")] workspaces: JsValue,
    ) -> Result<ShellHandle, JsValue> {
        let record: SessionRecord = from_js("record", record)?;
        let documents: Vec<serde_json::Value> = from_js("workspaces", workspaces)?;
        let mut migrated = Vec::with_capacity(documents.len());
        for document in documents {
            let value = migrate_workspace_to_2(document).map_err(throw)?;
            let workspace: Workspace = serde_json::from_value(value).map_err(|e| {
                throw(WasmError::InvalidArgument {
                    parameter: "workspaces".to_owned(),
                    message: e.to_string(),
                })
            })?;
            migrated.push(workspace);
        }
        shell_of(Session::from_records(record, migrated).map_err(throw::<ShellError>)?)
    }

    /// The `shell-commands` conformance session with the fixture text provider.
    #[must_use]
    pub fn fixture() -> ShellHandle {
        let session = openlogos_conformance::fixture_session();
        let mut handle = shell_of(session).expect("the fixture session checks");
        handle.text = Some(openlogos_conformance::FixtureTextProvider::new());
        handle
    }

    /// Answers the Text section from the conformance fixture texts (until B071's search).
    #[wasm_bindgen(js_name = useFixtureText)]
    pub fn use_fixture_text(&mut self) {
        self.text = Some(openlogos_conformance::FixtureTextProvider::new());
    }

    /// Registers the short names resources are shown under (`{ bsb: "BSB" }`); palette
    /// rows read `BSB · Jn 3:16`. A resource without one shows its id.
    #[wasm_bindgen(js_name = setResourceTitles)]
    pub fn set_resource_titles(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "Record<string, string>")] titles: JsValue,
    ) -> Result<(), JsValue> {
        let titles: std::collections::BTreeMap<String, String> = from_js("titles", titles)?;
        self.shell.set_resource_titles(titles);
        Ok(())
    }

    /// The session.
    #[wasm_bindgen(unchecked_return_type = "ShellSession")]
    pub fn session(&self) -> Result<JsValue, JsValue> {
        to_js(&self.shell.session)
    }

    /// Bumped by every change.
    #[must_use]
    pub fn revision(&self) -> f64 {
        self.shell.revision as f64
    }

    /// The records to persist.
    #[wasm_bindgen(unchecked_return_type = "ShellRecords")]
    pub fn records(&self) -> Result<JsValue, JsValue> {
        #[derive(Serialize)]
        struct Records {
            session: SessionRecord,
            workspaces: Vec<Workspace>,
        }
        let (session, workspaces) = self.shell.session.to_records();
        to_js(&Records {
            session,
            workspaces,
        })
    }

    /// Every registered command with whether it can run now.
    #[wasm_bindgen(unchecked_return_type = "ShellCommandState[]")]
    pub fn commands(
        &self,
        #[wasm_bindgen(unchecked_param_type = "ShellCommandContext | undefined")] context: JsValue,
    ) -> Result<JsValue, JsValue> {
        let context: CommandContext = optional("context", context)?;
        to_js(&self.shell.commands(&context))
    }

    /// Runs a command; `handled === false` means the host performs it.
    #[wasm_bindgen(unchecked_return_type = "ShellOutcome")]
    pub fn apply(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "ShellInvocation")] invocation: JsValue,
        #[wasm_bindgen(unchecked_param_type = "ShellCommandContext | undefined")] context: JsValue,
    ) -> Result<JsValue, JsValue> {
        let invocation: Invocation = from_js("invocation", invocation)?;
        let context: CommandContext = optional("context", context)?;
        let outcome = self.shell.apply(&invocation, &context).map_err(throw)?;
        to_js(&outcome)
    }

    /// Takes back the last undoable command.
    #[wasm_bindgen(unchecked_return_type = "ShellOutcome")]
    pub fn undo(&mut self) -> Result<JsValue, JsValue> {
        let outcome = self.shell.undo().map_err(throw)?;
        to_js(&outcome)
    }

    /// Runs the last undone command again.
    #[wasm_bindgen(unchecked_return_type = "ShellOutcome")]
    pub fn redo(&mut self) -> Result<JsValue, JsValue> {
        let outcome = self.shell.redo().map_err(throw)?;
        to_js(&outcome)
    }

    /// The palette's answer for the field's text.
    #[wasm_bindgen(js_name = paletteQuery, unchecked_return_type = "ShellPaletteResult")]
    pub fn palette_query(
        &mut self,
        text: &str,
        #[wasm_bindgen(unchecked_param_type = "ShellPaletteContext | undefined")] context: JsValue,
    ) -> Result<JsValue, JsValue> {
        let context: PaletteContext = optional("context", context)?;
        let revision = self.shell.revision;
        if self.index.as_ref().is_none_or(|(r, _)| *r != revision) {
            self.index = Some((revision, PaletteIndex::new(&self.shell.session)));
        }
        let index = &self.index.as_ref().expect("built above").1;
        let provider = self.text.as_ref().map(|p| p as &dyn TextProvider);
        to_js(&palette_query_indexed(
            &self.shell,
            index,
            text,
            &context,
            provider,
        ))
    }
}

/// A parameter that may be `undefined` or `null`, defaulting.
fn optional<T: Default + for<'de> Deserialize<'de>>(
    parameter: &str,
    value: JsValue,
) -> Result<T, JsValue> {
    if value.is_undefined() || value.is_null() {
        Ok(T::default())
    } else {
        from_js(parameter, value)
    }
}

/// Every registered command (B070), without state.
#[wasm_bindgen(js_name = shellRegistry, unchecked_return_type = "ShellCommandSpec[]")]
pub fn shell_registry() -> Result<JsValue, JsValue> {
    to_js(&registry().to_vec())
}

/// Migrates a workspace document at any 1.x version to 2.0 (B070).
#[wasm_bindgen(js_name = migrateWorkspace, unchecked_return_type = "ShellWorkspace")]
pub fn migrate_workspace(
    #[wasm_bindgen(unchecked_param_type = "unknown")] document: JsValue,
) -> Result<JsValue, JsValue> {
    let value: serde_json::Value = from_js("document", document)?;
    to_js(&migrate_workspace_to_2(value).map_err(throw)?)
}

/// The core's pane minimums and measure caps, so the web shell does not mirror them.
#[wasm_bindgen(js_name = layoutConstants, unchecked_return_type = "LayoutConstants")]
pub fn layout_constants() -> Result<JsValue, JsValue> {
    to_js(&openlogos_workspaces::layout_constants())
}
