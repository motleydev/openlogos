//! The version policy of docs/adr/0003-identity-and-schema-versioning.md, as code.
//!
//! Two independent version lines exist. [`crate::API_VERSION`] names the core API the
//! bindings are generated against; [`api_compatible`] says whether a host built against one
//! version may load a core reporting another. Every serialized record (pack manifest,
//! collection, export receipt, lineage, alignment) carries a `schema_version` of the form
//! `MAJOR.MINOR`; [`SchemaVersion::compatibility`] says what a reader that implements one
//! version does with a file written at another.

use std::fmt;

use serde::{Deserialize, Serialize};

/// `MAJOR.MINOR` of a serialized record's schema.
///
/// MINOR bumps are additive: new optional fields with defaults, new enum values a reader may
/// skip. MAJOR bumps change or remove something a reader relies on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SchemaVersion {
    /// Incompatible line.
    pub major: u32,
    /// Additive step within the line.
    pub minor: u32,
}

/// What a reader does with a file, given both schema versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Compatibility {
    /// Same version: read as is.
    Same,
    /// The file is older within the reader's line: read it, filling the defaults of every
    /// field added since (the migration path of ADR 0003), and write it back only at the
    /// reader's version.
    OlderMinor,
    /// The file is newer within the reader's line: read it, ignoring fields and enum values
    /// the reader does not know, and never write it back (a write would drop them).
    NewerMinor,
    /// The file is from an older line: refuse unless an explicit major migration exists.
    OlderMajor,
    /// The file is from a newer line: refuse and tell the user to update.
    NewerMajor,
}

impl Compatibility {
    /// Whether the reader may parse the file at all.
    #[must_use]
    pub fn readable(self) -> bool {
        matches!(
            self,
            Compatibility::Same | Compatibility::OlderMinor | Compatibility::NewerMinor
        )
    }

    /// Whether the reader may write the record back at its own version without losing
    /// anything the file carried.
    #[must_use]
    pub fn writable(self) -> bool {
        matches!(self, Compatibility::Same | Compatibility::OlderMinor)
    }
}

/// A version string that is not `MAJOR.MINOR` (or `MAJOR.MINOR.PATCH` for the API).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[error("`{text}` is not a version of the form {expected}")]
pub struct VersionSyntax {
    /// The text as given.
    pub text: String,
    /// The form expected.
    pub expected: String,
}

impl SchemaVersion {
    /// A version.
    #[must_use]
    pub fn new(major: u32, minor: u32) -> SchemaVersion {
        SchemaVersion { major, minor }
    }

    /// Parses `MAJOR.MINOR`.
    pub fn parse(text: &str) -> Result<SchemaVersion, VersionSyntax> {
        let syntax = || VersionSyntax {
            text: text.to_owned(),
            expected: "MAJOR.MINOR".to_owned(),
        };
        let (major, minor) = text.split_once('.').ok_or_else(syntax)?;
        Ok(SchemaVersion {
            major: parse_field(major).ok_or_else(syntax)?,
            minor: parse_field(minor).ok_or_else(syntax)?,
        })
    }

    /// What a reader implementing `self` does with a file written at `file`.
    #[must_use]
    pub fn compatibility(self, file: SchemaVersion) -> Compatibility {
        match (file.major.cmp(&self.major), file.minor.cmp(&self.minor)) {
            (std::cmp::Ordering::Less, _) => Compatibility::OlderMajor,
            (std::cmp::Ordering::Greater, _) => Compatibility::NewerMajor,
            (std::cmp::Ordering::Equal, std::cmp::Ordering::Less) => Compatibility::OlderMinor,
            (std::cmp::Ordering::Equal, std::cmp::Ordering::Equal) => Compatibility::Same,
            (std::cmp::Ordering::Equal, std::cmp::Ordering::Greater) => Compatibility::NewerMinor,
        }
    }
}

/// A numeric field: ASCII digits, no sign, no leading `+`, no empty string.
fn parse_field(text: &str) -> Option<u32> {
    (!text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()))
        .then(|| text.parse().ok())
        .flatten()
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

impl From<SchemaVersion> for String {
    fn from(v: SchemaVersion) -> String {
        v.to_string()
    }
}

impl TryFrom<String> for SchemaVersion {
    type Error = VersionSyntax;

    fn try_from(text: String) -> Result<SchemaVersion, VersionSyntax> {
        SchemaVersion::parse(&text)
    }
}

/// `MAJOR.MINOR.PATCH` of the core API ([`crate::API_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ApiVersion {
    /// Breaking line (while 0, MINOR is the breaking line).
    pub major: u32,
    /// Additive step (while MAJOR is 0, the breaking line).
    pub minor: u32,
    /// Fixes (while MAJOR is 0, the additive step).
    pub patch: u32,
}

impl ApiVersion {
    /// Parses `MAJOR.MINOR.PATCH`.
    pub fn parse(text: &str) -> Result<ApiVersion, VersionSyntax> {
        let syntax = || VersionSyntax {
            text: text.to_owned(),
            expected: "MAJOR.MINOR.PATCH".to_owned(),
        };
        let mut parts = text.split('.');
        let (Some(major), Some(minor), Some(patch), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(syntax());
        };
        Ok(ApiVersion {
            major: parse_field(major).ok_or_else(syntax)?,
            minor: parse_field(minor).ok_or_else(syntax)?,
            patch: parse_field(patch).ok_or_else(syntax)?,
        })
    }

    /// The version this crate reports.
    pub fn current() -> ApiVersion {
        ApiVersion::parse(crate::API_VERSION).expect("API_VERSION is MAJOR.MINOR.PATCH")
    }
}

impl fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl From<ApiVersion> for String {
    fn from(v: ApiVersion) -> String {
        v.to_string()
    }
}

impl TryFrom<String> for ApiVersion {
    type Error = VersionSyntax;

    fn try_from(text: String) -> Result<ApiVersion, VersionSyntax> {
        ApiVersion::parse(&text)
    }
}

/// Whether a host built against API `host` may load a core reporting API `core`.
///
/// The host may use every item it was generated against, so the core must be the same
/// breaking line and at least as new: with MAJOR ≥ 1, same MAJOR and `core.MINOR >=
/// host.MINOR`; with MAJOR 0, same MAJOR and MINOR and `core.PATCH >= host.PATCH`.
#[must_use]
pub fn api_compatible(core: ApiVersion, host: ApiVersion) -> bool {
    if core.major != host.major {
        return false;
    }
    if core.major == 0 {
        core.minor == host.minor && core.patch >= host.patch
    } else {
        core.minor >= host.minor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(text: &str) -> SchemaVersion {
        SchemaVersion::parse(text).unwrap()
    }

    fn a(text: &str) -> ApiVersion {
        ApiVersion::parse(text).unwrap()
    }

    #[test]
    fn schema_versions_parse_and_compare() {
        assert_eq!(v("1.0"), SchemaVersion::new(1, 0));
        assert_eq!(v("1.10").to_string(), "1.10");
        assert!(v("1.2") < v("1.10") && v("1.10") < v("2.0"));
        for bad in ["1", "1.0.0", "a.b", "1.", ".1", "+1.0", "1.-1", ""] {
            assert!(SchemaVersion::parse(bad).is_err(), "{bad}");
        }
        let json = serde_json::to_string(&v("1.1")).unwrap();
        assert_eq!(json, "\"1.1\"");
        assert_eq!(
            serde_json::from_str::<SchemaVersion>("\"1.1\"").unwrap(),
            v("1.1")
        );
        assert!(serde_json::from_str::<SchemaVersion>("\"1\"").is_err());
    }

    #[test]
    fn a_reader_at_n_plus_one_reads_n_and_refuses_another_line() {
        let reader = v("1.1");
        assert_eq!(reader.compatibility(v("1.1")), Compatibility::Same);
        assert_eq!(reader.compatibility(v("1.0")), Compatibility::OlderMinor);
        assert_eq!(reader.compatibility(v("1.2")), Compatibility::NewerMinor);
        assert_eq!(reader.compatibility(v("0.9")), Compatibility::OlderMajor);
        assert_eq!(reader.compatibility(v("2.0")), Compatibility::NewerMajor);
        assert!(Compatibility::OlderMinor.readable() && Compatibility::OlderMinor.writable());
        assert!(Compatibility::NewerMinor.readable() && !Compatibility::NewerMinor.writable());
        assert!(!Compatibility::NewerMajor.readable() && !Compatibility::OlderMajor.readable());
        let json = serde_json::to_string(&Compatibility::NewerMinor).unwrap();
        assert_eq!(json, "\"newer_minor\"");
    }

    #[test]
    fn api_versions_follow_the_zero_major_rule() {
        assert_eq!(ApiVersion::current(), a(crate::API_VERSION));
        // While MAJOR is 0, MINOR is the breaking line and PATCH the additive step.
        assert!(api_compatible(a("0.1.1"), a("0.1.0")));
        assert!(api_compatible(a("0.1.1"), a("0.1.1")));
        assert!(!api_compatible(a("0.1.0"), a("0.1.1")));
        assert!(!api_compatible(a("0.2.0"), a("0.1.1")));
        assert!(!api_compatible(a("1.0.0"), a("0.1.1")));
        // From 1.0.0, MAJOR is the breaking line and MINOR the additive step.
        assert!(api_compatible(a("1.2.0"), a("1.1.5")));
        assert!(api_compatible(a("1.1.0"), a("1.1.5")));
        assert!(!api_compatible(a("1.0.9"), a("1.1.0")));
        assert!(!api_compatible(a("2.0.0"), a("1.1.0")));
        for bad in ["1.0", "1.0.0.0", "1.0.x", ""] {
            assert!(ApiVersion::parse(bad).is_err(), "{bad}");
        }
        assert_eq!(serde_json::to_string(&a("0.1.1")).unwrap(), "\"0.1.1\"");
    }
}
