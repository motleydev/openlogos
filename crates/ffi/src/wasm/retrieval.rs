//! The embedded retrieval engine on the web, behind the `retrieval` feature (B064 item 2).
//! This is the size and capability probe for the wasm path: SQLite compiled into the module
//! by sqlite-wasm-rs with FTS5 on, the `openlogos` tokenizer registered through the same
//! `fts5_api` code as on native, and the engine's normalizer. It opens an in-memory database
//! only; the OPFS `sahpool` VFS and attached packs are the R7 follow-up.

use openlogos_retrieval::TextIndex;
use openlogos_retrieval_sqlite::{Engine, sqlite_version};
use wasm_bindgen::prelude::*;

/// Runs one FTS5 round trip through the `openlogos` tokenizer and returns a summary line:
/// the SQLite version, the number of hits for an unaccented query against accented Greek,
/// the Hebrew hit count for a consonantal query, and the normalization mode.
#[wasm_bindgen(js_name = retrievalProbe)]
pub fn retrieval_probe() -> Result<String, JsValue> {
    let err = |e: String| JsValue::from_str(&e);
    let engine = Engine::new().map_err(|e| err(e.to_string()))?;
    let conn = engine.connection();
    conn.execute_batch(
        "CREATE VIRTUAL TABLE probe USING fts5(body, tokenize = 'openlogos');
         INSERT INTO probe(rowid, body) VALUES (1, 'Ἐν ἀρχῇ ἦν ὁ λόγος, καὶ ὁ λόγος ἦν πρὸς τὸν θεόν');
         INSERT INTO probe(rowid, body) VALUES (2, 'בְּרֵאשִׁ֖ית בָּרָ֣א אֱלֹהִ֑ים');",
    )
    .map_err(|e| err(e.to_string()))?;
    let count = |q: &str| -> Result<i64, JsValue> {
        conn.query_row(
            "SELECT count(*) FROM probe WHERE probe MATCH ?1",
            [q],
            |r| r.get(0),
        )
        .map_err(|e| err(e.to_string()))
    };
    let greek = count("λογος")?;
    let hebrew = count("ברא")?;
    Ok(format!(
        "sqlite {} fts5 greek_hits {greek} hebrew_hits {hebrew} mode {}",
        sqlite_version(),
        engine.normalization_mode()
    ))
}
