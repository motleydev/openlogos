//! Identities and key material: vault ids, device ids, device key pairs, the vault key and
//! the recovery secret.
//!
//! Secret types ([`DeviceSecrets`], [`VaultKey`], [`RecoverySecret`]) implement neither
//! `Serialize` nor `Debug` output of their bytes, so they cannot reach a portable record by
//! accident; the host keeps them in OS secure storage through [`DeviceSecrets::to_bytes`] and
//! friends. Every key is generated from the [`CryptoRng`] the host supplies.

use std::fmt;

use ed25519_dalek::{SigningKey, VerifyingKey};
use rand_core::CryptoRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use subtle::ConstantTimeEq as _;
use x25519_dalek::{PublicKey as ExchangePublic, StaticSecret};
use zeroize::{Zeroize as _, Zeroizing};

use crate::canonical::{HexError, from_hex_array, to_hex};

/// Fills a fixed-size array from the host's random source.
pub fn random_array<const N: usize, R: CryptoRng + ?Sized>(rng: &mut R) -> [u8; N] {
    let mut out = [0u8; N];
    rng.fill_bytes(&mut out);
    out
}

/// The locally generated identity of a vault: `vault-` followed by 32 lower-case hex digits
/// (128 random bits). Independent of any remote principal; linked to one by enrollment.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct VaultId(String);

impl VaultId {
    /// A fresh vault id from the host's random source.
    pub fn generate<R: CryptoRng + ?Sized>(rng: &mut R) -> VaultId {
        VaultId(format!("vault-{}", to_hex(&random_array::<16, _>(rng))))
    }

    /// Parses `vault-<32 hex>`.
    pub fn parse(text: &str) -> Result<VaultId, IdSyntax> {
        match text.strip_prefix("vault-") {
            Some(hex) if hex.len() == 32 && hex.bytes().all(is_lower_hex) => {
                Ok(VaultId(text.to_owned()))
            }
            _ => Err(IdSyntax {
                text: text.to_owned(),
                expected: "vault-<32 lower-case hex digits>",
            }),
        }
    }

    /// The id as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A device's identity: `dev-` followed by the first 16 bytes (32 hex digits) of the SHA-256
/// of its Ed25519 verifying key. Nobody can claim a device id without the matching key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DeviceId(String);

impl DeviceId {
    /// The device id of a signing public key.
    #[must_use]
    pub fn for_signing_key(signing_public: &[u8; 32]) -> DeviceId {
        let digest = Sha256::digest(signing_public);
        DeviceId(format!("dev-{}", to_hex(&digest[..16])))
    }

    /// Parses `dev-<32 hex>`.
    pub fn parse(text: &str) -> Result<DeviceId, IdSyntax> {
        match text.strip_prefix("dev-") {
            Some(hex) if hex.len() == 32 && hex.bytes().all(is_lower_hex) => {
                Ok(DeviceId(text.to_owned()))
            }
            _ => Err(IdSyntax {
                text: text.to_owned(),
                expected: "dev-<32 lower-case hex digits>",
            }),
        }
    }

    /// The id as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A globally unique operation id: `op-` followed by the 64 hex digits of the SHA-256 of the
/// operation's signing input, so an id is a commitment to the whole operation header.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct OpId(String);

impl OpId {
    /// The id of a signing input.
    #[must_use]
    pub fn for_signing_input(input: &[u8]) -> OpId {
        OpId(format!("op-{}", to_hex(&Sha256::digest(input))))
    }

    /// Parses `op-<64 hex>`.
    pub fn parse(text: &str) -> Result<OpId, IdSyntax> {
        match text.strip_prefix("op-") {
            Some(hex) if hex.len() == 64 && hex.bytes().all(is_lower_hex) => {
                Ok(OpId(text.to_owned()))
            }
            _ => Err(IdSyntax {
                text: text.to_owned(),
                expected: "op-<64 lower-case hex digits>",
            }),
        }
    }

    /// The id as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_lower_hex(b: u8) -> bool {
    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
}

/// An id that is not of the required shape.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{text}` is not of the form {expected}")]
pub struct IdSyntax {
    /// The text as given.
    pub text: String,
    /// The shape required.
    pub expected: &'static str,
}

macro_rules! string_newtype {
    ($t:ident) => {
        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
        impl TryFrom<String> for $t {
            type Error = IdSyntax;
            fn try_from(text: String) -> Result<$t, IdSyntax> {
                $t::parse(&text)
            }
        }
        impl From<$t> for String {
            fn from(id: $t) -> String {
                id.0
            }
        }
    };
}
string_newtype!(VaultId);
string_newtype!(DeviceId);
string_newtype!(OpId);

/// The key epoch of a vault: 1 at founding, +1 at every rotation (revocation).
pub type KeyEpoch = u32;

/// The public half of a device: what the roster records and other devices see.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevicePublic {
    /// Derived from `signing_public`.
    pub device_id: DeviceId,
    /// Ed25519 verifying key, hex.
    pub signing_public: String,
    /// X25519 public key, hex. A different key from the signing key, always.
    pub exchange_public: String,
}

impl DevicePublic {
    /// The verifying key, if the record is well formed.
    pub fn verifying_key(&self) -> Result<VerifyingKey, KeyError> {
        let bytes = from_hex_array::<32>(&self.signing_public)?;
        VerifyingKey::from_bytes(&bytes).map_err(|_| KeyError::InvalidVerifyingKey)
    }

    /// The X25519 public key.
    pub fn exchange_key(&self) -> Result<ExchangePublic, KeyError> {
        Ok(ExchangePublic::from(from_hex_array::<32>(
            &self.exchange_public,
        )?))
    }

    /// Whether `device_id` is the id of `signing_public`.
    pub fn check_id(&self) -> Result<(), KeyError> {
        let bytes = from_hex_array::<32>(&self.signing_public)?;
        if DeviceId::for_signing_key(&bytes) == self.device_id {
            Ok(())
        } else {
            Err(KeyError::DeviceIdMismatch)
        }
    }
}

/// A device's two private keys: Ed25519 for signatures, X25519 for receiving wrapped keys.
///
/// Generated with [`DeviceSecrets::generate`]; persisted only through [`DeviceSecrets::to_bytes`]
/// into the host's secure storage. Not serializable, not printable.
pub struct DeviceSecrets {
    signing: SigningKey,
    exchange: StaticSecret,
}

impl fmt::Debug for DeviceSecrets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DeviceSecrets({})", self.device_id())
    }
}

impl DeviceSecrets {
    /// Two independent keys from the host's random source: 32 random bytes as the Ed25519
    /// seed (`ed25519_dalek::SigningKey::from_bytes`, which is what the crate's own
    /// `generate` does) and `x25519_dalek::StaticSecret::random_from_rng`.
    pub fn generate<R: CryptoRng + ?Sized>(rng: &mut R) -> DeviceSecrets {
        let mut seed: [u8; 32] = random_array(rng);
        let signing = SigningKey::from_bytes(&seed);
        seed.zeroize();
        DeviceSecrets {
            signing,
            exchange: StaticSecret::random_from_rng(rng),
        }
    }

    /// The 64 bytes the host stores: signing seed then exchange scalar.
    #[must_use]
    pub fn to_bytes(&self) -> Zeroizing<[u8; 64]> {
        let mut out = Zeroizing::new([0u8; 64]);
        out[..32].copy_from_slice(&self.signing.to_bytes());
        out[32..].copy_from_slice(&self.exchange.to_bytes());
        out
    }

    /// Restores from [`DeviceSecrets::to_bytes`].
    #[must_use]
    pub fn from_bytes(bytes: &[u8; 64]) -> DeviceSecrets {
        let mut signing = [0u8; 32];
        let mut exchange = [0u8; 32];
        signing.copy_from_slice(&bytes[..32]);
        exchange.copy_from_slice(&bytes[32..]);
        let secrets = DeviceSecrets {
            signing: SigningKey::from_bytes(&signing),
            exchange: StaticSecret::from(exchange),
        };
        signing.zeroize();
        exchange.zeroize();
        secrets
    }

    /// The device id.
    #[must_use]
    pub fn device_id(&self) -> DeviceId {
        DeviceId::for_signing_key(self.signing.verifying_key().as_bytes())
    }

    /// The public record.
    #[must_use]
    pub fn public(&self) -> DevicePublic {
        DevicePublic {
            device_id: self.device_id(),
            signing_public: to_hex(self.signing.verifying_key().as_bytes()),
            exchange_public: to_hex(ExchangePublic::from(&self.exchange).as_bytes()),
        }
    }

    pub(crate) fn signing_key(&self) -> &SigningKey {
        &self.signing
    }

    pub(crate) fn exchange_secret(&self) -> &StaticSecret {
        &self.exchange
    }
}

/// The 256-bit symmetric key of one key epoch of a vault (the XChaCha20-Poly1305 key that
/// seals payloads). Lives wrapped in the roster; in the clear only in memory.
pub struct VaultKey(Zeroizing<[u8; 32]>);

impl fmt::Debug for VaultKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VaultKey(..)")
    }
}

impl PartialEq for VaultKey {
    fn eq(&self, other: &VaultKey) -> bool {
        bool::from(self.0.ct_eq(&*other.0))
    }
}

impl VaultKey {
    /// A fresh key from the host's random source.
    pub fn generate<R: CryptoRng + ?Sized>(rng: &mut R) -> VaultKey {
        VaultKey(Zeroizing::new(random_array(rng)))
    }

    /// Wraps existing key bytes (after unwrapping).
    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> VaultKey {
        VaultKey(Zeroizing::new(bytes))
    }

    /// The key bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// The user-held recovery secret: an X25519 identity whose private scalar the user keeps
/// (as a code or a passphrase-protected export) and whose public key sits in the roster, so
/// any device can rewrap a rotated vault key to it without holding it.
pub struct RecoverySecret(StaticSecret);

impl fmt::Debug for RecoverySecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecoverySecret(..)")
    }
}

impl RecoverySecret {
    /// A fresh secret from the host's random source.
    pub fn generate<R: CryptoRng + ?Sized>(rng: &mut R) -> RecoverySecret {
        RecoverySecret(StaticSecret::random_from_rng(rng))
    }

    /// Restores from the 32 secret bytes.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> RecoverySecret {
        RecoverySecret(StaticSecret::from(bytes))
    }

    /// The 32 secret bytes, for the host to display or store.
    #[must_use]
    pub fn to_bytes(&self) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(self.0.to_bytes())
    }

    /// The code a user writes down: 16 groups of 4 hex digits separated by `-`.
    #[must_use]
    pub fn display_code(&self) -> Zeroizing<String> {
        let hex = to_hex(&self.0.to_bytes());
        let groups: Vec<&str> = (0..16).map(|i| &hex[i * 4..i * 4 + 4]).collect();
        Zeroizing::new(groups.join("-"))
    }

    /// Parses [`RecoverySecret::display_code`], ignoring case, spaces and dashes.
    pub fn parse_code(code: &str) -> Result<RecoverySecret, KeyError> {
        let cleaned: String = code
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '-')
            .collect();
        let bytes = from_hex_array::<32>(&cleaned)?;
        Ok(RecoverySecret::from_bytes(bytes))
    }

    /// The public key the roster records, hex.
    #[must_use]
    pub fn public_hex(&self) -> String {
        to_hex(ExchangePublic::from(&self.0).as_bytes())
    }

    pub(crate) fn exchange_secret(&self) -> &StaticSecret {
        &self.0
    }
}

/// Key material that does not parse.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeyError {
    /// A hex field is malformed or of the wrong length.
    #[error("key field is not valid hex: {0}")]
    Hex(#[from] HexError),
    /// 32 bytes that are not an Ed25519 public key.
    #[error("not a valid Ed25519 verifying key")]
    InvalidVerifyingKey,
    /// The device id is not derived from the signing key.
    #[error("device id is not the hash of the signing key")]
    DeviceIdMismatch,
}

#[cfg(test)]
mod tests {
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng as _;

    use super::*;

    fn rng() -> ChaCha20Rng {
        ChaCha20Rng::from_seed([7u8; 32])
    }

    #[test]
    fn ids_have_fixed_shapes() {
        let vault = VaultId::generate(&mut rng());
        assert_eq!(VaultId::parse(vault.as_str()).unwrap(), vault);
        assert!(VaultId::parse("vault-fixture").is_err());
        assert!(DeviceId::parse("dev-XYZ").is_err());
        assert!(OpId::parse("op-00").is_err());
        let json = serde_json::to_string(&vault).unwrap();
        assert_eq!(json, format!("\"{}\"", vault.as_str()));
        assert!(serde_json::from_str::<VaultId>("\"vault-nope\"").is_err());
    }

    #[test]
    fn device_id_is_bound_to_the_signing_key() {
        let secrets = DeviceSecrets::generate(&mut rng());
        let public = secrets.public();
        public.check_id().unwrap();
        assert_ne!(public.signing_public, public.exchange_public);
        let mut forged = public.clone();
        forged.device_id = DeviceId::parse("dev-00000000000000000000000000000000").unwrap();
        assert_eq!(forged.check_id(), Err(KeyError::DeviceIdMismatch));
    }

    #[test]
    fn device_secrets_round_trip_through_secure_storage_bytes() {
        let secrets = DeviceSecrets::generate(&mut rng());
        let restored = DeviceSecrets::from_bytes(&secrets.to_bytes());
        assert_eq!(restored.public(), secrets.public());
    }

    #[test]
    fn recovery_code_round_trips_and_tolerates_formatting() {
        let secret = RecoverySecret::generate(&mut rng());
        let code = secret.display_code();
        assert_eq!(code.len(), 16 * 4 + 15);
        let parsed = RecoverySecret::parse_code(&code.to_uppercase().replace('-', " ")).unwrap();
        assert_eq!(parsed.public_hex(), secret.public_hex());
        assert!(RecoverySecret::parse_code("abcd").is_err());
    }

    #[test]
    fn vault_key_equality_and_debug_hide_bytes() {
        let mut r = rng();
        let a = VaultKey::generate(&mut r);
        let b = VaultKey::from_bytes(*a.as_bytes());
        assert_eq!(a, b);
        assert_ne!(a, VaultKey::generate(&mut r));
        assert_eq!(format!("{a:?}"), "VaultKey(..)");
    }
}
