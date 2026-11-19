//! B026: the morphology layers through UniFFI. `ShellHandle.use_morphology` attaches the
//! layers a build wrote and enables Search lemma on their editions' words;
//! `token_analysis`, `lemma_search` and `lemma_search_token` answer from them. Records mirror
//! `openlogos_retrieval_sqlite::morphology`; enums travel as their snake-case names
//! (`analyzed`, `qere`, `hbo`) so a new value is not a breaking change.

use std::path::Path;

use openlogos_retrieval_sqlite::MorphologyLayers;
use openlogos_retrieval_sqlite::morphology as m;

use super::shell::{ShellCommandError, ShellHandle, invalid};

/// One installed layer.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MorphologyLayer {
    /// `macula-greek`.
    pub layer_id: String,
    /// The edition it analyses (`sblgnt`).
    pub edition: String,
    /// Its name (`MACULA Greek`).
    pub title: String,
    /// Its revision.
    pub revision: String,
    /// Licence.
    pub license: String,
    /// What a host shows with every analysis.
    pub attribution: String,
    /// Where it came from.
    pub source: String,
    /// The text pack revision its tokens belong to.
    pub text_content_revision: String,
    /// The source columns not carried (restricted by the rights record).
    pub excluded_columns: Vec<String>,
}

/// A parsing field.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MorphologyFeature {
    /// `tense`, `stem`, …
    pub name: String,
    /// The source's value.
    pub value: String,
}

/// One morpheme of an analysis.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MorphologyMorpheme {
    /// `wlc:01001001001b`; `None` for a qere with no written token.
    pub id: Option<String>,
    /// MACULA `xml:id`.
    pub external_id: String,
    /// Its letters; empty for an implied article.
    pub surface: String,
    /// Scalar range in the token's text, when written there.
    pub start: Option<u32>,
    /// End of it, exclusive.
    pub end: Option<u32>,
    /// No letters of its own.
    pub implied: bool,
    /// `grc`, `hbo`, `arc`.
    pub language: String,
    /// Lemma.
    pub lemma: String,
    /// Strong's number as the source gives it.
    pub strong: String,
    /// Part of speech.
    pub pos: String,
    /// The source's code.
    pub morph_code: String,
    /// Decoded fields.
    pub features: Vec<MorphologyFeature>,
    /// The parsing in words.
    pub description: String,
    /// Gloss.
    pub gloss: String,
    /// English.
    pub english: String,
    /// Transliteration.
    pub transliteration: String,
}

/// One layer's analysis of a token.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MorphologyAnalysis {
    /// The layer.
    pub layer_id: String,
    /// Its title.
    pub layer_title: String,
    /// Show with the analysis.
    pub attribution: String,
    /// `written`, `qere` or `qere_without_ketiv`.
    pub reading: String,
    /// How the word was joined (`exact`, `punctuation`, `qere`, …).
    pub join: String,
    /// The analysed reading's letters.
    pub text: String,
    /// The morphemes.
    pub morphemes: Vec<MorphologyMorpheme>,
}

/// What the layers say about a token.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MorphologyTokenAnalysis {
    /// The token.
    pub token_id: String,
    /// Its edition.
    pub edition: String,
    /// `analyzed`, `conflicting` or `absent`.
    pub status: String,
    /// `no_layer`, `ketiv_not_read`, `not_analyzed`, … when absent.
    pub absent_reason: Option<String>,
    /// One per layer.
    pub analyses: Vec<MorphologyAnalysis>,
}

/// A lemma a search matched.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MorphologyLemmaCount {
    /// The edition.
    pub edition: String,
    /// The lemma.
    pub lemma: String,
    /// Strong's number(s).
    pub strong: String,
    /// `grc`, `hbo`, `arc`.
    pub language: String,
    /// Occurrences.
    pub count: u64,
}

/// One occurrence.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MorphologyLemmaHit {
    /// The edition.
    pub edition: String,
    /// OSIS verse, also the segment id.
    pub reference: String,
    /// The token, if written.
    pub token_id: Option<String>,
    /// The morpheme.
    pub morpheme_id: Option<String>,
    /// Its letters.
    pub surface: String,
    /// Its lemma.
    pub lemma: String,
    /// `written`, `qere`, `qere_without_ketiv`.
    pub reading: String,
}

/// A lemma search's answer.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MorphologyLemmaSearch {
    /// What was asked.
    pub query: String,
    /// `exact` or `key`.
    pub matched_by: String,
    /// The lemmas matched, most frequent first.
    pub lemmas: Vec<MorphologyLemmaCount>,
    /// Every match.
    pub total: u64,
    /// The first matches.
    pub hits: Vec<MorphologyLemmaHit>,
    /// Editions asked for with no layer.
    pub unavailable: Vec<String>,
}

fn snake<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

impl From<m::LayerInfo> for MorphologyLayer {
    fn from(l: m::LayerInfo) -> Self {
        MorphologyLayer {
            layer_id: l.layer_id,
            edition: l.edition,
            title: l.title,
            revision: l.revision,
            license: l.license,
            attribution: l.attribution,
            source: l.source,
            text_content_revision: l.text_content_revision,
            excluded_columns: l.excluded_columns,
        }
    }
}

impl From<m::Morpheme> for MorphologyMorpheme {
    fn from(x: m::Morpheme) -> Self {
        MorphologyMorpheme {
            id: x.id,
            external_id: x.external_id,
            surface: x.surface,
            start: x.start,
            end: x.end,
            implied: x.implied,
            language: x.language.tag().to_owned(),
            lemma: x.lemma,
            strong: x.strong,
            pos: x.pos,
            morph_code: x.morph_code,
            features: x
                .features
                .into_iter()
                .map(|f| MorphologyFeature {
                    name: f.name,
                    value: f.value,
                })
                .collect(),
            description: x.description,
            gloss: x.gloss,
            english: x.english,
            transliteration: x.transliteration,
        }
    }
}

impl From<m::TokenAnalysis> for MorphologyTokenAnalysis {
    fn from(a: m::TokenAnalysis) -> Self {
        MorphologyTokenAnalysis {
            status: snake(&a.status),
            token_id: a.token_id,
            edition: a.edition,
            absent_reason: a.absent_reason,
            analyses: a
                .analyses
                .into_iter()
                .map(|x| MorphologyAnalysis {
                    layer_id: x.layer_id,
                    layer_title: x.layer_title,
                    attribution: x.attribution,
                    reading: x.reading.as_str().to_owned(),
                    join: x.join,
                    text: x.text,
                    morphemes: x.morphemes.into_iter().map(Into::into).collect(),
                })
                .collect(),
        }
    }
}

impl From<m::LemmaSearch> for MorphologyLemmaSearch {
    fn from(s: m::LemmaSearch) -> Self {
        MorphologyLemmaSearch {
            query: s.query,
            matched_by: s.matched_by,
            lemmas: s
                .lemmas
                .into_iter()
                .map(|l| MorphologyLemmaCount {
                    edition: l.edition,
                    lemma: l.lemma,
                    strong: l.strong,
                    language: l.language.tag().to_owned(),
                    count: l.count,
                })
                .collect(),
            total: s.total,
            hits: s
                .hits
                .into_iter()
                .map(|h| MorphologyLemmaHit {
                    edition: h.edition,
                    reference: h.reference,
                    token_id: h.token_id,
                    morpheme_id: h.morpheme_id,
                    surface: h.surface,
                    lemma: h.lemma,
                    reading: h.reading.as_str().to_owned(),
                })
                .collect(),
            unavailable: s.unavailable,
        }
    }
}

/// The layers of a morphology build: `directory` is its output (holding `layers/`) or the
/// `layers/` directory itself.
pub(crate) fn open(directory: &Path) -> Result<MorphologyLayers, ShellCommandError> {
    let layers = directory.join("layers");
    let directory = if layers.is_dir() { &layers } else { directory };
    MorphologyLayers::open_directory(directory).map_err(invalid)
}

#[uniffi::export]
impl ShellHandle {
    /// B026: attaches the morphology layers under `directory` (a `just
    /// preview-pack-morphology` output, or its `layers/`), read-only, and enables Search
    /// lemma on the words of their editions. Returns the layers, whose attribution a host
    /// shows with their analyses.
    pub fn use_morphology(
        &self,
        directory: String,
    ) -> Result<Vec<MorphologyLayer>, ShellCommandError> {
        let layers = open(Path::new(&directory))?;
        let infos = layers.layers();
        let mut state = self.lock();
        state
            .shell
            .set_morphology_editions(layers.editions().into_iter().collect());
        state.morphology = Some(layers);
        Ok(infos.into_iter().map(Into::into).collect())
    }

    /// What the installed layers say about a token (`wlc:01001001001`); `absent` with
    /// `no_layer` when none analyses its edition.
    pub fn token_analysis(
        &self,
        token_id: String,
    ) -> Result<MorphologyTokenAnalysis, ShellCommandError> {
        let state = self.lock();
        let answer = match &state.morphology {
            Some(layers) => layers.analyze(&token_id),
            None => MorphologyLayers::new().and_then(|l| l.analyze(&token_id)),
        };
        answer.map(Into::into).map_err(invalid)
    }

    /// Every occurrence of a lemma in `editions` (all with a layer when empty): exact when
    /// the query is spelled as a lemma, else by the search key (`ראשית`, `λογος`).
    pub fn lemma_search(
        &self,
        editions: Vec<String>,
        query: String,
        limit: u32,
    ) -> Result<MorphologyLemmaSearch, ShellCommandError> {
        let state = self.lock();
        let answer = match &state.morphology {
            Some(layers) => layers.lemma_search(&editions, &query, limit as usize),
            None => MorphologyLayers::new()
                .and_then(|l| l.lemma_search(&editions, &query, limit as usize)),
        };
        answer.map(Into::into).map_err(invalid)
    }

    /// Search lemma for a word: the token's principal lemma in its edition.
    pub fn lemma_search_token(
        &self,
        token_id: String,
        limit: u32,
    ) -> Result<MorphologyLemmaSearch, ShellCommandError> {
        let state = self.lock();
        let layers = state
            .morphology
            .as_ref()
            .ok_or_else(|| invalid("no morphology layer is installed"))?;
        layers
            .lemma_search_token(&token_id, limit as usize)
            .map(Into::into)
            .map_err(invalid)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use openlogos_retrieval::VerseId;
    use openlogos_retrieval_sqlite::morphology::{
        Language, LayerInfo, LayerWriter, MorphemeRow, Reading,
    };

    use super::*;

    fn layer(dir: &Path) {
        std::fs::create_dir_all(dir.join("layers")).unwrap();
        let info = LayerInfo {
            layer_id: "test".into(),
            edition: "sblgnt".into(),
            title: "Test".into(),
            revision: "r".into(),
            license: "CC-BY-4.0".into(),
            attribution: "Test attribution".into(),
            source: "test".into(),
            text_content_revision: "t".into(),
            text_manifest_sha256: "0".repeat(64),
            excluded_columns: vec!["ln".into(), "domain".into()],
        };
        let mut w = LayerWriter::create(
            &dir.join("layers/sblgnt.test.sqlite"),
            &info,
            "org",
            &["tense"],
        )
        .unwrap();
        w.add(&MorphemeRow {
            verse: VerseId::new(43, 3, 16),
            word: 4,
            morpheme: 0,
            morphemes: 1,
            word_start: true,
            span: Some((0, 8)),
            surface: "ἠγάπησεν".into(),
            reading: Reading::Written,
            join: "exact".into(),
            external_id: "n43003016004".into(),
            language: Language::Greek,
            lemma: "ἀγαπάω".into(),
            strong: "25".into(),
            pos: "verb".into(),
            morph: "V-AAI-3S".into(),
            parsing: BTreeMap::from([("tense".to_owned(), "aorist".to_owned())]),
            gloss: "loved".into(),
            english: "loved".into(),
            transliteration: String::new(),
        })
        .unwrap();
        w.finish().unwrap();
    }

    #[test]
    fn layers_enable_search_lemma_and_answer_through_the_handle() {
        let dir = std::env::temp_dir().join(format!("openlogos-ffi-b026-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        layer(&dir);
        let handle = ShellHandle::fixture();
        let none = handle.token_analysis("sblgnt:43003016004".into()).unwrap();
        assert_eq!(
            (none.status.as_str(), none.absent_reason.as_deref()),
            ("absent", Some("no_layer"))
        );
        assert!(
            handle
                .lemma_search_token("sblgnt:43003016004".into(), 5)
                .is_err()
        );
        let layers = handle.use_morphology(dir.display().to_string()).unwrap();
        assert_eq!(layers[0].excluded_columns, ["ln", "domain"]);
        let a = handle.token_analysis("sblgnt:43003016004".into()).unwrap();
        assert_eq!(a.status, "analyzed");
        assert_eq!(a.analyses[0].reading, "written");
        assert_eq!(a.analyses[0].attribution, "Test attribution");
        assert_eq!(a.analyses[0].morphemes[0].language, "grc");
        assert_eq!(a.analyses[0].morphemes[0].description, "verb · aorist");
        let found = handle.lemma_search(vec![], "αγαπαω".into(), 10).unwrap();
        assert_eq!((found.matched_by.as_str(), found.total), ("key", 1));
        assert_eq!(
            found.hits[0].token_id.as_deref(),
            Some("sblgnt:43003016004")
        );
        assert_eq!(
            handle
                .lemma_search_token("sblgnt:43003016004".into(), 5)
                .unwrap()
                .total,
            1
        );
        // Search lemma runs (in the host) on a word of the layer's edition, and still says
        // what it needs on another.
        let args = |resource: &str| crate::native::shell::ShellCommandArgs {
            text: Some("ἠγάπησεν".into()),
            resource_id: Some(resource.into()),
            ..Default::default()
        };
        let outcome = handle
            .apply("search.lemma".into(), args("sblgnt"), true)
            .unwrap();
        assert!(!outcome.handled);
        assert!(
            handle
                .apply("search.lemma".into(), args("bsb"), true)
                .is_err()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
