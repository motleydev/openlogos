//! Works (B016) on the web: `WorkHandle` over a work pack's bytes, behind the `retrieval`
//! feature. Values are the core's JSON shapes (`openlogos_retrieval_sqlite::work_pack`), as
//! everywhere on this surface. A page fetches `institutes-allen.sqlite` once, opens it here for
//! reading and hands the same bytes to `ShellHandle.useEditionSearch` beside the editions'
//! search packs for the palette's Text section (the bytes are copied into SQLite's memory by
//! each). Errors throw with `kind` `not_a_work_pack`, `not_found`, `passage` or `storage`.

use openlogos_retrieval_sqlite::work_pack::WorkPack;
use wasm_bindgen::prelude::*;

use super::{throw, to_js};

#[wasm_bindgen(typescript_custom_section)]
const TYPES: &str = r#"
/** One printed volume behind a work edition (B016). */
export interface WorkVolume { volume: number; title: string; identifier: string; release_date: string; last_update: string; credits: string }
/** The bibliographic evidence of a work edition: what its catalog entry shows offline. */
export interface WorkBibliography { author: string; translator: string; edition: string; place: string; publisher: string; date_printed: string; original_date: string; subjects: string[]; volumes: WorkVolume[]; note: string }
/** A work edition's identity, rights, bibliography and counts. */
export interface WorkInfo { resource_id: string; work: string; locator_scheme: string; content_revision: string; title: string; short_title: string; language: string; source_id: string; license: string; attribution: string; source: string; bibliography: WorkBibliography; sections: number; notes: number; citations: number; resolved: number }
/** A chapter-level unit: a numbered chapter (`3.2`), a book's argument (`1.argument`) or a front-matter piece (`dedication`). */
export interface WorkChapterInfo { locator: string; kind: "front" | "argument" | "chapter"; label: string | null; title: string | null; sections: string[] }
/** A top-level division: the front matter or a numbered book. */
export interface WorkDivision { locator: string; kind: "front" | "book"; label: string | null; title: string | null; chapters: WorkChapterInfo[] }
/** One paragraph; a block keeps its line breaks. */
export interface WorkParagraph { style: "prose" | "block" | "heading"; text: string }
/** One note, anchored at a scalar offset of a paragraph of its section. */
export interface WorkNote { id: string; marker: string; paragraph: number; scalar: number; text: string; kind: "citations" | "comment" | "mixed" }
/** One section with its text and notes. */
export interface WorkSection { locator: string; ordinal: number; chapter: string; number: number | null; label: string | null; citation: string | null; paragraphs: WorkParagraph[]; notes: WorkNote[] }
/** A chapter with its sections. */
export interface WorkChapter { division: string; info: WorkChapterInfo; sections: WorkSection[] }
/** One citation in a section's notes; an unresolved one keeps its class. */
export interface WorkCitationLink { section: string; note: string; text: string; start: number; end: number; status: "resolved" | "unresolved"; target: string | null; class: string | null; detail: string | null }
/** A section citing a passage: what "cited in Calvin" lists under a verse. */
export interface CitingSection { section: string; label: string; chapter_title: string | null; text: string; note: string; target: string }
"#;

/// One work pack opened from its bytes.
#[wasm_bindgen]
pub struct WorkHandle {
    pack: WorkPack,
}

#[wasm_bindgen]
impl WorkHandle {
    /// Opens a work pack from the bytes a page fetched.
    #[wasm_bindgen(constructor)]
    pub fn new(pack: js_sys::Uint8Array) -> Result<WorkHandle, JsValue> {
        WorkPack::from_bytes(&pack.to_vec())
            .map(|pack| WorkHandle { pack })
            .map_err(throw)
    }

    /// Identity, rights, bibliography and counts: the catalog entry.
    #[wasm_bindgen(js_name = catalogEntry, unchecked_return_type = "WorkInfo")]
    pub fn catalog_entry(&self) -> Result<JsValue, JsValue> {
        to_js(self.pack.info())
    }

    /// Divisions, chapters and section locators in reading order.
    #[wasm_bindgen(unchecked_return_type = "WorkDivision[]")]
    pub fn structure(&self) -> Result<JsValue, JsValue> {
        to_js(&self.pack.structure())
    }

    /// The canonical form of a place as people write it (`Inst. 3.2.7` → `3.2.7`).
    #[wasm_bindgen(js_name = canonicalLocator)]
    pub fn canonical_locator(&self, input: &str) -> Result<String, JsValue> {
        self.pack.canonical_locator(input).map_err(throw)
    }

    /// One section.
    #[wasm_bindgen(unchecked_return_type = "WorkSection")]
    pub fn section(&self, locator: &str) -> Result<JsValue, JsValue> {
        to_js(&self.pack.section(locator).map_err(throw)?)
    }

    /// A chapter-level unit with its sections; a section's locator opens its chapter.
    #[wasm_bindgen(unchecked_return_type = "WorkChapter")]
    pub fn chapter(&self, locator: &str) -> Result<JsValue, JsValue> {
        to_js(&self.pack.chapter(locator).map_err(throw)?)
    }

    /// A section's citations, unresolved ones included.
    #[wasm_bindgen(unchecked_return_type = "WorkCitationLink[]")]
    pub fn citations(&self, locator: &str) -> Result<JsValue, JsValue> {
        to_js(&self.pack.citations(locator).map_err(throw)?)
    }

    /// The sections citing a passage or a verse inside it; `scheme` is the passage's
    /// versification when it is not `eng`.
    #[wasm_bindgen(js_name = citedBy, unchecked_return_type = "CitingSection[]")]
    pub fn cited_by(&self, passage: &str, scheme: Option<String>) -> Result<JsValue, JsValue> {
        to_js(
            &self
                .pack
                .cited_by(passage, scheme.as_deref())
                .map_err(throw)?,
        )
    }
}
