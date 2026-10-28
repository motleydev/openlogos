//! The offline recovery package (schemas/recovery-export.schema.json): what a user keeps
//! outside every device and every server so that losing all devices is survivable.
//!
//! It holds the recovery secret sealed under a passphrase (Argon2id, then
//! XChaCha20-Poly1305), the vault key of the export's epoch wrapped to the recovery identity,
//! and the public roster as of the export. With the package and the passphrase, a fresh
//! device recovers the vault key and approves itself ([`crate::enrollment::Roster::approve_with_recovery`]);
//! with the recovery code alone it needs the current roster from a device or a hub. Without
//! both the package (or code) and the passphrase, nothing can be recovered: there is no
//! server-side recovery, by design.

use rand_core::CryptoRng;
use serde::{Deserialize, Serialize};

use crate::RECOVERY_EXPORT_SCHEMA;
use crate::enrollment::{EnrollError, Roster};
use crate::envelope::{
    Argon2Params, PassphraseSealed, WrappedVaultKey, open_with_passphrase, seal_with_passphrase,
    unwrap_with_recovery,
};
use crate::keys::{KeyEpoch, RecoverySecret, VaultId, VaultKey};

/// The package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryExport {
    /// `1.0`.
    pub schema_version: String,
    /// The vault.
    pub vault_id: VaultId,
    /// RFC 3339 time of the export (host clock).
    pub created_at: String,
    /// The key epoch at export time.
    pub key_epoch: KeyEpoch,
    /// X25519 public key of the recovery identity, hex (matches the roster's).
    pub recovery_public: String,
    /// The recovery secret, sealed under the passphrase.
    pub protected_secret: PassphraseSealed,
    /// The vault key of `key_epoch`, wrapped to the recovery identity.
    pub wrapped_vault_key: WrappedVaultKey,
    /// The roster as of the export: public keys and provenance, nothing secret.
    pub roster: Roster,
}

impl RecoveryExport {
    /// Builds the package for `roster`'s vault from the recovery secret and a passphrase.
    pub fn create<R: CryptoRng + ?Sized>(
        rng: &mut R,
        roster: &Roster,
        recovery: &RecoverySecret,
        passphrase: &[u8],
        params: Argon2Params,
        now: &str,
    ) -> Result<RecoveryExport, EnrollError> {
        if recovery.public_hex() != roster.recovery.recovery_public {
            return Err(EnrollError::WrongRecoverySecret);
        }
        let wrapped = roster
            .recovery
            .wrapped_keys
            .iter()
            .find(|w| w.key_epoch == roster.key_epoch)
            .ok_or(EnrollError::NoKeyForEpoch(roster.key_epoch))?
            .clone();
        let protected_secret = seal_with_passphrase(
            rng,
            &roster.vault_id,
            &recovery.to_bytes(),
            passphrase,
            params,
        )?;
        Ok(RecoveryExport {
            schema_version: RECOVERY_EXPORT_SCHEMA.to_owned(),
            vault_id: roster.vault_id.clone(),
            created_at: now.to_owned(),
            key_epoch: roster.key_epoch,
            recovery_public: recovery.public_hex(),
            protected_secret,
            wrapped_vault_key: wrapped,
            roster: roster.clone(),
        })
    }

    /// Opens the package: the recovery secret and the vault key of the export's epoch.
    /// A wrong passphrase fails without saying so specifically.
    pub fn open(&self, passphrase: &[u8]) -> Result<(RecoverySecret, VaultKey), EnrollError> {
        let bytes = open_with_passphrase(&self.vault_id, &self.protected_secret, passphrase)?;
        let recovery = RecoverySecret::from_bytes(*bytes);
        if recovery.public_hex() != self.recovery_public {
            return Err(EnrollError::WrongRecoverySecret);
        }
        let key = unwrap_with_recovery(&self.vault_id, &self.wrapped_vault_key, &recovery)?;
        Ok((recovery, key))
    }

    /// Shape checks a reader does before trusting the package: the roster is for this vault
    /// and verifies, and the epochs and recovery keys agree.
    pub fn check(&self) -> Result<(), EnrollError> {
        if self.roster.vault_id != self.vault_id {
            return Err(EnrollError::WrongVault);
        }
        if self.roster.recovery.recovery_public != self.recovery_public {
            return Err(EnrollError::Invalid(
                "recovery public key differs from the roster's".to_owned(),
            ));
        }
        if self.wrapped_vault_key.key_epoch != self.key_epoch
            || self.roster.key_epoch != self.key_epoch
        {
            return Err(EnrollError::Invalid("epochs disagree".to_owned()));
        }
        self.roster.verify()
    }
}

#[cfg(test)]
mod tests {
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng as _;

    use super::*;
    use crate::enrollment::EnrollmentRequest;
    use crate::envelope::{PayloadContext, open_payload, seal_payload};
    use crate::keys::DeviceSecrets;

    const FAST: Argon2Params = Argon2Params {
        m_cost: 256,
        t_cost: 2,
        p_cost: 1,
    };

    #[test]
    fn package_plus_passphrase_restores_a_vault_whose_devices_are_all_gone() {
        let mut rng = ChaCha20Rng::from_seed([21u8; 32]);
        let vault = VaultId::generate(&mut rng);
        let founder = DeviceSecrets::generate(&mut rng);
        let recovery = RecoverySecret::generate(&mut rng);
        let (mut roster, key) = Roster::found(
            &mut rng,
            vault.clone(),
            &founder,
            "iPad",
            &recovery.public_hex(),
            "t",
        )
        .unwrap();
        let context = PayloadContext {
            vault_id: &vault,
            device_id: &founder.device_id(),
            device_sequence: 1,
        };
        let sealed = seal_payload(&mut rng, &key, 1, &context, b"sermon notes").unwrap();
        let export =
            RecoveryExport::create(&mut rng, &roster, &recovery, b"long passphrase", FAST, "t")
                .unwrap();
        export.check().unwrap();
        let file = serde_json::to_string(&export).unwrap();
        assert!(!file.contains(&crate::canonical::to_hex(&recovery.to_bytes()[..])));
        assert!(!file.contains(&crate::canonical::to_hex(key.as_bytes())));
        drop((founder, recovery, roster.clone()));

        let package: RecoveryExport = serde_json::from_str(&file).unwrap();
        package.check().unwrap();
        assert!(matches!(
            package.open(b"wrong"),
            Err(EnrollError::Envelope(crate::envelope::EnvelopeError::Open))
        ));
        let (recovered, recovered_key) = package.open(b"long passphrase").unwrap();
        assert_eq!(recovered_key, key);
        assert_eq!(
            &open_payload(&recovered_key, &context, &sealed).unwrap()[..],
            b"sermon notes"
        );
        // Re-enrol a fresh device from the roster inside the package.
        let fresh = DeviceSecrets::generate(&mut rng);
        roster = package.roster.clone();
        roster
            .approve_with_recovery(
                &mut rng,
                &EnrollmentRequest::new(vault.clone(), &fresh, "new", "t"),
                &recovered,
                "t",
            )
            .unwrap();
        assert_eq!(roster.vault_key_for(&fresh).unwrap(), key);
    }

    #[test]
    fn a_package_for_another_recovery_identity_is_refused() {
        let mut rng = ChaCha20Rng::from_seed([22u8; 32]);
        let vault = VaultId::generate(&mut rng);
        let founder = DeviceSecrets::generate(&mut rng);
        let recovery = RecoverySecret::generate(&mut rng);
        let (roster, _) = Roster::found(
            &mut rng,
            vault,
            &founder,
            "iPad",
            &recovery.public_hex(),
            "t",
        )
        .unwrap();
        let other = RecoverySecret::generate(&mut rng);
        assert_eq!(
            RecoveryExport::create(&mut rng, &roster, &other, b"p", FAST, "t"),
            Err(EnrollError::WrongRecoverySecret)
        );
        let mut export =
            RecoveryExport::create(&mut rng, &roster, &recovery, b"p", FAST, "t").unwrap();
        export.key_epoch = 2;
        assert!(matches!(export.check(), Err(EnrollError::Invalid(_))));
    }
}
