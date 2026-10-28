//! Registers the shared normalizer as the FTS5 tokenizer `openlogos`, so `MATCH` queries and
//! the index agree character for character. One FTS5 tokenizer object serves every table of
//! a connection; the callbacks hold no state, so no allocation crosses the boundary.
//!
//! The only `unsafe` in the crate lives here and in the extension loader of `vectors`: the
//! FTS5 tokenizer API is a C vtable obtained through the `fts5()` SQL function's pointer
//! argument (https://sqlite.org/fts5.html, "Extending FTS5").

#![allow(unsafe_code)]

use std::ffi::{c_char, c_int, c_void};
use std::ptr;

use rusqlite::Connection;
use rusqlite::ffi;

use crate::tokenizer::tokenize;

/// Name of the tokenizer inside `CREATE VIRTUAL TABLE ... USING fts5(..., tokenize = ...)`.
pub const TOKENIZER_NAME: &str = "openlogos";

/// Fetches the `fts5_api` pointer of a connection through the documented pointer-passing
/// interface: `SELECT fts5(?1)` with `?1` bound as a pointer of type `fts5_api_ptr`.
unsafe fn fts5_api(db: *mut ffi::sqlite3) -> Result<*mut ffi::fts5_api, rusqlite::Error> {
    let mut api: *mut ffi::fts5_api = ptr::null_mut();
    let mut stmt: *mut ffi::sqlite3_stmt = ptr::null_mut();
    // SAFETY: `db` is a live connection handle borrowed from rusqlite for the duration of the
    // call; every pointer passed is valid for the statement's lifetime and the statement is
    // finalized on every path.
    unsafe {
        let rc = ffi::sqlite3_prepare_v2(
            db,
            c"SELECT fts5(?1)".as_ptr(),
            -1,
            &mut stmt,
            ptr::null_mut(),
        );
        if rc != ffi::SQLITE_OK {
            return Err(rusqlite::Error::SqliteFailure(
                ffi::Error::new(rc),
                Some("prepare fts5()".into()),
            ));
        }
        let rc = ffi::sqlite3_bind_pointer(
            stmt,
            1,
            (&raw mut api).cast::<c_void>(),
            c"fts5_api_ptr".as_ptr(),
            None,
        );
        if rc == ffi::SQLITE_OK {
            ffi::sqlite3_step(stmt);
        }
        ffi::sqlite3_finalize(stmt);
    }
    if api.is_null() {
        return Err(rusqlite::Error::SqliteFailure(
            ffi::Error::new(ffi::SQLITE_ERROR),
            Some("fts5_api unavailable: SQLite was built without FTS5".into()),
        ));
    }
    Ok(api)
}

unsafe extern "C" fn x_create(
    _user: *mut c_void,
    _args: *mut *const c_char,
    _n_args: c_int,
    out: *mut *mut ffi::Fts5Tokenizer,
) -> c_int {
    // The tokenizer is stateless; FTS5 only needs a non-null handle. The dangling pointer is
    // never dereferenced (x_delete ignores it).
    // SAFETY: `out` is a valid out-pointer supplied by FTS5.
    unsafe { *out = ptr::NonNull::<ffi::Fts5Tokenizer>::dangling().as_ptr() };
    ffi::SQLITE_OK
}

unsafe extern "C" fn x_delete(_tokenizer: *mut ffi::Fts5Tokenizer) {}

type TokenCallback =
    unsafe extern "C" fn(*mut c_void, c_int, *const c_char, c_int, c_int, c_int) -> c_int;

unsafe extern "C" fn x_tokenize(
    _tokenizer: *mut ffi::Fts5Tokenizer,
    ctx: *mut c_void,
    _flags: c_int,
    text: *const c_char,
    n_text: c_int,
    x_token: Option<TokenCallback>,
) -> c_int {
    let Some(emit) = x_token else {
        return ffi::SQLITE_MISUSE;
    };
    if text.is_null() || n_text <= 0 {
        return ffi::SQLITE_OK;
    }
    // SAFETY: FTS5 guarantees `text` points at `n_text` readable bytes for this call.
    let bytes = unsafe { std::slice::from_raw_parts(text.cast::<u8>(), n_text as usize) };
    // FTS5 may hand over invalid UTF-8; index what is valid and leave the rest.
    let text = String::from_utf8_lossy(bytes);
    for token in tokenize(&text) {
        let Ok(len) = c_int::try_from(token.key.len()) else {
            continue;
        };
        let (Ok(start), Ok(end)) = (c_int::try_from(token.start), c_int::try_from(token.end))
        else {
            continue;
        };
        // SAFETY: `emit` is FTS5's callback; the key buffer outlives the call.
        let rc = unsafe { emit(ctx, 0, token.key.as_ptr().cast::<c_char>(), len, start, end) };
        if rc != ffi::SQLITE_OK {
            return rc;
        }
    }
    ffi::SQLITE_OK
}

/// Registers the `openlogos` tokenizer on a connection. Must run before any FTS5 table that
/// names it is created or queried on that connection.
pub fn register_tokenizer(conn: &Connection) -> Result<(), rusqlite::Error> {
    let mut vtable = ffi::fts5_tokenizer {
        xCreate: Some(x_create),
        xDelete: Some(x_delete),
        xTokenize: Some(x_tokenize),
    };
    // SAFETY: the connection handle is live while `conn` is borrowed; the vtable is copied by
    // FTS5 during xCreateTokenizer, so it may live on the stack.
    unsafe {
        let api = fts5_api(conn.handle())?;
        let create = (*api).xCreateTokenizer.ok_or_else(|| {
            rusqlite::Error::SqliteFailure(
                ffi::Error::new(ffi::SQLITE_ERROR),
                Some("fts5_api has no xCreateTokenizer".into()),
            )
        })?;
        let rc = create(
            api,
            c"openlogos".as_ptr(),
            ptr::null_mut(),
            &mut vtable,
            None,
        );
        if rc != ffi::SQLITE_OK {
            return Err(rusqlite::Error::SqliteFailure(
                ffi::Error::new(rc),
                Some("xCreateTokenizer".into()),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        register_tokenizer(&conn).unwrap();
        conn.execute_batch(
            "CREATE VIRTUAL TABLE t USING fts5(body, tokenize = 'openlogos');
             INSERT INTO t(rowid, body) VALUES (1, 'Ἐν ἀρχῇ ἦν ὁ λόγος, καὶ ὁ λόγος ἦν πρὸς τὸν θεόν');
             INSERT INTO t(rowid, body) VALUES (2, 'בְּרֵאשִׁ֖ית בָּרָ֣א אֱלֹהִ֑ים אֵ֥ת הַשָּׁמַ֖יִם');
             INSERT INTO t(rowid, body) VALUES (3, 'In the beginning was the Word');",
        )
        .unwrap();
        conn
    }

    fn ids(conn: &Connection, q: &str) -> Vec<i64> {
        let mut stmt = conn
            .prepare("SELECT rowid FROM t WHERE t MATCH ?1 ORDER BY rowid")
            .unwrap();
        stmt.query_map([q], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    #[test]
    fn unaccented_lowercase_query_finds_accented_text() {
        let conn = conn();
        assert_eq!(ids(&conn, "λογος"), vec![1]);
        assert_eq!(ids(&conn, "ΛΌΓΟΣ"), vec![1]);
        assert_eq!(ids(&conn, "θεον"), vec![1]);
        assert_eq!(ids(&conn, "θεος"), Vec::<i64>::new());
        assert_eq!(ids(&conn, "\"ο λογος\""), vec![1]);
    }

    #[test]
    fn consonantal_query_finds_pointed_hebrew() {
        let conn = conn();
        assert_eq!(ids(&conn, "בראשית"), vec![2]);
        assert_eq!(ids(&conn, "אלהים"), vec![2]);
        assert_eq!(ids(&conn, "\"ברא אלהים\""), vec![2]);
    }

    #[test]
    fn boolean_and_phrase_operators_work_through_the_tokenizer() {
        let conn = conn();
        assert_eq!(ids(&conn, "λογος AND θεον"), vec![1]);
        assert_eq!(ids(&conn, "λογος NOT θεον"), Vec::<i64>::new());
        assert_eq!(ids(&conn, "beginning OR αρχη"), vec![1, 3]);
    }

    #[test]
    fn highlight_offsets_point_at_the_original_bytes() {
        let conn = conn();
        let hl: String = conn
            .query_row(
                "SELECT highlight(t, 0, '[', ']') FROM t WHERE t MATCH 'λογος' AND rowid = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hl, "Ἐν ἀρχῇ ἦν ὁ [λόγος], καὶ ὁ [λόγος] ἦν πρὸς τὸν θεόν");
    }
}
