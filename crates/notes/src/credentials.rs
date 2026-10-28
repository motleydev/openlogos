//! No credential fields in portable state (specs/12-workspace-ux.md: "Credentials and
//! transient selected private text never enter layout files"; specs/03-data-model.md:
//! optional sync "never" covers provider credentials). The workspace, note-binding,
//! reading-snapshot and research-thread records are all plain data; this module is the
//! Rust side of the check `tools/validate.py` runs over their JSON forms, so a type that
//! grows a credential-shaped field fails a test here before it reaches a schema.
//!
//! A key is credential-shaped when one of its `_`-, `-` or `.`-separated words (camelCase
//! split too) is a credential word, or when its last word is `token` and the key is not one
//! of the token-id fields the anchor and alignment records legitimately use. A value is
//! credential-shaped when it is a PEM private key block, a bearer header, or a well-known
//! provider key prefix.

use serde_json::Value;

/// Words that make a key credential-shaped wherever they appear.
pub const CREDENTIAL_WORDS: [&str; 16] = [
    "password",
    "passwd",
    "passphrase",
    "secret",
    "secrets",
    "apikey",
    "credential",
    "credentials",
    "bearer",
    "cookie",
    "cookies",
    "oauth",
    "jwt",
    "privatekey",
    "clientsecret",
    "accesskey",
];

/// Two-word credential pairs (first word, second word).
pub const CREDENTIAL_PAIRS: [(&str, &str); 7] = [
    ("api", "key"),
    ("private", "key"),
    ("client", "secret"),
    ("access", "key"),
    ("secret", "key"),
    ("signing", "key"),
    ("session", "key"),
];

/// Keys ending in `token` that are not credentials: token ids and counts.
pub const TOKEN_ALLOW: [&str; 4] = ["token_ids", "tokens", "token_count", "token"];

fn words(key: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut previous_lower = false;
    for c in key.chars() {
        if matches!(c, '_' | '-' | '.' | ' ') {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            previous_lower = false;
            continue;
        }
        if c.is_ascii_uppercase() && previous_lower && !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
        previous_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        current.extend(c.to_lowercase());
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// Whether a field name is credential-shaped.
#[must_use]
pub fn is_credential_shaped_key(key: &str) -> bool {
    let words = words(key);
    if words.iter().any(|w| CREDENTIAL_WORDS.contains(&w.as_str())) {
        return true;
    }
    if words
        .windows(2)
        .any(|pair| CREDENTIAL_PAIRS.contains(&(pair[0].as_str(), pair[1].as_str())))
    {
        return true;
    }
    let lower = key.to_ascii_lowercase();
    if words.last().is_some_and(|w| w == "token" || w == "tokens") {
        return !TOKEN_ALLOW.contains(&lower.as_str()) && !lower.ends_with("_token_count");
    }
    false
}

/// Whether a string value looks like a secret regardless of its key.
#[must_use]
pub fn is_credential_shaped_value(value: &str) -> bool {
    let trimmed = value.trim();
    let pem = trimmed.contains("-----BEGIN") && trimmed.contains("PRIVATE KEY-----");
    let bearer = trimmed.len() > 7
        && trimmed
            .get(..7)
            .is_some_and(|p| p.eq_ignore_ascii_case("bearer "));
    let prefixed = [
        "sk-", "sk_live_", "sk_test_", "ghp_", "gho_", "xoxb-", "xoxp-", "AKIA",
    ]
    .iter()
    .any(|prefix| trimmed.starts_with(prefix) && trimmed.len() >= 20);
    pem || bearer || prefixed
}

/// Every JSON pointer in `value` whose key or string value is credential-shaped.
#[must_use]
pub fn find_credential_shaped(value: &Value) -> Vec<String> {
    let mut found = Vec::new();
    walk(value, String::new(), &mut found);
    found
}

fn walk(value: &Value, path: String, found: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let here = format!("{path}/{key}");
                if is_credential_shaped_key(key) {
                    found.push(here.clone());
                }
                walk(child, here, found);
            }
        }
        Value::Array(items) => {
            for (i, child) in items.iter().enumerate() {
                walk(child, format!("{path}/{i}"), found);
            }
        }
        Value::String(s) if is_credential_shaped_value(s) => found.push(path),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn credential_keys() {
        for bad in [
            "api_key",
            "apiKey",
            "API_KEY",
            "password",
            "vault_password",
            "provider_secret",
            "client_secret",
            "private_key",
            "privateKey",
            "access_token",
            "refresh_token",
            "id_token",
            "credentials",
            "auth.bearer",
            "oauth_state",
            "session-cookie",
            "signing_key",
        ] {
            assert!(is_credential_shaped_key(bad), "{bad}");
        }
        for ok in [
            "token_ids",
            "tokens",
            "token",
            "token_count",
            "max_token_count",
            "tokenization_hash",
            "authority",
            "author",
            "key",
            "keyboard_focus",
            "last_seen_sha256",
            "provider_account_ref",
            "model_ref",
            "vault_id",
            "vault_path",
            "secretary",
            "passage",
        ] {
            assert!(!is_credential_shaped_key(ok), "{ok}");
        }
    }

    #[test]
    fn credential_values() {
        assert!(is_credential_shaped_value(
            "-----BEGIN PRIVATE KEY-----\nMIIE\n-----END PRIVATE KEY-----"
        ));
        assert!(is_credential_shaped_value(
            "-----BEGIN RSA PRIVATE KEY-----\nMIIE\n-----END RSA PRIVATE KEY-----"
        ));
        assert!(is_credential_shaped_value("Bearer abc.def.ghi"));
        assert!(is_credential_shaped_value("sk-abcdefghijklmnopqrstuvwxyz"));
        assert!(is_credential_shaped_value("AKIAIOSFODNN7EXAMPLE1234"));
        assert!(!is_credential_shaped_value("sk-short"));
        assert!(!is_credential_shaped_value("a bearer of good news"));
        assert!(!is_credential_shaped_value("John 3:16"));
    }

    #[test]
    fn finds_nested_fields_and_values() {
        let doc = json!({
            "id": "x",
            "panes": [{"id": "p1", "provider": {"api_key": "nope"}}],
            "notes": {"body": "Bearer abcdefghijklmnop"},
            "tokens": ["t1"]
        });
        assert_eq!(
            find_credential_shaped(&doc),
            vec!["/notes/body", "/panes/0/provider/api_key"]
        );
        assert!(find_credential_shaped(&json!({"token_ids": [], "authority": "app"})).is_empty());
    }
}
