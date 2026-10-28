//! Key wrapping, payload sealing and operation signing: a thin layer over the pinned
//! libraries, composing only constructions those libraries document.
//!
//! | Purpose | Construction | Library and documentation |
//! |---|---|---|
//! | Wrap the vault key to a device or to the recovery identity | X25519 ephemeral–static Diffie–Hellman, HKDF-SHA256 (salt = ephemeral public ‖ recipient public, info = domain string) to a 256-bit key, XChaCha20-Poly1305 with a random 192-bit nonce and associated data naming vault, epoch and recipient. This is the X25519 recipient construction of the age file format (age-encryption.org/v1), with the single-use key additionally given a random nonce. | `x25519_dalek::EphemeralSecret::diffie_hellman` and `SharedSecret::was_contributory` (docs.rs/x25519-dalek); `hkdf::Hkdf::<Sha256>::new` / `expand` (RFC 5869, docs.rs/hkdf); `chacha20poly1305::XChaCha20Poly1305` with `aead::Payload` (draft-irtf-cfrg-xchacha, docs.rs/chacha20poly1305) |
//! | Seal an operation payload | XChaCha20-Poly1305 under the epoch's vault key, random nonce, associated data = vault, device, sequence | `chacha20poly1305::XChaCha20Poly1305` |
//! | Protect the recovery secret with a passphrase | Argon2id (RFC 9106) to a 256-bit key, then XChaCha20-Poly1305 | `argon2::Argon2::hash_password_into` (docs.rs/argon2) |
//! | Sign an operation or an approval | Ed25519 over the canonical signing input, verified with `verify_strict` | `ed25519_dalek::SigningKey::sign`, `VerifyingKey::verify_strict` (RFC 8032, docs.rs/ed25519-dalek) |
//! | Hash a payload or an operation header | SHA-256 | `sha2::Sha256` (docs.rs/sha2) |
//!
//! Nothing here implements a cipher, hash, key derivation or signature; the tests check the
//! libraries against their own published vectors (RFC 7748, RFC 8032, RFC 5869,
//! draft-irtf-cfrg-xchacha appendix A, the Argon2 reference test suite) and round-trip this
//! layer over them.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead as _, KeyInit as _, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use ed25519_dalek::{Signature, Signer as _, VerifyingKey};
use hkdf::Hkdf;
use rand_core::CryptoRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use x25519_dalek::{EphemeralSecret, PublicKey as ExchangePublic, StaticSecret};
use zeroize::Zeroizing;

use crate::canonical::{Canonical, HexError, from_hex, from_hex_array, to_hex};
use crate::keys::{
    DeviceId, DevicePublic, DeviceSecrets, KeyEpoch, KeyError, RecoverySecret, VaultId, VaultKey,
    random_array,
};

const WRAP_INFO: &[u8] = b"openlogos-sync/v1/x25519-wrap";
const WRAP_AAD_DOMAIN: &str = "openlogos-sync-wrap/1";
const PAYLOAD_AAD_DOMAIN: &str = "openlogos-sync-payload/1";
const PASSPHRASE_AAD_DOMAIN: &str = "openlogos-sync-passphrase/1";

/// The AEAD under a 256-bit key.
fn aead(key: &[u8; 32]) -> XChaCha20Poly1305 {
    XChaCha20Poly1305::new(&Key::from(*key))
}

/// Who a wrapped vault key is for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Recipient {
    /// An approved device, by id; the X25519 key is the one in its roster record.
    Device {
        /// The device.
        device_id: DeviceId,
    },
    /// The user-held recovery identity, by its public key (hex).
    Recovery {
        /// X25519 public key of the recovery secret.
        recovery_public: String,
    },
}

/// The vault key of one epoch, wrapped to one recipient. Carries no secret: without the
/// recipient's private X25519 key the ciphertext is opaque.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WrappedVaultKey {
    /// The epoch of the wrapped key.
    pub key_epoch: KeyEpoch,
    /// Who can unwrap it.
    pub recipient: Recipient,
    /// The sender's single-use X25519 public key, hex.
    pub ephemeral_public: String,
    /// 24-byte XChaCha20-Poly1305 nonce, hex.
    pub nonce: String,
    /// 32-byte key plus 16-byte tag, hex.
    pub ciphertext: String,
}

fn wrap_aad(vault_id: &VaultId, epoch: KeyEpoch, recipient: &Recipient, pk: &[u8; 32]) -> Vec<u8> {
    let label = match recipient {
        Recipient::Device { device_id } => format!("device:{device_id}"),
        Recipient::Recovery { .. } => "recovery".to_owned(),
    };
    Canonical::new(WRAP_AAD_DOMAIN)
        .text(vault_id.as_str())
        .u32(epoch)
        .text(&label)
        .bytes(pk)
        .finish()
}

/// HKDF-SHA256 of an X25519 shared secret, as the age X25519 recipient does it.
fn wrap_key(shared: &[u8; 32], ephemeral: &[u8; 32], recipient: &[u8; 32]) -> Zeroizing<[u8; 32]> {
    let mut salt = Vec::with_capacity(64);
    salt.extend_from_slice(ephemeral);
    salt.extend_from_slice(recipient);
    let mut key = Zeroizing::new([0u8; 32]);
    Hkdf::<Sha256>::new(Some(&salt), shared)
        .expand(WRAP_INFO, key.as_mut())
        .expect("32 bytes is within HKDF-SHA256's output limit");
    key
}

fn wrap_to<R: CryptoRng + ?Sized>(
    rng: &mut R,
    vault_id: &VaultId,
    epoch: KeyEpoch,
    recipient: Recipient,
    recipient_pk: &ExchangePublic,
    key: &VaultKey,
) -> Result<WrappedVaultKey, EnvelopeError> {
    let ephemeral = EphemeralSecret::random_from_rng(rng);
    let ephemeral_pk = ExchangePublic::from(&ephemeral);
    let shared = ephemeral.diffie_hellman(recipient_pk);
    if !shared.was_contributory() {
        return Err(EnvelopeError::NonContributory);
    }
    let wrapping = wrap_key(
        shared.as_bytes(),
        ephemeral_pk.as_bytes(),
        recipient_pk.as_bytes(),
    );
    let nonce: [u8; 24] = random_array(rng);
    let aad = wrap_aad(vault_id, epoch, &recipient, recipient_pk.as_bytes());
    let ciphertext = aead(&wrapping)
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: key.as_bytes(),
                aad: &aad,
            },
        )
        .map_err(|_| EnvelopeError::Seal)?;
    Ok(WrappedVaultKey {
        key_epoch: epoch,
        recipient,
        ephemeral_public: to_hex(ephemeral_pk.as_bytes()),
        nonce: to_hex(&nonce),
        ciphertext: to_hex(&ciphertext),
    })
}

fn unwrap_with(
    vault_id: &VaultId,
    wrapped: &WrappedVaultKey,
    secret: &StaticSecret,
) -> Result<VaultKey, EnvelopeError> {
    let my_pk = ExchangePublic::from(secret);
    let ephemeral_pk = ExchangePublic::from(from_hex_array::<32>(&wrapped.ephemeral_public)?);
    let shared = secret.diffie_hellman(&ephemeral_pk);
    if !shared.was_contributory() {
        return Err(EnvelopeError::NonContributory);
    }
    let wrapping = wrap_key(shared.as_bytes(), ephemeral_pk.as_bytes(), my_pk.as_bytes());
    let nonce = from_hex_array::<24>(&wrapped.nonce)?;
    let ciphertext = from_hex(&wrapped.ciphertext)?;
    let aad = wrap_aad(
        vault_id,
        wrapped.key_epoch,
        &wrapped.recipient,
        my_pk.as_bytes(),
    );
    let plaintext = aead(&wrapping)
        .decrypt(
            &XNonce::from(nonce),
            Payload {
                msg: &ciphertext,
                aad: &aad,
            },
        )
        .map_err(|_| EnvelopeError::Open)?;
    let plaintext = Zeroizing::new(plaintext);
    let bytes = <[u8; 32]>::try_from(&plaintext[..]).map_err(|_| EnvelopeError::Open)?;
    Ok(VaultKey::from_bytes(bytes))
}

/// Wraps `key` to a device's X25519 key.
pub fn wrap_for_device<R: CryptoRng + ?Sized>(
    rng: &mut R,
    vault_id: &VaultId,
    epoch: KeyEpoch,
    key: &VaultKey,
    device: &DevicePublic,
) -> Result<WrappedVaultKey, EnvelopeError> {
    device.check_id()?;
    wrap_to(
        rng,
        vault_id,
        epoch,
        Recipient::Device {
            device_id: device.device_id.clone(),
        },
        &device.exchange_key()?,
        key,
    )
}

/// Unwraps a key wrapped to this device. Fails if the record is for another device, another
/// vault or epoch, or was altered.
pub fn unwrap_for_device(
    vault_id: &VaultId,
    wrapped: &WrappedVaultKey,
    device: &DeviceSecrets,
) -> Result<VaultKey, EnvelopeError> {
    match &wrapped.recipient {
        Recipient::Device { device_id } if *device_id == device.device_id() => {}
        _ => return Err(EnvelopeError::WrongRecipient),
    }
    unwrap_with(vault_id, wrapped, device.exchange_secret())
}

/// Wraps `key` to the recovery identity whose public key (hex) the roster records.
pub fn wrap_for_recovery<R: CryptoRng + ?Sized>(
    rng: &mut R,
    vault_id: &VaultId,
    epoch: KeyEpoch,
    key: &VaultKey,
    recovery_public: &str,
) -> Result<WrappedVaultKey, EnvelopeError> {
    let pk = ExchangePublic::from(from_hex_array::<32>(recovery_public)?);
    wrap_to(
        rng,
        vault_id,
        epoch,
        Recipient::Recovery {
            recovery_public: recovery_public.to_owned(),
        },
        &pk,
        key,
    )
}

/// Unwraps a key wrapped to the recovery identity.
pub fn unwrap_with_recovery(
    vault_id: &VaultId,
    wrapped: &WrappedVaultKey,
    recovery: &RecoverySecret,
) -> Result<VaultKey, EnvelopeError> {
    match &wrapped.recipient {
        Recipient::Recovery { recovery_public } if *recovery_public == recovery.public_hex() => {}
        _ => return Err(EnvelopeError::WrongRecipient),
    }
    unwrap_with(vault_id, wrapped, recovery.exchange_secret())
}

/// What a sealed payload is bound to: the operation slot that carries it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadContext<'a> {
    /// The vault.
    pub vault_id: &'a VaultId,
    /// The writing device.
    pub device_id: &'a DeviceId,
    /// The device's sequence number of the operation.
    pub device_sequence: u64,
}

impl PayloadContext<'_> {
    fn aad(&self) -> Vec<u8> {
        Canonical::new(PAYLOAD_AAD_DOMAIN)
            .text(self.vault_id.as_str())
            .text(self.device_id.as_str())
            .u64(self.device_sequence)
            .finish()
    }
}

/// An encrypted payload: what a hub or a peer stores beside the operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SealedPayload {
    /// The vault key epoch that seals it.
    pub key_epoch: KeyEpoch,
    /// 24-byte nonce, hex.
    pub nonce: String,
    /// Ciphertext plus 16-byte tag, hex.
    pub ciphertext: String,
}

impl SealedPayload {
    /// The hash an operation commits to: SHA-256 over nonce then ciphertext, hex. A hash of
    /// ciphertext, never of plaintext, so equal notes do not produce equal hashes.
    pub fn hash(&self) -> Result<String, EnvelopeError> {
        let mut hasher = Sha256::new();
        hasher.update(from_hex(&self.nonce)?);
        hasher.update(from_hex(&self.ciphertext)?);
        Ok(to_hex(&hasher.finalize()))
    }
}

/// Seals `plaintext` under the vault key of `epoch` for the operation slot `context`.
pub fn seal_payload<R: CryptoRng + ?Sized>(
    rng: &mut R,
    key: &VaultKey,
    epoch: KeyEpoch,
    context: &PayloadContext<'_>,
    plaintext: &[u8],
) -> Result<SealedPayload, EnvelopeError> {
    let nonce: [u8; 24] = random_array(rng);
    seal_payload_with_nonce(key, epoch, context, plaintext, &nonce)
}

fn seal_payload_with_nonce(
    key: &VaultKey,
    epoch: KeyEpoch,
    context: &PayloadContext<'_>,
    plaintext: &[u8],
    nonce: &[u8; 24],
) -> Result<SealedPayload, EnvelopeError> {
    let aad = context.aad();
    let ciphertext = aead(key.as_bytes())
        .encrypt(
            &XNonce::from(*nonce),
            Payload {
                msg: plaintext,
                aad: &aad,
            },
        )
        .map_err(|_| EnvelopeError::Seal)?;
    Ok(SealedPayload {
        key_epoch: epoch,
        nonce: to_hex(nonce),
        ciphertext: to_hex(&ciphertext),
    })
}

/// Opens a sealed payload with the vault key of its epoch. Fails on the wrong key, another
/// operation slot or any alteration.
pub fn open_payload(
    key: &VaultKey,
    context: &PayloadContext<'_>,
    sealed: &SealedPayload,
) -> Result<Zeroizing<Vec<u8>>, EnvelopeError> {
    let nonce = from_hex_array::<24>(&sealed.nonce)?;
    let ciphertext = from_hex(&sealed.ciphertext)?;
    let aad = context.aad();
    aead(key.as_bytes())
        .decrypt(
            &XNonce::from(nonce),
            Payload {
                msg: &ciphertext,
                aad: &aad,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| EnvelopeError::Open)
}

/// Argon2id cost parameters, recorded beside what they protect so the same derivation can
/// be repeated years later. The defaults are the `argon2` crate's (m = 19 MiB, t = 2, p = 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Argon2Params {
    /// Memory in KiB.
    pub m_cost: u32,
    /// Passes.
    pub t_cost: u32,
    /// Lanes.
    pub p_cost: u32,
}

impl Default for Argon2Params {
    fn default() -> Argon2Params {
        Argon2Params {
            m_cost: Params::DEFAULT_M_COST,
            t_cost: Params::DEFAULT_T_COST,
            p_cost: Params::DEFAULT_P_COST,
        }
    }
}

/// Derives a 256-bit key from a passphrase with Argon2id version 0x13.
pub fn passphrase_key(
    passphrase: &[u8],
    salt: &[u8],
    params: Argon2Params,
) -> Result<Zeroizing<[u8; 32]>, EnvelopeError> {
    let params = Params::new(params.m_cost, params.t_cost, params.p_cost, Some(32))
        .map_err(|e| EnvelopeError::Argon2(e.to_string()))?;
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(passphrase, salt, key.as_mut())
        .map_err(|e| EnvelopeError::Argon2(e.to_string()))?;
    Ok(key)
}

/// A 32-byte secret sealed under a passphrase-derived key: the shape of the recovery
/// secret inside an offline recovery package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PassphraseSealed {
    /// The key derivation.
    pub argon2id: Argon2Params,
    /// 16-byte Argon2 salt, hex.
    pub salt: String,
    /// 24-byte nonce, hex.
    pub nonce: String,
    /// 32-byte secret plus tag, hex.
    pub ciphertext: String,
}

/// Seals `secret` under `passphrase` for `vault_id`.
pub fn seal_with_passphrase<R: CryptoRng + ?Sized>(
    rng: &mut R,
    vault_id: &VaultId,
    secret: &[u8; 32],
    passphrase: &[u8],
    params: Argon2Params,
) -> Result<PassphraseSealed, EnvelopeError> {
    let salt: [u8; 16] = random_array(rng);
    let nonce: [u8; 24] = random_array(rng);
    let key = passphrase_key(passphrase, &salt, params)?;
    let aad = Canonical::new(PASSPHRASE_AAD_DOMAIN)
        .text(vault_id.as_str())
        .finish();
    let ciphertext = aead(&key)
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: secret,
                aad: &aad,
            },
        )
        .map_err(|_| EnvelopeError::Seal)?;
    Ok(PassphraseSealed {
        argon2id: params,
        salt: to_hex(&salt),
        nonce: to_hex(&nonce),
        ciphertext: to_hex(&ciphertext),
    })
}

/// Opens [`seal_with_passphrase`]. A wrong passphrase fails authentication; nothing
/// distinguishes it from a damaged record.
pub fn open_with_passphrase(
    vault_id: &VaultId,
    sealed: &PassphraseSealed,
    passphrase: &[u8],
) -> Result<Zeroizing<[u8; 32]>, EnvelopeError> {
    let salt = from_hex_array::<16>(&sealed.salt)?;
    let nonce = from_hex_array::<24>(&sealed.nonce)?;
    let ciphertext = from_hex(&sealed.ciphertext)?;
    let key = passphrase_key(passphrase, &salt, sealed.argon2id)?;
    let aad = Canonical::new(PASSPHRASE_AAD_DOMAIN)
        .text(vault_id.as_str())
        .finish();
    let plaintext = aead(&key)
        .decrypt(
            &XNonce::from(nonce),
            Payload {
                msg: &ciphertext,
                aad: &aad,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| EnvelopeError::Open)?;
    let mut out = Zeroizing::new([0u8; 32]);
    if plaintext.len() != 32 {
        return Err(EnvelopeError::Open);
    }
    out.copy_from_slice(&plaintext);
    Ok(out)
}

/// Signs `input` with the device's Ed25519 key; 64 bytes as hex.
#[must_use]
pub fn sign(device: &DeviceSecrets, input: &[u8]) -> String {
    to_hex(&device.signing_key().sign(input).to_bytes())
}

/// Verifies an Ed25519 signature (hex) over `input` under `verifying`, with the strict
/// rules of `ed25519_dalek::VerifyingKey::verify_strict` (no small-order or non-canonical
/// points).
pub fn verify(
    verifying: &VerifyingKey,
    input: &[u8],
    signature_hex: &str,
) -> Result<(), EnvelopeError> {
    let bytes = from_hex_array::<64>(signature_hex)?;
    verifying
        .verify_strict(input, &Signature::from_bytes(&bytes))
        .map_err(|_| EnvelopeError::BadSignature)
}

/// SHA-256 of `bytes`, hex.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    to_hex(&Sha256::digest(bytes))
}

/// Why an envelope operation failed. Authentication failures carry no detail by design.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EnvelopeError {
    /// A hex field does not decode.
    #[error("field is not valid hex: {0}")]
    Hex(#[from] HexError),
    /// A key field does not parse.
    #[error("{0}")]
    Key(#[from] KeyError),
    /// The Diffie–Hellman output was all zeros (a low-order public key); refused as the
    /// x25519-dalek documentation advises.
    #[error("X25519 exchange was not contributory")]
    NonContributory,
    /// The record is wrapped to someone else.
    #[error("wrapped key is for another recipient")]
    WrongRecipient,
    /// Encryption failed (never expected with valid inputs).
    #[error("could not seal")]
    Seal,
    /// Decryption or authentication failed: wrong key, wrong context or altered bytes.
    #[error("could not open: wrong key, wrong context or altered record")]
    Open,
    /// Argon2 refused the parameters.
    #[error("argon2: {0}")]
    Argon2(String),
    /// Signature does not verify.
    #[error("signature does not verify")]
    BadSignature,
}

#[cfg(test)]
mod tests {
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng as _;

    use super::*;

    fn rng(seed: u8) -> ChaCha20Rng {
        ChaCha20Rng::from_seed([seed; 32])
    }

    fn vault() -> VaultId {
        VaultId::parse("vault-00112233445566778899aabbccddeeff").unwrap()
    }

    /// RFC 7748 section 6.1, as `x25519-dalek`'s own `tests/x25519_tests.rs` carries it.
    #[test]
    fn x25519_matches_rfc7748_section_6_1() {
        let alice = from_hex_array::<32>(
            "77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a",
        )
        .unwrap();
        let bob_public = from_hex_array::<32>(
            "de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f",
        )
        .unwrap();
        let alice_public = ExchangePublic::from(&StaticSecret::from(alice));
        assert_eq!(
            to_hex(alice_public.as_bytes()),
            "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a"
        );
        let shared = StaticSecret::from(alice).diffie_hellman(&ExchangePublic::from(bob_public));
        assert_eq!(
            to_hex(shared.as_bytes()),
            "4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742"
        );
    }

    /// RFC 8032 section 7.1 test 1 (empty message) through this layer's `sign`/`verify`.
    #[test]
    fn ed25519_matches_rfc8032_test_1() {
        let seed = from_hex_array::<32>(
            "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
        )
        .unwrap();
        let mut bytes = [0u8; 64];
        bytes[..32].copy_from_slice(&seed);
        let device = DeviceSecrets::from_bytes(&bytes);
        assert_eq!(
            device.public().signing_public,
            "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
        );
        let signature = sign(&device, b"");
        assert_eq!(
            signature,
            "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b"
        );
        let verifying = device.public().verifying_key().unwrap();
        verify(&verifying, b"", &signature).unwrap();
        assert_eq!(
            verify(&verifying, b"x", &signature),
            Err(EnvelopeError::BadSignature)
        );
    }

    /// draft-irtf-cfrg-xchacha appendix A.3.1, as `chacha20poly1305`'s `tests/lib.rs` carries it,
    /// through the library call this layer makes.
    #[test]
    fn xchacha20poly1305_matches_the_draft_vector() {
        let key = from_hex_array::<32>(
            "808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f",
        )
        .unwrap();
        let nonce =
            from_hex_array::<24>("404142434445464748494a4b4c4d4e4f5051525354555657").unwrap();
        let aad = from_hex("50515253c0c1c2c3c4c5c6c7").unwrap();
        let plaintext = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
        let ciphertext = aead(&key)
            .encrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .unwrap();
        assert_eq!(
            to_hex(&ciphertext),
            "bd6d179d3e83d43b9576579493c0e939572a1700252bfaccbed2902c21396cbb731c7f1b0b4aa6440bf3a82f4eda7e39ae64c6708c54c216cb96b72e1213b4522f8c9ba40db5d945b11b69b982c1bb9e3f3fac2bc369488f76b2383565d3fff921f9664c97637da9768812f615c68b13b52e\
             c0875924c1c7987947deafd8780acf49"
        );
    }

    /// RFC 5869 appendix A test case 1 through the HKDF call this layer makes.
    #[test]
    fn hkdf_sha256_matches_rfc5869_case_1() {
        let ikm = [0x0bu8; 22];
        let salt = from_hex("000102030405060708090a0b0c").unwrap();
        let info = from_hex("f0f1f2f3f4f5f6f7f8f9").unwrap();
        let mut okm = [0u8; 42];
        Hkdf::<Sha256>::new(Some(&salt), &ikm)
            .expand(&info, &mut okm)
            .unwrap();
        assert_eq!(
            to_hex(&okm),
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
        );
    }

    /// The Argon2 reference test suite (phc-winner-argon2 `test.c`, carried in `argon2`'s
    /// `tests/kat.rs`): Argon2id v0x13, t = 2, m = 2^8 KiB, p = 1.
    #[test]
    fn argon2id_matches_the_reference_suite() {
        let key = passphrase_key(
            b"password",
            b"somesalt",
            Argon2Params {
                m_cost: 256,
                t_cost: 2,
                p_cost: 1,
            },
        )
        .unwrap();
        assert_eq!(
            to_hex(key.as_ref()),
            "9dfeb910e80bad0311fee20f9c0e2b12c17987b4cac90c2ef54d5b3021c68bfe"
        );
    }

    #[test]
    fn vault_key_wraps_to_a_device_and_only_that_device() {
        let mut r = rng(1);
        let key = VaultKey::generate(&mut r);
        let alice = DeviceSecrets::generate(&mut r);
        let bob = DeviceSecrets::generate(&mut r);
        let wrapped = wrap_for_device(&mut r, &vault(), 1, &key, &alice.public()).unwrap();
        assert_eq!(unwrap_for_device(&vault(), &wrapped, &alice).unwrap(), key);
        assert_eq!(
            unwrap_for_device(&vault(), &wrapped, &bob),
            Err(EnvelopeError::WrongRecipient)
        );
        let other_vault = VaultId::parse("vault-ffffffffffffffffffffffffffffffff").unwrap();
        assert_eq!(
            unwrap_for_device(&other_vault, &wrapped, &alice),
            Err(EnvelopeError::Open)
        );
        let mut relabelled = wrapped.clone();
        relabelled.key_epoch = 2;
        assert_eq!(
            unwrap_for_device(&vault(), &relabelled, &alice),
            Err(EnvelopeError::Open)
        );
        let mut forged = wrapped.clone();
        forged.recipient = Recipient::Device {
            device_id: bob.device_id(),
        };
        assert_eq!(
            unwrap_for_device(&vault(), &forged, &bob),
            Err(EnvelopeError::Open)
        );
        let json = serde_json::to_value(&wrapped).unwrap();
        assert_eq!(json["recipient"]["kind"], "device");
        assert_eq!(json["ciphertext"].as_str().unwrap().len(), 96);
    }

    #[test]
    fn vault_key_wraps_to_the_recovery_identity() {
        let mut r = rng(2);
        let key = VaultKey::generate(&mut r);
        let recovery = RecoverySecret::generate(&mut r);
        let wrapped = wrap_for_recovery(&mut r, &vault(), 3, &key, &recovery.public_hex()).unwrap();
        assert_eq!(
            unwrap_with_recovery(&vault(), &wrapped, &recovery).unwrap(),
            key
        );
        let other = RecoverySecret::generate(&mut r);
        assert_eq!(
            unwrap_with_recovery(&vault(), &wrapped, &other),
            Err(EnvelopeError::WrongRecipient)
        );
        let restored = RecoverySecret::parse_code(&recovery.display_code()).unwrap();
        assert_eq!(
            unwrap_with_recovery(&vault(), &wrapped, &restored).unwrap(),
            key
        );
    }

    #[test]
    fn low_order_recipient_is_refused() {
        let mut r = rng(3);
        let key = VaultKey::generate(&mut r);
        let zero = ExchangePublic::from([0u8; 32]);
        assert_eq!(
            wrap_to(
                &mut r,
                &vault(),
                1,
                Recipient::Recovery {
                    recovery_public: to_hex(zero.as_bytes())
                },
                &zero,
                &key
            ),
            Err(EnvelopeError::NonContributory)
        );
    }

    #[test]
    fn payload_round_trips_and_is_bound_to_its_slot() {
        let mut r = rng(4);
        let key = VaultKey::generate(&mut r);
        let device = DeviceId::for_signing_key(&[1u8; 32]);
        let v = vault();
        let context = PayloadContext {
            vault_id: &v,
            device_id: &device,
            device_sequence: 7,
        };
        let sealed = seal_payload(&mut r, &key, 1, &context, b"In the beginning").unwrap();
        assert_eq!(
            &open_payload(&key, &context, &sealed).unwrap()[..],
            b"In the beginning"
        );
        let moved = PayloadContext {
            device_sequence: 8,
            ..context.clone()
        };
        assert_eq!(
            open_payload(&key, &moved, &sealed),
            Err(EnvelopeError::Open)
        );
        let other_key = VaultKey::generate(&mut r);
        assert_eq!(
            open_payload(&other_key, &context, &sealed),
            Err(EnvelopeError::Open)
        );
        let mut altered = sealed.clone();
        altered.ciphertext.replace_range(0..2, "00");
        assert!(open_payload(&key, &context, &altered).is_err());
        assert_eq!(sealed.hash().unwrap().len(), 64);
        assert_ne!(sealed.hash().unwrap(), altered.hash().unwrap());
        let again = seal_payload(&mut r, &key, 1, &context, b"In the beginning").unwrap();
        assert_ne!(
            again.hash().unwrap(),
            sealed.hash().unwrap(),
            "fresh nonce, fresh hash"
        );
    }

    #[test]
    fn passphrase_seal_round_trips_with_recorded_parameters() {
        let mut r = rng(5);
        let secret = RecoverySecret::generate(&mut r);
        let params = Argon2Params {
            m_cost: 256,
            t_cost: 2,
            p_cost: 1,
        };
        let sealed = seal_with_passphrase(
            &mut r,
            &vault(),
            &secret.to_bytes(),
            b"correct horse",
            params,
        )
        .unwrap();
        assert_eq!(sealed.argon2id, params);
        let opened = open_with_passphrase(&vault(), &sealed, b"correct horse").unwrap();
        assert_eq!(&opened[..], &secret.to_bytes()[..]);
        assert_eq!(
            open_with_passphrase(&vault(), &sealed, b"wrong"),
            Err(EnvelopeError::Open)
        );
        assert!(
            Argon2Params::default().m_cost >= 19 * 1024,
            "the default cost is the library's, not a weaker one"
        );
    }
}
