//! B067: the alignment layers through UniFFI. `ShellHandle.use_alignments` attaches the
//! layers a build wrote; `token_alignment` answers a BSB word or selection with the aligned
//! Greek or Hebrew (with each word's morphology when the matching layer is attached through
//! `use_morphology`), or an original-language word with its BSB words; `lemma_renderings`
//! counts how the BSB renders a lemma, with scope and denominator. Records mirror
//! `openlogos_retrieval_sqlite::alignment`; statuses travel as their snake-case names
//! (`aligned`, `unaligned`, `not_joined`, `no_layer`) so a new value is not a breaking change.

use std::path::Path;

use openlogos_retrieval_sqlite::AlignmentLayers;
use openlogos_retrieval_sqlite::alignment as a;
use openlogos_workspaces::commands::ALIGNMENTS_LAYER;

use super::morphology::MorphologyMorpheme;
use super::shell::{ShellCommandError, ShellHandle, invalid};

/// One installed alignment layer.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AlignmentLayer {
    /// `clear-sblgnt-bsb`.
    pub alignment_id: String,
    /// Its name.
    pub title: String,
    /// The original-language edition (`sblgnt`).
    pub source_edition: String,
    /// The translation (`bsb`).
    pub target_edition: String,
    /// Its revision.
    pub revision: String,
    /// Licence.
    pub license: String,
    /// What a host shows with every answer.
    pub attribution: String,
    /// Where it came from.
    pub source: String,
    /// `curated:clear-alignments@c99bd0a`.
    pub origin: String,
    /// The source text pack revision.
    pub source_text_content_revision: String,
    /// Its tokenization hash.
    pub source_tokenization_hash: String,
    /// The morphology layer whose positions key the source words.
    pub source_morphology_layer: String,
    /// Its revision.
    pub source_morphology_revision: String,
    /// The target text pack revision.
    pub target_text_content_revision: String,
    /// Its tokenization hash.
    pub target_tokenization_hash: String,
}

impl From<a::AlignmentInfo> for AlignmentLayer {
    fn from(i: a::AlignmentInfo) -> Self {
        AlignmentLayer {
            alignment_id: i.alignment_id,
            title: i.title,
            source_edition: i.source_edition,
            target_edition: i.target_edition,
            revision: i.revision,
            license: i.license,
            attribution: i.attribution,
            source: i.source,
            origin: i.origin,
            source_text_content_revision: i.source_text_content_revision,
            source_tokenization_hash: i.source_tokenization_hash,
            source_morphology_layer: i.source_morphology_layer,
            source_morphology_revision: i.source_morphology_revision,
            target_text_content_revision: i.target_text_content_revision,
            target_tokenization_hash: i.target_tokenization_hash,
        }
    }
}

/// One word of the alignment inside a token asked about.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AlignmentWord {
    /// The curator's id.
    pub external_id: String,
    /// The curator's letters.
    pub text: String,
    /// Scalar range inside the token (BSB words).
    pub start: Option<u32>,
    /// End of it, exclusive.
    pub end: Option<u32>,
    /// The morpheme id, for an original-language word.
    pub morpheme_id: Option<String>,
    /// How it was joined (`exact`, `punctuation`, `shares_token`, …).
    pub join: String,
    /// `aligned` or `unaligned`.
    pub status: String,
    /// The records it is in.
    pub records: Vec<String>,
}

/// A token asked about.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AlignmentToken {
    /// The token.
    pub token_id: String,
    /// Its edition.
    pub edition: String,
    /// `aligned`, `unaligned`, `not_joined` or `no_layer`.
    pub status: String,
    /// Why not joined (`not_in_clear_target`, `not_in_alignment_source`, …).
    pub reason: Option<String>,
    /// The alignment's words inside it.
    pub words: Vec<AlignmentWord>,
}

/// A key and value of a record's metadata, as the curator gives them.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AlignmentMetaEntry {
    /// `origin`, `status`.
    pub key: String,
    /// `manual`, `created`.
    pub value: String,
}

/// An original-language word of a record.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AlignmentSourceWord {
    /// The text pack token; none when the word has none.
    pub token_id: Option<String>,
    /// The morpheme id.
    pub morpheme_id: Option<String>,
    /// MACULA `xml:id`.
    pub external_id: String,
    /// The curator's letters.
    pub text: String,
    /// How it was joined.
    pub join: String,
    /// Lemma, gloss and parsing from the morphology layer, when attached.
    pub morphology: Option<MorphologyMorpheme>,
}

/// A BSB word of a record.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AlignmentTargetWord {
    /// The text pack token holding it; none when not joined.
    pub token_id: Option<String>,
    /// Scalar range inside the token.
    pub start: Option<u32>,
    /// End of it, exclusive.
    pub end: Option<u32>,
    /// The curator's id.
    pub external_id: String,
    /// The curator's letters.
    pub text: String,
    /// How it was joined.
    pub join: String,
}

/// One alignment record.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AlignmentGroup {
    /// The layer.
    pub alignment_id: String,
    /// The curator's record id.
    pub record_id: String,
    /// `curated:<name>@<version>`.
    pub origin: String,
    /// The curator's metadata (`origin` = `manual`, `status` = `created`).
    pub meta: Vec<AlignmentMetaEntry>,
    /// Original-language words.
    pub sources: Vec<AlignmentSourceWord>,
    /// BSB words.
    pub targets: Vec<AlignmentTargetWord>,
    /// Whether the BSB words are not one run.
    pub discontinuous: bool,
}

/// What the layers say about a selection.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AlignmentAnswer {
    /// Per token asked about.
    pub tokens: Vec<AlignmentToken>,
    /// The records they are in, once each.
    pub groups: Vec<AlignmentGroup>,
    /// What to show with the answer.
    pub attribution: Vec<String>,
}

/// One rendering with its count.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AlignmentRenderingRow {
    /// The BSB words; empty for the unaligned row.
    pub rendering: String,
    /// `aligned` or `unaligned`.
    pub status: String,
    /// Occurrences.
    pub count: u64,
    /// Of those, in a record with other original-language words.
    pub with_other_source_words: u64,
    /// The first places (`John.1.1`).
    pub examples: Vec<String>,
}

/// How the BSB renders one lemma.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AlignmentRenderingTable {
    /// The lemma.
    pub lemma: String,
    /// Strong's number of its first occurrence.
    pub strong: String,
    /// `sblgnt`, `wlc`.
    pub source_edition: String,
    /// `bsb`.
    pub target_edition: String,
    /// The layer.
    pub alignment_id: String,
    /// The scope in words.
    pub scope: String,
    /// The denominator.
    pub occurrences: u64,
    /// In some record.
    pub aligned: u64,
    /// In no record.
    pub unaligned: u64,
    /// Not in the alignment's source text.
    pub not_covered: u64,
    /// Rows, most frequent first, the unaligned row last.
    pub rows: Vec<AlignmentRenderingRow>,
    /// What to show with the table.
    pub attribution: Vec<String>,
}

/// Rendering tables for a lemma query.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AlignmentRenderings {
    /// What was asked.
    pub query: String,
    /// `exact` or `key`.
    pub matched_by: String,
    /// Per lemma and layer.
    pub tables: Vec<AlignmentRenderingTable>,
    /// Editions without what the answer needs, with the reason.
    pub unavailable: Vec<String>,
    /// Renderings are evidence of translation choices, not definitions.
    pub note: String,
}

fn snake<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

impl From<a::AlignmentAnswer> for AlignmentAnswer {
    fn from(x: a::AlignmentAnswer) -> Self {
        AlignmentAnswer {
            tokens: x
                .tokens
                .into_iter()
                .map(|t| AlignmentToken {
                    status: snake(&t.status),
                    token_id: t.token_id,
                    edition: t.edition,
                    reason: t.reason,
                    words: t
                        .words
                        .into_iter()
                        .map(|w| AlignmentWord {
                            status: snake(&w.status),
                            external_id: w.external_id,
                            text: w.text,
                            start: w.start,
                            end: w.end,
                            morpheme_id: w.morpheme_id,
                            join: w.join,
                            records: w.records,
                        })
                        .collect(),
                })
                .collect(),
            groups: x
                .groups
                .into_iter()
                .map(|g| AlignmentGroup {
                    alignment_id: g.alignment_id,
                    record_id: g.record_id,
                    origin: g.origin,
                    meta: g
                        .meta
                        .into_iter()
                        .map(|(key, value)| AlignmentMetaEntry { key, value })
                        .collect(),
                    sources: g
                        .sources
                        .into_iter()
                        .map(|s| AlignmentSourceWord {
                            token_id: s.token_id,
                            morpheme_id: s.morpheme_id,
                            external_id: s.external_id,
                            text: s.text,
                            join: s.join,
                            morphology: s.morphology.map(Into::into),
                        })
                        .collect(),
                    targets: g
                        .targets
                        .into_iter()
                        .map(|t| AlignmentTargetWord {
                            token_id: t.token_id,
                            start: t.start,
                            end: t.end,
                            external_id: t.external_id,
                            text: t.text,
                            join: t.join,
                        })
                        .collect(),
                    discontinuous: g.discontinuous,
                })
                .collect(),
            attribution: x.attribution,
        }
    }
}

impl From<a::Renderings> for AlignmentRenderings {
    fn from(r: a::Renderings) -> Self {
        AlignmentRenderings {
            query: r.query,
            matched_by: r.matched_by,
            tables: r
                .tables
                .into_iter()
                .map(|t| AlignmentRenderingTable {
                    lemma: t.lemma,
                    strong: t.strong,
                    source_edition: t.source_edition,
                    target_edition: t.target_edition,
                    alignment_id: t.alignment_id,
                    scope: t.scope,
                    occurrences: t.occurrences,
                    aligned: t.aligned,
                    unaligned: t.unaligned,
                    not_covered: t.not_covered,
                    rows: t
                        .rows
                        .into_iter()
                        .map(|row| AlignmentRenderingRow {
                            status: snake(&row.status),
                            rendering: row.rendering,
                            count: row.count,
                            with_other_source_words: row.with_other_source_words,
                            examples: row.examples,
                        })
                        .collect(),
                    attribution: t.attribution,
                })
                .collect(),
            unavailable: r.unavailable,
            note: r.note,
        }
    }
}

/// The layers of an alignment build: `directory` is its output (holding `layers/`) or the
/// `layers/` directory itself.
pub(crate) fn open(directory: &Path) -> Result<AlignmentLayers, ShellCommandError> {
    let layers = directory.join("layers");
    let directory = if layers.is_dir() { &layers } else { directory };
    AlignmentLayers::open_directory(directory).map_err(invalid)
}

#[uniffi::export]
impl ShellHandle {
    /// B067: attaches the alignment layers under `directory` (a `just
    /// preview-pack-alignment` output, or its `layers/`), read-only, and adds `alignments` to
    /// the session's data layers (others stay), which enables "Original word". Returns the
    /// layers, whose attribution a host shows with their answers.
    pub fn use_alignments(
        &self,
        directory: String,
    ) -> Result<Vec<AlignmentLayer>, ShellCommandError> {
        let layers = open(Path::new(&directory))?;
        let infos = layers.layers();
        let mut state = self.lock();
        let mut registered = state.shell.session.data_layers.clone();
        registered.insert(ALIGNMENTS_LAYER.to_owned());
        state.shell.set_data_layers(registered);
        state.alignments = Some(layers);
        Ok(infos.into_iter().map(Into::into).collect())
    }

    /// The aligned words of a token or selection (`bsb:43001001004`, or
    /// `sblgnt:43003016003`, `wlc:01001001001b`): per token its status, and every record it
    /// is in with both sides' words; original-language words carry their morphology when
    /// `use_morphology` attached the layer the alignment was joined to. A token of an
    /// edition with no layer is `no_layer`.
    pub fn token_alignment(
        &self,
        token_ids: Vec<String>,
    ) -> Result<AlignmentAnswer, ShellCommandError> {
        let state = self.lock();
        let answer = match &state.alignments {
            Some(layers) => layers.align(&token_ids, state.morphology.as_ref()),
            None => AlignmentLayers::new().and_then(|l| l.align(&token_ids, None)),
        };
        answer.map(Into::into).map_err(invalid)
    }

    /// How the BSB renders a lemma (`λόγος`, or `λογος` by key) in `editions` (every
    /// aligned one when empty), in `book` (OSIS) or everywhere: counts with scope,
    /// denominator, unaligned and uncovered occurrences. Needs the morphology layers too.
    pub fn lemma_renderings(
        &self,
        editions: Vec<String>,
        query: String,
        book: Option<String>,
    ) -> Result<AlignmentRenderings, ShellCommandError> {
        let state = self.lock();
        let layers = state
            .alignments
            .as_ref()
            .ok_or_else(|| invalid("no alignment layer is installed"))?;
        let morphology = state
            .morphology
            .as_ref()
            .ok_or_else(|| invalid("no morphology layer is installed"))?;
        layers
            .renderings(morphology, &editions, &query, book.as_deref())
            .map(Into::into)
            .map_err(invalid)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use openlogos_retrieval::VerseId;
    use openlogos_retrieval_sqlite::alignment::{
        AlignmentInfo, AlignmentWriter, RecordRow, SourceWordRow, TargetWordRow,
    };
    use openlogos_retrieval_sqlite::morphology::{
        Language, LayerInfo, LayerWriter, MorphemeRow, Reading,
    };

    use super::*;

    fn build(dir: &Path) {
        std::fs::create_dir_all(dir.join("morphology/layers")).unwrap();
        std::fs::create_dir_all(dir.join("alignments/layers")).unwrap();
        let mut w = LayerWriter::create(
            &dir.join("morphology/layers/sblgnt.test.sqlite"),
            &LayerInfo {
                layer_id: "macula-greek".into(),
                edition: "sblgnt".into(),
                title: "Test".into(),
                revision: "m1".into(),
                license: "CC-BY-4.0".into(),
                attribution: "Morphology attribution".into(),
                source: "test".into(),
                text_content_revision: "t".into(),
                text_manifest_sha256: "0".repeat(64),
                excluded_columns: vec![],
            },
            "org",
            &[],
        )
        .unwrap();
        w.add(&MorphemeRow {
            verse: VerseId::new(44, 3, 16),
            word: 3,
            morpheme: 0,
            morphemes: 1,
            word_start: true,
            span: Some((0, 8)),
            surface: "ἠγάπησεν".into(),
            reading: Reading::Written,
            join: "exact".into(),
            external_id: "n43003016003".into(),
            language: Language::Greek,
            lemma: "ἀγαπάω".into(),
            strong: "G0025".into(),
            pos: "verb".into(),
            morph: "V-AAI-3S".into(),
            parsing: BTreeMap::new(),
            gloss: "loved".into(),
            english: String::new(),
            transliteration: String::new(),
        })
        .unwrap();
        w.finish().unwrap();
        let hash = "1".repeat(64);
        let mut w = AlignmentWriter::create(
            &dir.join("alignments/layers/sblgnt-bsb.test.sqlite"),
            &AlignmentInfo {
                alignment_id: "clear-sblgnt-bsb".into(),
                title: "Test".into(),
                source_edition: "sblgnt".into(),
                target_edition: "bsb".into(),
                revision: "a".into(),
                license: "CC-BY-4.0".into(),
                attribution: "Alignment attribution".into(),
                source: "test".into(),
                origin: "curated:clear-alignments@c99bd0a".into(),
                source_text_content_revision: "s".into(),
                source_text_manifest_sha256: "0".repeat(64),
                source_tokenization_hash: hash.clone(),
                source_morphology_layer: "macula-greek".into(),
                source_morphology_revision: "m1".into(),
                target_text_content_revision: "b".into(),
                target_text_manifest_sha256: "0".repeat(64),
                target_tokenization_hash: hash,
            },
        )
        .unwrap();
        let s = w.add_source(SourceWordRow {
            pos: 0,
            verse: VerseId::new(44, 3, 16),
            word: 3,
            morpheme: 0,
            morphemes: 1,
            external_id: "n43003016003".into(),
            text: "ἠγάπησεν".into(),
            join: "exact".into(),
        });
        let t = w.add_target(TargetWordRow {
            verse: VerseId::new(44, 3, 16),
            word: 4,
            span: Some((0, 5)),
            external_id: "43003016004".into(),
            text: "loved".into(),
            join: "exact".into(),
        });
        w.add_record(RecordRow {
            external_id: "43003016.003".into(),
            sources: vec![s],
            targets: vec![t],
            meta: BTreeMap::from([("origin".to_owned(), "manual".to_owned())]),
        })
        .unwrap();
        w.finish().unwrap();
    }

    #[test]
    fn alignments_answer_through_the_handle() {
        let dir = std::env::temp_dir().join(format!("openlogos-ffi-b067-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        build(&dir);
        let handle = ShellHandle::fixture();
        let none = handle
            .token_alignment(vec!["bsb:44003016004".into()])
            .unwrap();
        assert_eq!(none.tokens[0].status, "no_layer");
        assert!(
            handle
                .lemma_renderings(vec![], "ἀγαπάω".into(), None)
                .is_err()
        );
        let args = crate::native::shell::ShellCommandArgs {
            text: Some("loved".into()),
            reference: Some("John.3.16".into()),
            resource_id: Some("bsb".into()),
            ..Default::default()
        };
        assert!(
            handle
                .apply("nav.original_word".into(), args.clone(), true)
                .is_err()
        );
        handle.set_data_layers(vec!["cross_references".into()]);
        let layers = handle
            .use_alignments(dir.join("alignments").display().to_string())
            .unwrap();
        let outcome = handle
            .apply("nav.original_word".into(), args, true)
            .unwrap();
        assert!(!outcome.handled, "the host shows the alignment");
        assert!(
            handle
                .lock()
                .shell
                .session
                .data_layers
                .contains("cross_references"),
            "other layers stay registered"
        );
        assert_eq!(layers[0].origin, "curated:clear-alignments@c99bd0a");
        handle
            .use_morphology(dir.join("morphology").display().to_string())
            .unwrap();
        let answer = handle
            .token_alignment(vec!["bsb:44003016004".into()])
            .unwrap();
        assert_eq!(answer.tokens[0].status, "aligned");
        let group = &answer.groups[0];
        assert_eq!(group.meta[0].value, "manual");
        let m = group.sources[0].morphology.as_ref().unwrap();
        assert_eq!((m.lemma.as_str(), m.gloss.as_str()), ("ἀγαπάω", "loved"));
        let back = handle
            .token_alignment(vec!["sblgnt:44003016003".into()])
            .unwrap();
        assert_eq!(back.groups[0].targets[0].text, "loved");
        let r = handle
            .lemma_renderings(vec![], "αγαπαω".into(), Some("John".into()))
            .unwrap();
        assert_eq!(r.tables[0].occurrences, 1);
        assert_eq!(r.tables[0].rows[0].rendering, "loved");
        assert_eq!(r.tables[0].rows[0].status, "aligned");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
