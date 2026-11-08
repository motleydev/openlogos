//! The B071 importer (specs/23 §4, specs/22 §7, docs/evidence/B071.md): BSB, SBLGNT and WLC from
//! the NAS artifacts into one text pack per edition and one FTS5 search pack per edition, outside
//! the repository. `just preview-pack` runs it; no corpus byte ever enters the repository.
//!
//! - [`rights`]: the rights record and hash gate, before any text is parsed.
//! - [`usfm`], [`oshb`], [`sblgnt`]: one parser per source format, each listing what it keeps.
//! - [`build`]: the edition definitions, the versification reconciliation against the canon table,
//!   the publisher's second form as an oracle (BSB), the pack and search writers.
//! - [`bench`]: text search latency against the 50 ms budget of specs/22 §7.
//!
//! [`institutes`] is the B016 importer beside it: Calvin's *Institutes* (Allen) into a work pack
//! (`just institutes-pack`, binary `openlogos-institutes-pack`).

pub mod bench;
pub mod build;
pub mod institutes;
pub mod model;
pub mod oshb;
pub mod rights;
pub mod sblgnt;
pub mod usfm;
