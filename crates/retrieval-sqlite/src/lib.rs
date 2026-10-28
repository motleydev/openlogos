//! SQLite implementation of the retrieval boundary (ADR 0002), built and measured in B064.
//!
//! - [`tokenizer`]: the shared Greek/Hebrew normalizer (`ol-norm-1`).
//! - [`fts`]: that normalizer registered as the FTS5 tokenizer `openlogos`.

pub mod fts;
pub mod tokenizer;
