//! The embedded retrieval engine behind the `retrieval` feature (B064). This is the size
//! probe for benchmark R3: it reaches every part of `openlogos-retrieval-sqlite` that ships
//! on a device (bundled SQLite with FTS5 and the `openlogos` tokenizer, the token engine,
//! the link index, sqlite-vec), so the linker keeps them and the dylib's growth over the
//! default build is the engine's contribution. The real query API arrives with the search
//! crate; this function is deliberately a single smoke call.

use openlogos_retrieval::{
    Locator, QueryVector, RelationStore, TextIndex, TextQuery, TextStore, TokenEngine,
    TokenPattern, TokenQuery, VectorIndex, VerseId,
};
use openlogos_retrieval_sqlite::Engine;

/// What the smoke test saw.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RetrievalSmoke {
    /// SQLite library version compiled in.
    pub sqlite_version: String,
    /// Normalization mode of the FTS5 tokenizer.
    pub normalization_mode: String,
    /// Edition the pack serves.
    pub edition: String,
    /// Verses in the pack.
    pub verses: u64,
    /// Hits of a keyword query for `query` through FTS5.
    pub text_hits: u64,
    /// Matches of a lemma query for `query` through the token engine.
    pub lemma_hits: u64,
    /// Backlinks of the pack's first verse through the link index.
    pub backlinks: u64,
    /// Nearest neighbours returned by the first vector set, if any.
    pub nearest: u64,
}

/// Opens `pack_path` read-only, installs its edges into the link index at `link_index_path`
/// and runs one query of each kind for `query` (a word, searched as text and as lemma).
/// Errors are returned as text: the smoke test is diagnostic, not an API.
#[uniffi::export]
pub fn retrieval_smoke(
    pack_path: String,
    link_index_path: String,
    query: String,
) -> Result<RetrievalSmoke, String> {
    let run = || -> openlogos_retrieval::Result<RetrievalSmoke> {
        let mut engine = Engine::new()?.with_link_index(std::path::Path::new(&link_index_path))?;
        let info = engine.attach_pack(std::path::Path::new(&pack_path))?;
        if let Some(index) = engine.link_index_mut() {
            index.install_pack(&info.edition, std::path::Path::new(&pack_path))?;
        }
        let text_hits = engine
            .search_text(&TextQuery {
                text: query.clone(),
                editions: vec![info.edition.clone()],
                book: None,
                limit: 1000,
            })?
            .len() as u64;
        let lemma_hits = engine
            .token_search(&TokenQuery::single(
                &info.edition,
                TokenPattern::one("lemma", &query),
            ))?
            .count;
        let first_verse = engine
            .chapter_text(&info.edition, 1, 1)?
            .first()
            .map_or(VerseId(0), |(v, _)| *v);
        let backlinks = engine.backlink_count(&Locator::verse(&info.edition, first_verse), None)?;
        let nearest = match engine.vector_sets()?.first() {
            Some(set) => {
                let query = match set.quantization {
                    openlogos_retrieval::Quantization::Int8 => {
                        QueryVector::Int8(vec![0; set.dimensions as usize])
                    }
                    openlogos_retrieval::Quantization::Binary => {
                        QueryVector::Binary(vec![0; set.dimensions as usize / 8])
                    }
                };
                engine.nearest(&set.name, &query, 5)?.len() as u64
            }
            None => 0,
        };
        Ok(RetrievalSmoke {
            sqlite_version: rusqlite_version(),
            normalization_mode: engine.normalization_mode().to_owned(),
            edition: info.edition,
            verses: info.verses,
            text_hits,
            lemma_hits,
            backlinks,
            nearest,
        })
    };
    run().map_err(|e| e.to_string())
}

fn rusqlite_version() -> String {
    openlogos_retrieval_sqlite::sqlite_version().to_owned()
}
