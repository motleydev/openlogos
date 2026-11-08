//! Joining Clear's source tokens to the SBLGNT and WLC pack tokens, through the B026
//! morphology layers.
//!
//! Clear's source tables (`SBLGNT+required.tsv`, `WLCM+required.tsv`) are MACULA's words and
//! morphemes under MACULA's own ids (`n40001001001`, `o010010010011`). The morphology layer
//! keeps that id beside every morpheme it joined to a pack token (B026 decision 4), so the
//! join is by id, exactly: an id the layer lacks stops nothing and is listed (`not_in_layer`).
//! The letters are then compared and classed, never used to choose:
//!
//! - `exact`: Clear's letters are the layer's.
//! - `normalization`: equal once both are NFC (John 7:53–8:11).
//! - `accents_omitted`: equal once the Hebrew cantillation marks, meteg and rafe the layer
//!   keeps are removed (Clear's WLCM table prints the words without accents).
//! - `movable_letter_omitted`: Clear has `οὐ` where the layer has `οὐκ` or `οὐχ`.
//! - `elision_omitted`: Clear has `ἐπ` where the layer has `ἐπ᾿` (the elision mark, and any
//!   punctuation after it, which MACULA keeps with the word).
//! - `no_pack_token`: the layer holds the morpheme but no pack token does (a qere with
//!   nothing written; a MACULA word B026 could not join).
//! - `surface_differs`: none of the above; not joined, listed.
//!
//! A layer morpheme Clear's table does not have (the article a prefix's vowel implies) is
//! listed as `not_in_alignment_source`; a pack token with no morpheme at all keeps the
//! layer's reason (`ketiv_not_read`, …).

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use openlogos_retrieval::VerseId;
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use unicode_normalization::UnicodeNormalization;

use super::clear::SourceToken;
use crate::model::ImportError;

/// One morpheme of a morphology layer, as the join needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerMorpheme {
    /// Its position.
    pub pos: u32,
    /// Its verse.
    pub verse: VerseId,
    /// The pack word number; 0 for none.
    pub word: u32,
    /// Index among the token's morphemes.
    pub morpheme: u8,
    /// The token's morphemes.
    pub morphemes: u8,
    /// MACULA `xml:id`.
    pub external_id: String,
    /// The layer's letters.
    pub surface: String,
}

/// A layer's morphemes in position order, and its manifest values the build checks.
#[derive(Debug, Clone)]
pub struct LayerIndex {
    /// The morphemes.
    pub morphemes: Vec<LayerMorpheme>,
    /// `layer_id`.
    pub layer_id: String,
    /// `layer_revision`.
    pub revision: String,
    /// `text_manifest_sha256`.
    pub text_manifest_sha256: String,
    /// `(verse, word)` → reason, for pack tokens the layer has no analysis for.
    pub absent: BTreeMap<(VerseId, u32), String>,
}

fn sql(path: &Path, error: rusqlite::Error) -> ImportError {
    ImportError::Pack(format!("{}: {error}", path.display()))
}

/// Reads a morphology layer file (read-only).
pub fn read_layer(path: &Path) -> Result<LayerIndex, ImportError> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| sql(path, e))?;
    let value = |key: &str| -> Result<String, ImportError> {
        conn.query_row("SELECT value FROM manifest WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .map_err(|e| sql(path, e))
    };
    let mut index = LayerIndex {
        morphemes: Vec::new(),
        layer_id: value("layer_id")?,
        revision: value("layer_revision")?,
        text_manifest_sha256: value("text_manifest_sha256")?,
        absent: BTreeMap::new(),
    };
    let mut stmt = conn
        .prepare(
            "SELECT t.pos, t.verse_id, t.word, a.morpheme, a.morphemes, a.external_id, t.surface
             FROM token t JOIN analysis a ON a.pos = t.pos ORDER BY t.pos",
        )
        .map_err(|e| sql(path, e))?;
    let mut rows = stmt.query([]).map_err(|e| sql(path, e))?;
    while let Some(r) = rows.next().map_err(|e| sql(path, e))? {
        let get = |i: usize| r.get::<_, i64>(i).map_err(|e| sql(path, e));
        index.morphemes.push(LayerMorpheme {
            pos: get(0)? as u32,
            verse: VerseId(get(1)? as u32),
            word: get(2)? as u32,
            morpheme: get(3)? as u8,
            morphemes: get(4)? as u8,
            external_id: r.get(5).map_err(|e| sql(path, e))?,
            surface: r.get(6).map_err(|e| sql(path, e))?,
        });
    }
    drop(rows);
    let mut stmt = conn
        .prepare("SELECT verse_id, word, reason FROM absence")
        .map_err(|e| sql(path, e))?;
    let mut rows = stmt.query([]).map_err(|e| sql(path, e))?;
    while let Some(r) = rows.next().map_err(|e| sql(path, e))? {
        let verse: i64 = r.get(0).map_err(|e| sql(path, e))?;
        let word: i64 = r.get(1).map_err(|e| sql(path, e))?;
        let reason: String = r.get(2).map_err(|e| sql(path, e))?;
        index
            .absent
            .insert((VerseId(verse as u32), word as u32), reason);
    }
    Ok(index)
}

/// Where one Clear source token went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinedSource {
    /// Index into Clear's table.
    pub clear: usize,
    /// Index into the layer's morphemes; `None` when not joined.
    pub morpheme: Option<usize>,
    /// The class.
    pub class: &'static str,
}

/// A place in the report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourcePlace {
    /// The pack token or morpheme, if any.
    pub token: Option<String>,
    /// Clear's id (MACULA's), or the pack token's verse.
    pub clear: String,
    /// Clear's letters.
    pub clear_text: String,
    /// The layer's letters.
    pub layer_text: String,
}

/// What the source join did.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SourceReport {
    /// Rows of Clear's table.
    pub clear_tokens: u64,
    /// Morphemes of the layer.
    pub layer_morphemes: u64,
    /// Clear tokens joined to a layer morpheme with a pack token.
    pub joined: u64,
    /// Clear tokens not joined to a pack token, each listed.
    pub not_joined: u64,
    /// Layer morphemes Clear's table does not have, each listed.
    pub layer_morphemes_not_in_clear: u64,
    /// Pack tokens the layer has no morpheme for, with the layer's reason.
    pub tokens_without_morphemes: u64,
    /// Clear tokens per class.
    pub classes: BTreeMap<String, u64>,
    /// Every place of every class but `exact` and `accents_omitted` (counted).
    pub places: BTreeMap<String, Vec<SourcePlace>>,
}

/// The join's output.
#[derive(Debug, Default)]
pub struct SourceJoin {
    /// One per Clear source token, in Clear's order.
    pub words: Vec<JoinedSource>,
    /// Layer morphemes Clear lacks, by index.
    pub not_in_clear: Vec<usize>,
    /// Counts and places.
    pub report: SourceReport,
}

fn is_elision(c: char) -> bool {
    matches!(c, '\u{02BC}' | '\u{2019}' | '\u{1FBD}' | '\u{1FBF}')
}

fn is_accent(c: char) -> bool {
    matches!(
        c,
        '\u{0591}'..='\u{05AF}' | '\u{05BD}' | '\u{05BF}' | '\u{05C4}' | '\u{05C5}'
    )
}

/// The class of Clear's letters against the layer's.
#[must_use]
pub fn letters_class(clear: &str, layer: &str) -> &'static str {
    if clear == layer {
        return "exact";
    }
    let nfc = |s: &str| s.nfc().collect::<String>();
    if nfc(clear) == nfc(layer) {
        return "normalization";
    }
    let bare = |s: &str| nfc(&s.chars().filter(|c| !is_accent(*c)).collect::<String>());
    if bare(clear) == bare(layer) {
        return "accents_omitted";
    }
    if let Some(rest) = nfc(layer).strip_prefix(nfc(clear).as_str()) {
        if matches!(rest, "κ" | "χ") {
            return "movable_letter_omitted";
        }
        if rest.chars().next().is_some_and(is_elision) && rest.chars().all(|c| !c.is_alphanumeric())
        {
            return "elision_omitted";
        }
    }
    "surface_differs"
}

fn morpheme_id(edition: &str, m: &LayerMorpheme) -> String {
    let mut id = format!(
        "{edition}:{:02}{:03}{:03}{:03}",
        m.verse.book(),
        m.verse.chapter(),
        m.verse.verse(),
        m.word
    );
    if m.morphemes >= 2 {
        id.push(char::from(b'a' + m.morpheme.min(25)));
    }
    id
}

/// The join. `pack_tokens` are the edition's pack tokens `(verse, word)`, to list the ones
/// with no morpheme.
#[must_use]
pub fn join(
    edition: &str,
    layer: &LayerIndex,
    clear: &[SourceToken],
    pack_tokens: &[(VerseId, u32)],
) -> SourceJoin {
    let mut out = SourceJoin::default();
    let report = &mut out.report;
    report.clear_tokens = clear.len() as u64;
    report.layer_morphemes = layer.morphemes.len() as u64;
    let by_id: HashMap<&str, usize> = layer
        .morphemes
        .iter()
        .enumerate()
        .map(|(i, m)| (m.external_id.as_str(), i))
        .collect();
    let mut seen = vec![false; layer.morphemes.len()];
    for (i, token) in clear.iter().enumerate() {
        let (morpheme, class) = match by_id.get(token.id.as_str()) {
            None => (None, "not_in_layer"),
            Some(m) => {
                seen[*m] = true;
                let lm = &layer.morphemes[*m];
                match letters_class(&token.text, &lm.surface) {
                    "surface_differs" => (None, "surface_differs"),
                    _ if lm.word == 0 => (Some(*m), "no_pack_token"),
                    class => (Some(*m), class),
                }
            }
        };
        *report.classes.entry(class.to_owned()).or_default() += 1;
        if morpheme.is_some() && class != "no_pack_token" {
            report.joined += 1;
        } else {
            report.not_joined += 1;
        }
        if !matches!(class, "exact" | "accents_omitted") {
            let lm = morpheme.map(|m| &layer.morphemes[m]);
            report
                .places
                .entry(class.to_owned())
                .or_default()
                .push(SourcePlace {
                    token: lm.filter(|m| m.word != 0).map(|m| morpheme_id(edition, m)),
                    clear: token.id.clone(),
                    clear_text: token.text.clone(),
                    layer_text: lm.map(|m| m.surface.clone()).unwrap_or_default(),
                });
        }
        out.words.push(JoinedSource {
            clear: i,
            morpheme,
            class,
        });
    }
    for (m, was_seen) in seen.iter().enumerate() {
        if *was_seen {
            continue;
        }
        let lm = &layer.morphemes[m];
        out.not_in_clear.push(m);
        report
            .places
            .entry("not_in_alignment_source".to_owned())
            .or_default()
            .push(SourcePlace {
                token: (lm.word != 0).then(|| morpheme_id(edition, lm)),
                clear: lm.external_id.clone(),
                clear_text: String::new(),
                layer_text: lm.surface.clone(),
            });
    }
    report.layer_morphemes_not_in_clear = out.not_in_clear.len() as u64;
    let with_morphemes: std::collections::HashSet<(VerseId, u32)> = layer
        .morphemes
        .iter()
        .filter(|m| m.word != 0)
        .map(|m| (m.verse, m.word))
        .collect();
    report.tokens_without_morphemes = pack_tokens
        .iter()
        .filter(|t| !with_morphemes.contains(t))
        .count() as u64;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_are_classed_not_used_to_choose() {
        assert_eq!(letters_class("λόγος", "λόγος"), "exact");
        assert_eq!(letters_class("οὐ", "οὐκ"), "movable_letter_omitted");
        assert_eq!(letters_class("οὐ", "οὐχ"), "movable_letter_omitted");
        assert_eq!(letters_class("רֵאשִׁית", "רֵאשִׁ֖ית"), "accents_omitted");
        assert_eq!(letters_class("מַּלְאכִים", "מַּלְאֿכִ֗ים"), "accents_omitted");
        // NFD and NFC of one word.
        assert_eq!(letters_class("\u{3b1}\u{301}", "\u{3ac}"), "normalization");
        assert_eq!(letters_class("λόγος", "λόγου"), "surface_differs");
        assert_eq!(letters_class("οὐ", "οὐδέ"), "surface_differs");
        assert_eq!(letters_class("ἐπ", "ἐπ\u{1FBF}"), "elision_omitted");
        assert_eq!(
            letters_class("ἀλλ", "ἀλλ\u{2019}\u{0387}"),
            "elision_omitted"
        );
        assert_eq!(letters_class("ἀλλ", "ἀλλά"), "surface_differs");
    }

    #[test]
    fn ids_join_and_every_leftover_is_listed() {
        let m = |pos: u32, word: u32, morpheme: u8, morphemes: u8, id: &str, surface: &str| {
            LayerMorpheme {
                pos,
                verse: VerseId::new(1, 1, 1),
                word,
                morpheme,
                morphemes,
                external_id: id.to_owned(),
                surface: surface.to_owned(),
            }
        };
        let layer = LayerIndex {
            morphemes: vec![
                m(0, 1, 0, 2, "o010010010011", "בְּ"),
                m(1, 1, 1, 2, "o010010010012", "רֵאשִׁ֖ית"),
                m(2, 2, 0, 2, "o010010010021ה", ""),
                m(3, 2, 1, 2, "o010010010022", "עוֹף"),
                m(4, 0, 0, 1, "o010010010031", "קְרָא"),
            ],
            layer_id: "macula-hebrew".into(),
            revision: "r".into(),
            text_manifest_sha256: "0".repeat(64),
            absent: BTreeMap::new(),
        };
        let clear = vec![
            SourceToken {
                id: "o010010010011".into(),
                text: "בְּ".into(),
            },
            SourceToken {
                id: "o010010010012".into(),
                text: "רֵאשִׁית".into(),
            },
            SourceToken {
                id: "o010010010022".into(),
                text: "עוֹף".into(),
            },
            SourceToken {
                id: "o010010010031".into(),
                text: "קְרָא".into(),
            },
            SourceToken {
                id: "o019990010011".into(),
                text: "x".into(),
            },
        ];
        let tokens = [
            (VerseId::new(1, 1, 1), 1),
            (VerseId::new(1, 1, 1), 2),
            (VerseId::new(1, 1, 1), 3),
        ];
        let joined = join("wlc", &layer, &clear, &tokens);
        let classes: Vec<&str> = joined.words.iter().map(|w| w.class).collect();
        assert_eq!(
            classes,
            [
                "exact",
                "accents_omitted",
                "exact",
                "no_pack_token",
                "not_in_layer"
            ]
        );
        assert_eq!(joined.report.joined, 3);
        assert_eq!(joined.report.not_joined, 2);
        assert_eq!(joined.not_in_clear, vec![2]);
        assert_eq!(joined.report.tokens_without_morphemes, 1);
        assert_eq!(
            joined.report.places["not_in_alignment_source"][0]
                .token
                .as_deref(),
            Some("wlc:01001001002a")
        );
    }
}
