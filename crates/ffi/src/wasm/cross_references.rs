//! Cross references (B057) on the web: `ReadingHandle.crossReferences`, the source calls, and
//! `ShellHandle.setDataLayers`. Values are the core's JSON shapes, as everywhere on this
//! surface. A page fetches `relations/openbible-xrefs.sqlite` and hands its bytes to
//! `useCrossReferences` (`retrieval` feature); an item's `preview_chunk` names a chapter chunk
//! to fetch and `supplyChunk` before asking again for its preview.

use openlogos_text::reading::{CrossReferenceError, CrossReferenceOptions};
use wasm_bindgen::prelude::*;

use super::{ReadingHandle, ShellHandle, from_js, throw, to_js};

#[wasm_bindgen(typescript_custom_section)]
const TYPES: &str = r#"
/** What a cross-reference source is (B057). */
export interface ReadingCrossReferenceSource { source_id: string; edition_id: string; scheme: string; license: string; attribution: string; relation: string; kind: string; votes_meaning: string; review_state: string; coverage: string; rows: number }
/** One assertion behind an item: as published, or turned round for navigation. */
export interface ReadingCrossReferenceAssertion { from: string; to: string; votes: number; line: number; direction: "as_published" | "reverse_derived" }
/** One item: the target in the edition's numbering (null when it does not map), what the edition prints of it, the preview from the edition, and every assertion. */
export interface ReadingCrossReference { target: string | null; label: string; source_target: string; source_label: string; mapping: "exact" | "partial" | "ambiguous" | "unmapped"; availability: "printed" | "partly_printed" | "not_printed" | "book_not_in_edition" | "unmapped"; preview: string | null; preview_truncated: boolean; preview_chunk: ReadingChunk | null; relation: string; kind: string; direction: "as_published" | "reverse_derived"; votes: number; assertions: ReadingCrossReferenceAssertion[]; available_in: string[]; attribution: string }
/** The answer for one passage: the page asked for, the totals and the source. */
export interface ReadingCrossReferenceList { resource_id: string; reference: string; source_verses: string[]; mapping: "exact" | "partial" | "ambiguous" | "unmapped"; items: ReadingCrossReference[]; total: number; published: number; source: ReadingCrossReferenceSource }
/** What to ask for; every field may be left out (no reverse links, everything, three verses of preview). */
export interface ReadingCrossReferenceOptions { include_reverse?: boolean; limit?: number | null; offset?: number; preview_verses?: number }
"#;

fn refuse(error: CrossReferenceError) -> JsValue {
    match error {
        // A passage that does not locate throws the reading error itself (`verse_out_of_range`).
        CrossReferenceError::Reading { error } => throw(error),
        other => throw(other),
    }
}

#[wasm_bindgen(js_class = ReadingHandle)]
impl ReadingHandle {
    /// The cross references of a passage in an edition, published items first, best first by
    /// their votes; `options` pages the list and adds reverse links. Throws `not_installed`,
    /// `too_long`, `source` or a reading error's kind.
    #[wasm_bindgen(js_name = crossReferences, unchecked_return_type = "ReadingCrossReferenceList")]
    pub fn cross_references(
        &self,
        resource_id: &str,
        input: &str,
        #[wasm_bindgen(unchecked_param_type = "ReadingCrossReferenceOptions | undefined")]
        options: JsValue,
    ) -> Result<JsValue, JsValue> {
        let options: CrossReferenceOptions = if options.is_undefined() || options.is_null() {
            CrossReferenceOptions::default()
        } else {
            from_js("options", options)?
        };
        let list = self
            .library
            .cross_references(resource_id, input, &options)
            .map_err(refuse)?;
        to_js(&list)
    }

    /// The installed cross-reference source, or `null`.
    #[wasm_bindgen(
        js_name = crossReferenceSource,
        unchecked_return_type = "ReadingCrossReferenceSource | null"
    )]
    pub fn cross_reference_source(&self) -> Result<JsValue, JsValue> {
        to_js(&self.library.cross_reference_info())
    }

    /// Removes the installed cross-reference source.
    #[wasm_bindgen(js_name = clearCrossReferences)]
    pub fn clear_cross_references(&mut self) {
        self.library.clear_cross_references();
    }

    /// Installs a cross-reference pack from the bytes a page fetched; they are copied into
    /// SQLite's memory. Needs the `retrieval` feature.
    #[cfg(feature = "retrieval")]
    #[wasm_bindgen(
        js_name = useCrossReferences,
        unchecked_return_type = "ReadingCrossReferenceSource"
    )]
    pub fn use_cross_references(&mut self, pack: js_sys::Uint8Array) -> Result<JsValue, JsValue> {
        let pack = openlogos_retrieval_sqlite::CrossReferencePack::from_bytes(&pack.to_vec())
            .map_err(|detail| refuse(CrossReferenceError::Source { detail }))?;
        let info = openlogos_text::reading::CrossReferenceSource::info(&pack).clone();
        self.library.set_cross_references(Box::new(pack));
        to_js(&info)
    }
}

#[wasm_bindgen(js_class = ShellHandle)]
impl ShellHandle {
    /// Registers the data layers installed beside the editions (`["cross_references"]` once
    /// `useCrossReferences` succeeded), enabling the rows that need them.
    #[wasm_bindgen(js_name = setDataLayers)]
    pub fn set_data_layers(&mut self, layers: Vec<String>) {
        self.shell.set_data_layers(layers.into_iter().collect());
    }
}
