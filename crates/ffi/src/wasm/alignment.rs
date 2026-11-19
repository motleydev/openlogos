//! B067: the alignment layers on the web, behind the `retrieval` feature. The same three
//! calls as the native `ShellHandle`: `useAlignments` attaches the layers a page fetched
//! (`layers/*.sqlite` of an alignment build), `tokenAlignment` answers a word or selection,
//! `lemmaRenderings` counts how the BSB renders a lemma. Values are the core's JSON shapes.

use wasm_bindgen::prelude::*;

use super::{ShellHandle, to_js};

#[wasm_bindgen(typescript_custom_section)]
const TYPES: &str = r#"
export interface AlignmentLayer { alignment_id: string; title: string; source_edition: string; target_edition: string; revision: string; license: string; attribution: string; source: string; origin: string; source_text_content_revision: string; source_text_manifest_sha256: string; source_tokenization_hash: string; source_morphology_layer: string; source_morphology_revision: string; target_text_content_revision: string; target_text_manifest_sha256: string; target_tokenization_hash: string }
export type AlignmentStatus = "aligned" | "unaligned" | "not_joined" | "no_layer";
export interface AlignmentWord { external_id: string; text: string; start: number | null; end: number | null; morpheme_id: string | null; join: string; status: AlignmentStatus; records: string[] }
export interface AlignmentToken { token_id: string; edition: string; status: AlignmentStatus; reason: string | null; words: AlignmentWord[] }
export interface AlignmentSourceWord { token_id: string | null; morpheme_id: string | null; external_id: string; text: string; join: string; morphology: MorphologyMorpheme | null }
export interface AlignmentTargetWord { token_id: string | null; start: number | null; end: number | null; external_id: string; text: string; join: string }
export interface AlignmentGroup { alignment_id: string; record_id: string; origin: string; meta: Record<string, string>; sources: AlignmentSourceWord[]; targets: AlignmentTargetWord[]; discontinuous: boolean }
export interface AlignmentAnswer { tokens: AlignmentToken[]; groups: AlignmentGroup[]; attribution: string[] }
export interface AlignmentRenderingRow { rendering: string; status: AlignmentStatus; count: number; with_other_source_words: number; examples: string[] }
export interface AlignmentRenderingTable { lemma: string; strong: string; source_edition: string; target_edition: string; alignment_id: string; scope: string; occurrences: number; aligned: number; unaligned: number; not_covered: number; rows: AlignmentRenderingRow[]; attribution: string[] }
export interface AlignmentRenderings { query: string; matched_by: "exact" | "key"; tables: AlignmentRenderingTable[]; unavailable: string[]; note: string }
"#;

fn err(e: openlogos_retrieval::RetrievalError) -> JsValue {
    JsValue::from_str(&e.to_string())
}

#[wasm_bindgen(js_class = ShellHandle)]
impl ShellHandle {
    /// B067: attaches the alignment layers a page fetched, each as a `Uint8Array`, and adds
    /// `alignments` to the session's data layers (others stay), which enables "Original
    /// word". Returns the layers, whose attribution a host shows with their answers.
    #[wasm_bindgen(js_name = useAlignments, unchecked_return_type = "AlignmentLayer[]")]
    pub fn use_alignments(&mut self, layers: Vec<js_sys::Uint8Array>) -> Result<JsValue, JsValue> {
        let mut attached = openlogos_retrieval_sqlite::AlignmentLayers::new().map_err(err)?;
        for layer in layers {
            attached.attach_bytes(&layer.to_vec()).map_err(err)?;
        }
        let infos = attached.layers();
        let mut registered = self.shell.session.data_layers.clone();
        registered.insert(openlogos_workspaces::commands::ALIGNMENTS_LAYER.to_owned());
        self.shell.set_data_layers(registered);
        self.alignments = Some(attached);
        to_js(&infos)
    }

    /// The aligned words of a token or selection (`bsb:43001001004`, `sblgnt:…`, `wlc:…b`),
    /// with each original-language word's morphology when `useMorphology` attached the
    /// layer the alignment was joined to; `no_layer` for an edition with none.
    #[wasm_bindgen(js_name = tokenAlignment, unchecked_return_type = "AlignmentAnswer")]
    pub fn token_alignment(&self, token_ids: Vec<String>) -> Result<JsValue, JsValue> {
        let answer = match &self.alignments {
            Some(layers) => layers.align(&token_ids, self.morphology.as_ref()),
            None => openlogos_retrieval_sqlite::AlignmentLayers::new()
                .and_then(|l| l.align(&token_ids, None)),
        };
        to_js(&answer.map_err(err)?)
    }

    /// How the BSB renders a lemma in `editions` (every aligned one when empty), in `book`
    /// (OSIS) or everywhere, with scope and denominator. Needs the morphology layers too.
    #[wasm_bindgen(js_name = lemmaRenderings, unchecked_return_type = "AlignmentRenderings")]
    pub fn lemma_renderings(
        &self,
        editions: Vec<String>,
        query: &str,
        book: Option<String>,
    ) -> Result<JsValue, JsValue> {
        let layers = self
            .alignments
            .as_ref()
            .ok_or_else(|| JsValue::from_str("no alignment layer is installed"))?;
        let morphology = self
            .morphology
            .as_ref()
            .ok_or_else(|| JsValue::from_str("no morphology layer is installed"))?;
        to_js(
            &layers
                .renderings(morphology, &editions, query, book.as_deref())
                .map_err(err)?,
        )
    }
}
