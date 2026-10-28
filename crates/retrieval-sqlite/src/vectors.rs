//! sqlite-vec: statically linked, registered once per process as an auto-extension so every
//! connection (packs and the user database alike) sees the `vec0` virtual table and the
//! `vec_*` functions.

#![allow(unsafe_code)]

use std::sync::Once;

use openlogos_retrieval::{Quantization, QueryVector, RetrievalError, VectorHit};
use rusqlite::{Connection, params};

static INSTALL: Once = Once::new();

/// Registers `sqlite3_vec_init` with `sqlite3_auto_extension`. Idempotent.
pub fn install_vec_extension() {
    INSTALL.call_once(|| {
        // SAFETY: sqlite3_vec_init is the extension's documented entry point, compiled from the
        // bundled sqlite-vec.c with SQLITE_CORE; sqlite3_auto_extension takes a pointer of the
        // generic `xEntryPoint` type, which sqlite-vec's own tests cast exactly this way.
        unsafe {
            let init: unsafe extern "C" fn() = sqlite_vec::sqlite3_vec_init;
            let entry: unsafe extern "C" fn(
                *mut rusqlite::ffi::sqlite3,
                *mut *mut std::ffi::c_char,
                *const rusqlite::ffi::sqlite3_api_routines,
            ) -> std::ffi::c_int = std::mem::transmute(init);
            rusqlite::ffi::sqlite3_auto_extension(Some(entry));
        }
    });
}

/// The sqlite-vec version string (`v0.1.9`).
pub fn vec_version(conn: &Connection) -> Result<String, rusqlite::Error> {
    conn.query_row("SELECT vec_version()", [], |r| r.get(0))
}

/// Parses `vector_set.quantization`.
pub fn parse_quantization(text: &str) -> Option<Quantization> {
    match text {
        "int8" => Some(Quantization::Int8),
        "bit" => Some(Quantization::Binary),
        _ => None,
    }
}

/// Runs a k-nearest-neighbour query against `schema.vec_<set>`.
pub fn knn(
    conn: &Connection,
    schema: &str,
    set: &str,
    quantization: Quantization,
    query: &QueryVector,
    k: usize,
) -> Result<Vec<VectorHit>, RetrievalError> {
    let (wrap, bytes): (&str, Vec<u8>) = match (quantization, query) {
        (Quantization::Int8, QueryVector::Int8(v)) => {
            ("vec_int8", v.iter().map(|b| *b as u8).collect())
        }
        (Quantization::Binary, QueryVector::Binary(v)) => ("vec_bit", v.clone()),
        _ => {
            return Err(RetrievalError::InvalidQuery(format!(
                "query vector quantization does not match vector set `{set}`"
            )));
        }
    };
    let sql = format!(
        "SELECT rowid, distance FROM \"{schema}\".vec_{set} WHERE embedding MATCH {wrap}(?1) AND k = ?2 ORDER BY distance"
    );
    let mut stmt = conn.prepare_cached(&sql).map_err(storage)?;
    let rows = stmt
        .query_map(params![bytes, k as i64], |r| {
            Ok(VectorHit {
                id: r.get(0)?,
                distance: r.get::<_, f64>(1)? as f32,
            })
        })
        .map_err(storage)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(storage)
}

/// Maps a SQLite error into the engine-neutral storage error.
pub fn storage(e: rusqlite::Error) -> RetrievalError {
    RetrievalError::Storage(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_extension_is_available_on_every_connection() {
        install_vec_extension();
        let conn = Connection::open_in_memory().unwrap();
        assert_eq!(vec_version(&conn).unwrap(), "v0.1.9");
        let other = Connection::open_in_memory().unwrap();
        assert!(vec_version(&other).is_ok());
    }

    #[test]
    fn binary_vectors_use_hamming_distance() {
        install_vec_extension();
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE VIRTUAL TABLE vec_t USING vec0(embedding bit[16]);
             INSERT INTO vec_t(rowid, embedding) VALUES (1, vec_bit(X'FFFF'));
             INSERT INTO vec_t(rowid, embedding) VALUES (2, vec_bit(X'0000'));
             INSERT INTO vec_t(rowid, embedding) VALUES (3, vec_bit(X'FF00'));",
        )
        .unwrap();
        let hits = knn(
            &conn,
            "main",
            "t",
            Quantization::Binary,
            &QueryVector::Binary(vec![0xFF, 0x0F]),
            3,
        )
        .unwrap();
        // Rows 1 and 3 tie at 4 bits; row 2 differs in 12.
        let mut first_two = [hits[0].id, hits[1].id];
        first_two.sort_unstable();
        assert_eq!(first_two, [1, 3]);
        assert_eq!(
            (hits[0].distance, hits[1].distance, hits[2].distance),
            (4.0, 4.0, 12.0)
        );
        assert_eq!(hits[2].id, 2);
        let err = knn(
            &conn,
            "main",
            "t",
            Quantization::Binary,
            &QueryVector::Int8(vec![0; 16]),
            1,
        )
        .unwrap_err();
        assert!(matches!(err, RetrievalError::InvalidQuery(_)));
    }
}
