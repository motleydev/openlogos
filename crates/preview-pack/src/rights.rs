//! The gate before a corpus byte is read: the rights record must say `approved`, the source's
//! `MANIFEST.sha256` must be the one the record names (and the copy kept in the repository), and
//! every file it lists must hash to what it lists. The bytes that passed are what the parsers
//! get, so nothing is read twice from the NAS. Nothing here writes.

use std::collections::BTreeMap;
use std::path::Path;

use openlogos_text::reading::pack::sha256_hex;
use serde_json::Value;

use crate::model::ImportError;

/// The permissions a local preview pack with search needs.
const NEEDED_PERMISSIONS: [&str; 2] = ["local_read", "public_index"];

/// A source that passed, with the fields of its record the pack carries.
#[derive(Debug)]
pub struct VerifiedSource {
    /// `S52`.
    pub source_id: String,
    /// `license_identifier` of the record.
    pub license: String,
    /// `attribution` of the record.
    pub attribution: String,
    /// `edition_id` of the record.
    pub edition_id: String,
    /// `acquisition.upstream_url`.
    pub upstream_url: String,
    /// `acquisition.commit`, if the record names one.
    pub commit: Option<String>,
    /// The SHA-256 of the source's `MANIFEST.sha256`.
    pub manifest_sha256: String,
    /// How many files the manifest listed and were checked.
    pub files_checked: usize,
    /// Bytes of the files checked.
    pub bytes_checked: u64,
    /// The bytes of the files `keep` asked for, by manifest path.
    pub files: BTreeMap<String, Vec<u8>>,
}

fn io(path: &Path, error: std::io::Error) -> ImportError {
    ImportError::Io {
        path: path.display().to_string(),
        error,
    }
}

/// Checks one source and returns the bytes of the files `keep` accepts.
pub fn verify_source(
    source_id: &str,
    rights_dir: &Path,
    corpus_dir: &Path,
    keep: impl Fn(&str) -> bool,
) -> Result<VerifiedSource, ImportError> {
    let refuse = |detail: String| ImportError::Rights {
        source_id: source_id.to_owned(),
        detail,
    };
    let record_path = rights_dir.join(format!("{source_id}.json"));
    let record: Value =
        serde_json::from_slice(&std::fs::read(&record_path).map_err(|e| io(&record_path, e))?)
            .map_err(|e| refuse(format!("the rights record does not parse: {e}")))?;
    let text = |key: &str| record.get(key).and_then(Value::as_str);
    if text("status") != Some("approved") {
        return Err(refuse(format!(
            "status is {:?}, not approved",
            text("status").unwrap_or("missing")
        )));
    }
    for permission in NEEDED_PERMISSIONS {
        let granted = record
            .pointer(&format!("/permissions/{permission}"))
            .and_then(Value::as_str);
        if granted != Some("allow") {
            return Err(refuse(format!("permission {permission} is {granted:?}")));
        }
    }
    let manifest_path = corpus_dir.join("MANIFEST.sha256");
    let manifest = std::fs::read(&manifest_path).map_err(|e| io(&manifest_path, e))?;
    let manifest_sha256 = sha256_hex(&manifest);
    let named = text("artifact_sha256").unwrap_or_default();
    if manifest_sha256 != named {
        return Err(refuse(format!(
            "MANIFEST.sha256 hashes to {manifest_sha256}, the record names {named}"
        )));
    }
    let kept = rights_dir.join(source_id).join("manifest.sha256");
    let kept = std::fs::read(&kept).map_err(|e| io(&kept, e))?;
    if kept != manifest {
        return Err(refuse(
            "MANIFEST.sha256 differs from the copy under rights/".to_owned(),
        ));
    }
    let listing = String::from_utf8(manifest)
        .map_err(|_| refuse("MANIFEST.sha256 is not UTF-8".to_owned()))?;
    let mut files = BTreeMap::new();
    let mut files_checked = 0;
    let mut bytes_checked = 0;
    for line in listing.lines().filter(|l| !l.trim().is_empty()) {
        let (hash, path) = line
            .split_once("  ")
            .ok_or_else(|| refuse(format!("manifest line: {line}")))?;
        if path.split('/').any(|p| p == ".." || p.is_empty()) {
            return Err(refuse(format!("manifest path: {path}")));
        }
        let full = corpus_dir.join(path);
        let bytes = std::fs::read(&full).map_err(|e| io(&full, e))?;
        let actual = sha256_hex(&bytes);
        if actual != hash {
            return Err(refuse(format!("{path} hashes to {actual}, listed {hash}")));
        }
        files_checked += 1;
        bytes_checked += bytes.len() as u64;
        if keep(path) {
            files.insert(path.to_owned(), bytes);
        }
    }
    let expected_files = record
        .pointer("/acquisition/file_count")
        .and_then(Value::as_u64);
    if expected_files != Some(files_checked as u64) {
        return Err(refuse(format!(
            "{files_checked} files listed, the record says {expected_files:?}"
        )));
    }
    for license in record
        .get("license_files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let path = license.get("path").and_then(Value::as_str).unwrap_or("");
        let hash = license.get("sha256").and_then(Value::as_str).unwrap_or("");
        let listed = listing.lines().any(|l| l == format!("{hash}  {path}"));
        if !listed {
            return Err(refuse(format!(
                "licence file {path} ({hash}) is not in the manifest"
            )));
        }
    }
    let field = |key: &str| -> Result<String, ImportError> {
        text(key)
            .map(str::to_owned)
            .ok_or_else(|| refuse(format!("the record has no {key}")))
    };
    Ok(VerifiedSource {
        source_id: source_id.to_owned(),
        license: field("license_identifier")?,
        attribution: field("attribution")?,
        edition_id: field("edition_id")?,
        upstream_url: record
            .pointer("/acquisition/upstream_url")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        commit: record
            .pointer("/acquisition/commit")
            .and_then(Value::as_str)
            .map(str::to_owned),
        manifest_sha256,
        files_checked,
        bytes_checked,
        files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake {
        rights: std::path::PathBuf,
        corpus: std::path::PathBuf,
    }

    /// A made-up source: one file, its manifest, its record, in a temporary directory.
    fn fake(name: &str) -> Fake {
        let root =
            std::env::temp_dir().join(format!("openlogos-rights-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (rights, corpus) = (root.join("rights"), root.join("corpus"));
        std::fs::create_dir_all(rights.join("S99")).unwrap();
        std::fs::create_dir_all(&corpus).unwrap();
        std::fs::write(corpus.join("data.txt"), b"not a corpus").unwrap();
        let listing = format!("{}  data.txt\n", sha256_hex(b"not a corpus"));
        std::fs::write(corpus.join("MANIFEST.sha256"), &listing).unwrap();
        std::fs::write(rights.join("S99/manifest.sha256"), &listing).unwrap();
        let record = serde_json::json!({
            "status": "approved",
            "artifact_sha256": sha256_hex(listing.as_bytes()),
            "license_identifier": "CC0-1.0",
            "attribution": "Test",
            "edition_id": "test:1",
            "permissions": {"local_read": "allow", "public_index": "allow"},
            "acquisition": {"file_count": 1, "upstream_url": "https://example.org"},
        });
        std::fs::write(rights.join("S99.json"), record.to_string()).unwrap();
        Fake { rights, corpus }
    }

    fn edit_record(fake: &Fake, change: impl Fn(&mut Value)) {
        let path = fake.rights.join("S99.json");
        let mut record: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        change(&mut record);
        std::fs::write(path, record.to_string()).unwrap();
    }

    #[test]
    fn an_approved_source_with_matching_hashes_passes_and_keeps_what_was_asked() {
        let f = fake("ok");
        let v = verify_source("S99", &f.rights, &f.corpus, |p| p == "data.txt").unwrap();
        assert_eq!(v.files_checked, 1);
        assert_eq!(v.files["data.txt"], b"not a corpus");
        assert_eq!(v.attribution, "Test");
    }

    #[test]
    fn a_status_other_than_approved_is_refused() {
        let f = fake("status");
        edit_record(&f, |r| r["status"] = "proposed".into());
        let error = verify_source("S99", &f.rights, &f.corpus, |_| true).unwrap_err();
        assert!(error.to_string().contains("not approved"), "{error}");
    }

    #[test]
    fn a_withheld_permission_is_refused() {
        let f = fake("permission");
        edit_record(&f, |r| r["permissions"]["public_index"] = "deny".into());
        assert!(verify_source("S99", &f.rights, &f.corpus, |_| true).is_err());
    }

    #[test]
    fn a_changed_file_is_refused() {
        let f = fake("file");
        std::fs::write(f.corpus.join("data.txt"), b"changed").unwrap();
        let error = verify_source("S99", &f.rights, &f.corpus, |_| true).unwrap_err();
        assert!(error.to_string().contains("hashes to"), "{error}");
    }

    #[test]
    fn a_changed_manifest_is_refused() {
        let f = fake("manifest");
        std::fs::write(f.corpus.join("MANIFEST.sha256"), "changed").unwrap();
        let error = verify_source("S99", &f.rights, &f.corpus, |_| true).unwrap_err();
        assert!(error.to_string().contains("the record names"), "{error}");
    }

    #[test]
    fn a_manifest_with_the_wrong_file_count_is_refused() {
        let f = fake("count");
        edit_record(&f, |r| r["acquisition"]["file_count"] = 2.into());
        assert!(verify_source("S99", &f.rights, &f.corpus, |_| true).is_err());
    }
}
