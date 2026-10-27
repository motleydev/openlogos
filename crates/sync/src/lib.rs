//! Private sync as an encrypted operation log. Every data change is an operation; this crate
//! orders, merges and resolves conflicts between logs from several devices and defines what an
//! optional self-hosted hub stores without being able to read it
//! (specs/05-sharing-security.md). Direct device-to-device exchange and file transfer are
//! first-class alternatives to any server, and client recovery material remains user-held.

/// The crate's package name, as Cargo records it.
///
/// Placeholder API so the skeleton has one documented item and one test per crate; the
/// real surface arrives with the tasks named in the module documentation.
#[must_use]
pub fn crate_name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[cfg(test)]
mod tests {
    use super::crate_name;

    #[test]
    fn crate_is_part_of_the_openlogos_workspace() {
        assert!(crate_name().starts_with("openlogos-"));
    }
}
