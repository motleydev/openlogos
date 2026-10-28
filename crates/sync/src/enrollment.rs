//! Device enrollment, approval, revocation and key rotation: the [`Roster`] state machine of
//! ADR 0004 (schemas/device-enrollment.schema.json).
//!
//! A vault is founded by one device, which generates the vault key (epoch 1), wraps it to
//! itself and to the user's recovery identity, and records its own enrollment as the
//! founder. Every further device first produces an [`EnrollmentRequest`] (its public keys
//! and a name) and can read nothing until an **active device** or the **recovery secret**
//! approves it: approval wraps the current vault key to the new device and records who
//! approved. Revoking a device rotates the vault key to a new epoch, rewraps it to every
//! remaining active device and to the recovery identity, and records a cutoff: the revoked
//! device's operations up to that sequence stay valid, later ones are rejected
//! ([`Authority`]). What the revoked device already decrypted it keeps; no protocol can
//! recall plaintext from a device that had it.
//!
//! The roster carries public keys, wrapped keys, signatures and provenance only. It is not
//! secret and may live on a hub, on every device and in a recovery export; it cannot be used
//! to read anything without a device's private key or the recovery secret.

use ed25519_dalek::VerifyingKey;
use rand_core::CryptoRng;
use serde::{Deserialize, Serialize};

use crate::DEVICE_ENROLLMENT_SCHEMA;
use crate::canonical::Canonical;
use crate::envelope::{
    self, EnvelopeError, WrappedVaultKey, unwrap_for_device, unwrap_with_recovery, wrap_for_device,
    wrap_for_recovery,
};
use crate::keys::{
    DeviceId, DevicePublic, DeviceSecrets, KeyEpoch, KeyError, RecoverySecret, VaultId, VaultKey,
};
use crate::log::Authority;

const APPROVAL_DOMAIN: &str = "openlogos-sync-approval/1";
const REVOCATION_DOMAIN: &str = "openlogos-sync-revocation/1";

/// A device asking to join: everything the approver needs, nothing secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnrollmentRequest {
    /// The vault the device wants to join.
    pub vault_id: VaultId,
    /// The device's public keys.
    pub device: DevicePublic,
    /// A name the user chose for the device.
    pub display_name: String,
    /// RFC 3339 time the request was made (host clock; informational).
    pub requested_at: String,
}

impl EnrollmentRequest {
    /// The request `device` makes to join `vault_id`.
    #[must_use]
    pub fn new(
        vault_id: VaultId,
        device: &DeviceSecrets,
        display_name: &str,
        now: &str,
    ) -> EnrollmentRequest {
        EnrollmentRequest {
            vault_id,
            device: device.public(),
            display_name: display_name.to_owned(),
            requested_at: now.to_owned(),
        }
    }
}

/// Who approved a device, with proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "by", rename_all = "snake_case")]
pub enum Approval {
    /// The founding device, self-signed.
    Founder {
        /// Ed25519 signature by the device itself over the approval input.
        signature: String,
    },
    /// An existing active device.
    Device {
        /// The approving device.
        device_id: DeviceId,
        /// Ed25519 signature by the approving device over the approval input.
        signature: String,
    },
    /// The user, by presenting the recovery secret. The recovery identity is an X25519 key
    /// and cannot sign; the proof is the wrapped key itself, which only the holder of the
    /// recovery secret could have produced from the recovery-wrapped vault key.
    Recovery,
}

/// A device's revocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revocation {
    /// RFC 3339 time (host clock; informational).
    pub revoked_at: String,
    /// The device that revoked.
    pub by: DeviceId,
    /// Why, in the user's words (`lost`, `replaced`, …).
    pub reason: String,
    /// The revoked device's last accepted sequence number: later operations are rejected.
    pub cutoff_sequence: u64,
    /// The epoch the vault key was rotated to.
    pub epoch_after: KeyEpoch,
    /// Ed25519 signature by `by` over the revocation input.
    pub signature: String,
}

/// One device in the roster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceRecord {
    /// Public keys and id.
    pub device: DevicePublic,
    /// User-chosen name.
    pub display_name: String,
    /// RFC 3339 time of approval (host clock; informational).
    pub enrolled_at: String,
    /// The epoch current at approval.
    pub enrolled_epoch: KeyEpoch,
    /// Who approved.
    pub approval: Approval,
    /// The vault key of every epoch from enrollment until revocation, wrapped to this device.
    pub wrapped_keys: Vec<WrappedVaultKey>,
    /// Set once revoked. A revoked device never becomes active again; it re-enrolls as a
    /// new device with new keys.
    pub revocation: Option<Revocation>,
}

impl DeviceRecord {
    /// Whether the device may currently read and write.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.revocation.is_none()
    }
}

/// The user-held recovery identity as the roster sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryRecord {
    /// X25519 public key of the recovery secret, hex.
    pub recovery_public: String,
    /// The vault key of every epoch, wrapped to the recovery identity.
    pub wrapped_keys: Vec<WrappedVaultKey>,
}

/// The remote identity a vault is linked to, if any: `(issuer, subject)`, never an email.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemotePrincipal {
    /// OIDC issuer URL.
    pub issuer: String,
    /// OIDC subject.
    pub subject: String,
    /// RFC 3339 time of the user-approved link.
    pub linked_at: String,
    /// The device on which the user approved it.
    pub linked_by: DeviceId,
}

/// The device roster of one vault: who may read and write, at which epochs, approved by whom.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Roster {
    /// `1.0`.
    pub schema_version: String,
    /// The vault.
    pub vault_id: VaultId,
    /// The current key epoch.
    pub key_epoch: KeyEpoch,
    /// The linked remote principal, if the user linked one.
    pub principal: Option<RemotePrincipal>,
    /// The recovery identity and its wrapped keys.
    pub recovery: RecoveryRecord,
    /// Every device ever approved, active or revoked.
    pub devices: Vec<DeviceRecord>,
}

fn approval_input(
    vault_id: &VaultId,
    device: &DevicePublic,
    epoch: KeyEpoch,
    approver: &str,
) -> Vec<u8> {
    Canonical::new(APPROVAL_DOMAIN)
        .text(vault_id.as_str())
        .text(device.device_id.as_str())
        .text(&device.signing_public)
        .text(&device.exchange_public)
        .u32(epoch)
        .text(approver)
        .finish()
}

fn revocation_input(vault_id: &VaultId, revoked: &DeviceId, r: &Revocation) -> Vec<u8> {
    Canonical::new(REVOCATION_DOMAIN)
        .text(vault_id.as_str())
        .text(revoked.as_str())
        .text(r.by.as_str())
        .text(&r.reason)
        .u64(r.cutoff_sequence)
        .u32(r.epoch_after)
        .finish()
}

impl Roster {
    /// Founds a vault: generates the epoch-1 key, wraps it to `founder` and to the recovery
    /// identity, records the founder as its own approver. Returns the roster and the key.
    pub fn found<R: CryptoRng + ?Sized>(
        rng: &mut R,
        vault_id: VaultId,
        founder: &DeviceSecrets,
        display_name: &str,
        recovery_public: &str,
        now: &str,
    ) -> Result<(Roster, VaultKey), EnrollError> {
        let key = VaultKey::generate(rng);
        let public = founder.public();
        let signature = envelope::sign(founder, &approval_input(&vault_id, &public, 1, "founder"));
        let roster = Roster {
            schema_version: DEVICE_ENROLLMENT_SCHEMA.to_owned(),
            vault_id: vault_id.clone(),
            key_epoch: 1,
            principal: None,
            recovery: RecoveryRecord {
                recovery_public: recovery_public.to_owned(),
                wrapped_keys: vec![wrap_for_recovery(rng, &vault_id, 1, &key, recovery_public)?],
            },
            devices: vec![DeviceRecord {
                wrapped_keys: vec![wrap_for_device(rng, &vault_id, 1, &key, &public)?],
                device: public,
                display_name: display_name.to_owned(),
                enrolled_at: now.to_owned(),
                enrolled_epoch: 1,
                approval: Approval::Founder { signature },
                revocation: None,
            }],
        };
        Ok((roster, key))
    }

    /// The record of `device_id`, approved at some time.
    #[must_use]
    pub fn record(&self, device_id: &DeviceId) -> Option<&DeviceRecord> {
        self.devices
            .iter()
            .find(|d| d.device.device_id == *device_id)
    }

    /// Whether `device_id` may read and write now.
    #[must_use]
    pub fn is_active(&self, device_id: &DeviceId) -> bool {
        self.record(device_id).is_some_and(DeviceRecord::is_active)
    }

    /// Ids of the active devices, in roster order.
    #[must_use]
    pub fn active_devices(&self) -> Vec<DeviceId> {
        self.devices
            .iter()
            .filter(|d| d.is_active())
            .map(|d| d.device.device_id.clone())
            .collect()
    }

    fn require_active(&self, device_id: &DeviceId) -> Result<(), EnrollError> {
        if self.is_active(device_id) {
            Ok(())
        } else {
            Err(EnrollError::NotActive(device_id.clone()))
        }
    }

    fn check_request(&self, request: &EnrollmentRequest) -> Result<(), EnrollError> {
        if request.vault_id != self.vault_id {
            return Err(EnrollError::WrongVault);
        }
        request.device.check_id()?;
        request.device.verifying_key()?;
        if self.record(&request.device.device_id).is_some() {
            return Err(EnrollError::AlreadyEnrolled(
                request.device.device_id.clone(),
            ));
        }
        Ok(())
    }

    fn admit<R: CryptoRng + ?Sized>(
        &mut self,
        rng: &mut R,
        request: &EnrollmentRequest,
        key: &VaultKey,
        approval: Approval,
        now: &str,
    ) -> Result<(), EnrollError> {
        let wrapped = wrap_for_device(rng, &self.vault_id, self.key_epoch, key, &request.device)?;
        self.devices.push(DeviceRecord {
            device: request.device.clone(),
            display_name: request.display_name.clone(),
            enrolled_at: now.to_owned(),
            enrolled_epoch: self.key_epoch,
            approval,
            wrapped_keys: vec![wrapped],
            revocation: None,
        });
        Ok(())
    }

    /// An active device approves `request`: the current vault key is wrapped to the new
    /// device and the approval signed.
    pub fn approve<R: CryptoRng + ?Sized>(
        &mut self,
        rng: &mut R,
        request: &EnrollmentRequest,
        approver: &DeviceSecrets,
        now: &str,
    ) -> Result<(), EnrollError> {
        self.check_request(request)?;
        self.require_active(&approver.device_id())?;
        let key = self.vault_key_for(approver)?;
        let approver_id = approver.device_id();
        let signature = envelope::sign(
            approver,
            &approval_input(
                &self.vault_id,
                &request.device,
                self.key_epoch,
                approver_id.as_str(),
            ),
        );
        self.admit(
            rng,
            request,
            &key,
            Approval::Device {
                device_id: approver_id,
                signature,
            },
            now,
        )
    }

    /// The user approves `request` with the recovery secret (no other device needed).
    pub fn approve_with_recovery<R: CryptoRng + ?Sized>(
        &mut self,
        rng: &mut R,
        request: &EnrollmentRequest,
        recovery: &RecoverySecret,
        now: &str,
    ) -> Result<(), EnrollError> {
        self.check_request(request)?;
        let key = self.recover(recovery)?;
        self.admit(rng, request, &key, Approval::Recovery, now)
    }

    /// Revokes `revoked`: rotates the vault key to a new epoch, rewraps it to every other
    /// active device and to the recovery identity, and records the cutoff. Returns the new
    /// key (the revoker holds it; the others unwrap theirs from the roster).
    pub fn revoke<R: CryptoRng + ?Sized>(
        &mut self,
        rng: &mut R,
        revoked: &DeviceId,
        revoker: &DeviceSecrets,
        cutoff_sequence: u64,
        reason: &str,
        now: &str,
    ) -> Result<VaultKey, EnrollError> {
        let revoker_id = revoker.device_id();
        self.require_active(&revoker_id)?;
        self.require_active(revoked)?;
        if *revoked == revoker_id {
            return Err(EnrollError::CannotRevokeSelf);
        }
        let new_epoch = self.key_epoch + 1;
        let key = VaultKey::generate(rng);
        let mut revocation = Revocation {
            revoked_at: now.to_owned(),
            by: revoker_id,
            reason: reason.to_owned(),
            cutoff_sequence,
            epoch_after: new_epoch,
            signature: String::new(),
        };
        revocation.signature = envelope::sign(
            revoker,
            &revocation_input(&self.vault_id, revoked, &revocation),
        );
        let vault_id = self.vault_id.clone();
        self.recovery.wrapped_keys.push(wrap_for_recovery(
            rng,
            &vault_id,
            new_epoch,
            &key,
            &self.recovery.recovery_public,
        )?);
        for record in &mut self.devices {
            if record.device.device_id == *revoked {
                record.revocation = Some(revocation.clone());
            } else if record.is_active() {
                record.wrapped_keys.push(wrap_for_device(
                    rng,
                    &vault_id,
                    new_epoch,
                    &key,
                    &record.device,
                )?);
            }
        }
        self.key_epoch = new_epoch;
        Ok(key)
    }

    /// Records the user-approved link to a remote `(issuer, subject)`. Linking never happens
    /// by matching an email or any other claim; the user confirms it on an active device.
    pub fn link_principal(
        &mut self,
        issuer: &str,
        subject: &str,
        by: &DeviceSecrets,
        now: &str,
    ) -> Result<(), EnrollError> {
        self.require_active(&by.device_id())?;
        self.principal = Some(RemotePrincipal {
            issuer: issuer.to_owned(),
            subject: subject.to_owned(),
            linked_at: now.to_owned(),
            linked_by: by.device_id(),
        });
        Ok(())
    }

    /// The vault key of `epoch` as `device` can unwrap it. A device holds the keys of the
    /// epochs it was active in, so it can still open payloads sealed before a rotation.
    pub fn vault_key_for_epoch(
        &self,
        device: &DeviceSecrets,
        epoch: KeyEpoch,
    ) -> Result<VaultKey, EnrollError> {
        let record = self
            .record(&device.device_id())
            .ok_or(EnrollError::NotEnrolled(device.device_id()))?;
        let wrapped = record
            .wrapped_keys
            .iter()
            .find(|w| w.key_epoch == epoch)
            .ok_or(EnrollError::NoKeyForEpoch(epoch))?;
        Ok(unwrap_for_device(&self.vault_id, wrapped, device)?)
    }

    /// The current vault key as `device` can unwrap it: fails for a device that was never
    /// approved and for a revoked device (no wrap exists for it at the current epoch).
    pub fn vault_key_for(&self, device: &DeviceSecrets) -> Result<VaultKey, EnrollError> {
        self.require_active(&device.device_id()).map_err(|_| {
            match self.record(&device.device_id()) {
                Some(_) => EnrollError::NotActive(device.device_id()),
                None => EnrollError::NotEnrolled(device.device_id()),
            }
        })?;
        self.vault_key_for_epoch(device, self.key_epoch)
    }

    /// The current vault key from the recovery secret.
    pub fn recover(&self, recovery: &RecoverySecret) -> Result<VaultKey, EnrollError> {
        if recovery.public_hex() != self.recovery.recovery_public {
            return Err(EnrollError::WrongRecoverySecret);
        }
        let wrapped = self
            .recovery
            .wrapped_keys
            .iter()
            .find(|w| w.key_epoch == self.key_epoch)
            .ok_or(EnrollError::NoKeyForEpoch(self.key_epoch))?;
        Ok(unwrap_with_recovery(&self.vault_id, wrapped, recovery)?)
    }

    /// Checks every signature and provenance claim in the roster: founder self-approval,
    /// device approvals by devices that were enrolled no later than the approval epoch,
    /// revocations signed by enrolled devices, device ids bound to keys, and one wrapped
    /// key per epoch from enrollment to revocation (or now) for every device and for the
    /// recovery identity.
    pub fn verify(&self) -> Result<(), EnrollError> {
        let founders = self
            .devices
            .iter()
            .filter(|d| matches!(d.approval, Approval::Founder { .. }))
            .count();
        if founders != 1 {
            return Err(EnrollError::Invalid(format!("{founders} founders")));
        }
        let key_of = |id: &DeviceId| -> Result<VerifyingKey, EnrollError> {
            self.record(id)
                .ok_or_else(|| EnrollError::NotEnrolled(id.clone()))?
                .device
                .verifying_key()
                .map_err(EnrollError::from)
        };
        for record in &self.devices {
            record.device.check_id()?;
            let own = record.device.verifying_key()?;
            let (approver, signature, key) = match &record.approval {
                Approval::Founder { signature } => ("founder".to_owned(), signature, own),
                Approval::Device {
                    device_id,
                    signature,
                } => {
                    let approver = self
                        .record(device_id)
                        .ok_or_else(|| EnrollError::NotEnrolled(device_id.clone()))?;
                    if approver.enrolled_epoch > record.enrolled_epoch {
                        return Err(EnrollError::Invalid(format!(
                            "{} approved by a device enrolled later",
                            record.device.device_id
                        )));
                    }
                    (device_id.as_str().to_owned(), signature, key_of(device_id)?)
                }
                Approval::Recovery => (String::new(), &String::new(), own),
            };
            if !matches!(record.approval, Approval::Recovery) {
                let input = approval_input(
                    &self.vault_id,
                    &record.device,
                    record.enrolled_epoch,
                    &approver,
                );
                envelope::verify(&key, &input, signature).map_err(|_| {
                    EnrollError::Invalid(format!(
                        "approval of {} does not verify",
                        record.device.device_id
                    ))
                })?;
            }
            let last_epoch = record
                .revocation
                .as_ref()
                .map_or(self.key_epoch, |r| r.epoch_after - 1);
            let epochs: Vec<KeyEpoch> = record.wrapped_keys.iter().map(|w| w.key_epoch).collect();
            let expected: Vec<KeyEpoch> = (record.enrolled_epoch..=last_epoch).collect();
            if epochs != expected {
                return Err(EnrollError::Invalid(format!(
                    "{} holds keys for epochs {epochs:?}, expected {expected:?}",
                    record.device.device_id
                )));
            }
            if let Some(r) = &record.revocation {
                let input = revocation_input(&self.vault_id, &record.device.device_id, r);
                envelope::verify(&key_of(&r.by)?, &input, &r.signature).map_err(|_| {
                    EnrollError::Invalid(format!(
                        "revocation of {} does not verify",
                        record.device.device_id
                    ))
                })?;
            }
        }
        let recovery_epochs: Vec<KeyEpoch> = self
            .recovery
            .wrapped_keys
            .iter()
            .map(|w| w.key_epoch)
            .collect();
        if recovery_epochs != (1..=self.key_epoch).collect::<Vec<_>>() {
            return Err(EnrollError::Invalid(format!(
                "recovery holds keys for epochs {recovery_epochs:?}"
            )));
        }
        Ok(())
    }
}

impl Authority for Roster {
    fn verifying_key(&self, device_id: &DeviceId) -> Option<VerifyingKey> {
        self.record(device_id)
            .and_then(|r| r.device.verifying_key().ok())
    }

    fn authorizes(&self, device_id: &DeviceId, device_sequence: u64, key_epoch: KeyEpoch) -> bool {
        let Some(record) = self.record(device_id) else {
            return false;
        };
        if key_epoch < record.enrolled_epoch || key_epoch > self.key_epoch {
            return false;
        }
        match &record.revocation {
            None => true,
            Some(r) => device_sequence <= r.cutoff_sequence && key_epoch < r.epoch_after,
        }
    }
}

/// Why an enrollment step was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EnrollError {
    /// The request names another vault.
    #[error("request is for another vault")]
    WrongVault,
    /// The device is already in the roster (active or revoked).
    #[error("device {0} is already enrolled")]
    AlreadyEnrolled(DeviceId),
    /// Never approved.
    #[error("device {0} is not enrolled")]
    NotEnrolled(DeviceId),
    /// Enrolled but revoked.
    #[error("device {0} is not active")]
    NotActive(DeviceId),
    /// A device cannot revoke itself.
    #[error("a device cannot revoke itself")]
    CannotRevokeSelf,
    /// No wrapped key for that epoch.
    #[error("no wrapped key for epoch {0}")]
    NoKeyForEpoch(KeyEpoch),
    /// The recovery secret does not match the roster's recovery public key.
    #[error("recovery secret does not match this vault")]
    WrongRecoverySecret,
    /// Key material does not parse.
    #[error("{0}")]
    Key(#[from] KeyError),
    /// Wrapping or unwrapping failed.
    #[error("{0}")]
    Envelope(#[from] EnvelopeError),
    /// The roster does not verify.
    #[error("invalid roster: {0}")]
    Invalid(String),
}

#[cfg(test)]
mod tests {
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng as _;

    use super::*;
    use crate::envelope::{PayloadContext, open_payload, seal_payload};
    use crate::log::{Change, Draft, OpLog, Outcome, Rejection};

    struct Setup {
        rng: ChaCha20Rng,
        vault: VaultId,
        founder: DeviceSecrets,
        recovery: RecoverySecret,
        roster: Roster,
        key: VaultKey,
    }

    fn setup(seed: u8) -> Setup {
        let mut rng = ChaCha20Rng::from_seed([seed; 32]);
        let vault = VaultId::generate(&mut rng);
        let founder = DeviceSecrets::generate(&mut rng);
        let recovery = RecoverySecret::generate(&mut rng);
        let (roster, key) = Roster::found(
            &mut rng,
            vault.clone(),
            &founder,
            "iPad",
            &recovery.public_hex(),
            "2026-10-28T09:00:00Z",
        )
        .unwrap();
        Setup {
            rng,
            vault,
            founder,
            recovery,
            roster,
            key,
        }
    }

    #[test]
    fn founding_gives_the_founder_and_the_recovery_identity_the_key() {
        let s = setup(1);
        s.roster.verify().unwrap();
        assert_eq!(s.roster.vault_key_for(&s.founder).unwrap(), s.key);
        assert_eq!(s.roster.recover(&s.recovery).unwrap(), s.key);
        assert_eq!(s.roster.active_devices(), vec![s.founder.device_id()]);
    }

    #[test]
    fn a_request_without_approval_cannot_read() {
        let mut s = setup(2);
        let phone = DeviceSecrets::generate(&mut s.rng);
        let request =
            EnrollmentRequest::new(s.vault.clone(), &phone, "phone", "2026-10-28T09:01:00Z");
        let request_json = serde_json::to_string(&request).unwrap();
        assert!(!request_json.contains(&crate::canonical::to_hex(&phone.to_bytes()[..32])));
        assert_eq!(
            s.roster.vault_key_for(&phone),
            Err(EnrollError::NotEnrolled(phone.device_id()))
        );
        let context = PayloadContext {
            vault_id: &s.vault,
            device_id: &s.founder.device_id(),
            device_sequence: 1,
        };
        let sealed = seal_payload(&mut s.rng, &s.key, 1, &context, b"private").unwrap();
        let guess = VaultKey::generate(&mut s.rng);
        assert!(open_payload(&guess, &context, &sealed).is_err());
        // The log refuses the unapproved device's operations outright.
        let mut log = OpLog::new(s.vault.clone());
        let op = Draft {
            vault_id: s.vault.clone(),
            device_sequence: 1,
            parents: vec![],
            key_epoch: 1,
            change: Change::NoteBody {
                note_id: "note-0001".to_owned(),
            },
            payload_hash: None,
        }
        .sign(&phone);
        assert_eq!(log.apply(op, &s.roster), Err(Rejection::UnknownDevice));
        // Approval by the founder changes everything.
        s.roster
            .approve(&mut s.rng, &request, &s.founder, "2026-10-28T09:02:00Z")
            .unwrap();
        s.roster.verify().unwrap();
        assert_eq!(s.roster.vault_key_for(&phone).unwrap(), s.key);
        assert_eq!(
            &open_payload(&s.roster.vault_key_for(&phone).unwrap(), &context, &sealed).unwrap()[..],
            b"private"
        );
        assert!(matches!(
            s.roster.record(&phone.device_id()).unwrap().approval,
            Approval::Device { .. }
        ));
        assert_eq!(
            s.roster.approve(&mut s.rng, &request, &s.founder, "later"),
            Err(EnrollError::AlreadyEnrolled(phone.device_id()))
        );
    }

    #[test]
    fn only_an_active_device_can_approve() {
        let mut s = setup(3);
        let phone = DeviceSecrets::generate(&mut s.rng);
        let stranger = DeviceSecrets::generate(&mut s.rng);
        let request = EnrollmentRequest::new(s.vault.clone(), &phone, "phone", "t");
        assert_eq!(
            s.roster.approve(&mut s.rng, &request, &stranger, "t"),
            Err(EnrollError::NotActive(stranger.device_id()))
        );
        let other_vault =
            EnrollmentRequest::new(VaultId::generate(&mut s.rng), &phone, "phone", "t");
        assert_eq!(
            s.roster.approve(&mut s.rng, &other_vault, &s.founder, "t"),
            Err(EnrollError::WrongVault)
        );
        let mut tampered = request.clone();
        tampered.device.exchange_public = tampered.device.signing_public.clone();
        s.roster
            .approve(&mut s.rng, &tampered, &s.founder, "t")
            .unwrap();
        // The tampered request enrolled a device whose exchange key it does not hold: it
        // cannot unwrap, which is the point of binding the wrap to the request's keys.
        assert!(matches!(
            s.roster.vault_key_for(&phone),
            Err(EnrollError::Envelope(_))
        ));
    }

    #[test]
    fn revocation_rotates_rewraps_and_cuts_off_later_operations() {
        let mut s = setup(4);
        let phone = DeviceSecrets::generate(&mut s.rng);
        let laptop = DeviceSecrets::generate(&mut s.rng);
        for (d, name) in [(&phone, "phone"), (&laptop, "laptop")] {
            let request = EnrollmentRequest::new(s.vault.clone(), d, name, "t");
            s.roster
                .approve(&mut s.rng, &request, &s.founder, "t")
                .unwrap();
        }
        // The phone writes two operations, then is lost.
        let mut log = OpLog::new(s.vault.clone());
        let make = |seq: u64, log: &OpLog| {
            Draft {
                vault_id: s.vault.clone(),
                device_sequence: seq,
                parents: log.heads(),
                key_epoch: 1,
                change: Change::NoteBody {
                    note_id: "note-0001".to_owned(),
                },
                payload_hash: None,
            }
            .sign(&phone)
        };
        let p1 = make(1, &log);
        log.apply(p1, &s.roster).unwrap();
        let p2 = make(2, &log);
        let p3 = make(3, &log); // written before revocation reached the phone, delivered late
        let old_key = s.key;
        let new_key = s
            .roster
            .revoke(&mut s.rng, &phone.device_id(), &laptop, 2, "lost", "t")
            .unwrap();
        s.roster.verify().unwrap();
        assert_eq!(s.roster.key_epoch, 2);
        assert_ne!(new_key, old_key);
        assert_eq!(s.roster.vault_key_for(&laptop).unwrap(), new_key);
        assert_eq!(s.roster.vault_key_for(&s.founder).unwrap(), new_key);
        assert_eq!(s.roster.recover(&s.recovery).unwrap(), new_key);
        assert_eq!(
            s.roster.vault_key_for(&phone),
            Err(EnrollError::NotActive(phone.device_id()))
        );
        assert_eq!(
            s.roster.vault_key_for_epoch(&phone, 2),
            Err(EnrollError::NoKeyForEpoch(2))
        );
        // The documented limit: the phone still holds epoch 1 and can read what it had.
        assert_eq!(s.roster.vault_key_for_epoch(&phone, 1).unwrap(), old_key);
        assert_eq!(s.roster.vault_key_for_epoch(&laptop, 1).unwrap(), old_key);
        assert_eq!(
            s.roster.active_devices(),
            vec![s.founder.device_id(), laptop.device_id()]
        );
        // Operations up to the cutoff are still accepted; later ones are rejected.
        assert!(matches!(
            log.apply(p2, &s.roster),
            Ok(Outcome::Applied { .. })
        ));
        assert_eq!(
            log.apply(p3, &s.roster),
            Err(Rejection::Unauthorized {
                device_id: phone.device_id(),
                device_sequence: 3
            })
        );
        // And nothing sealed at the new epoch by the phone would be accepted either.
        let sneaky = Draft {
            vault_id: s.vault.clone(),
            device_sequence: 2,
            parents: vec![],
            key_epoch: 2,
            change: Change::Tombstone {
                entity_id: "note-0001".to_owned(),
            },
            payload_hash: None,
        }
        .sign(&phone);
        assert!(matches!(
            log.apply(sneaky, &s.roster),
            Err(Rejection::Unauthorized { .. })
        ));
        // A revoked device cannot approve, revoke or re-enroll with the same keys.
        let newcomer = DeviceSecrets::generate(&mut s.rng);
        let request = EnrollmentRequest::new(s.vault.clone(), &newcomer, "new", "t");
        assert_eq!(
            s.roster.approve(&mut s.rng, &request, &phone, "t"),
            Err(EnrollError::NotActive(phone.device_id()))
        );
        assert_eq!(
            s.roster
                .revoke(&mut s.rng, &laptop.device_id(), &phone, 0, "spite", "t"),
            Err(EnrollError::NotActive(phone.device_id()))
        );
        let again = EnrollmentRequest::new(s.vault.clone(), &phone, "phone", "t");
        assert_eq!(
            s.roster.approve(&mut s.rng, &again, &laptop, "t"),
            Err(EnrollError::AlreadyEnrolled(phone.device_id()))
        );
        assert_eq!(
            s.roster
                .revoke(&mut s.rng, &laptop.device_id(), &laptop, 0, "oops", "t"),
            Err(EnrollError::CannotRevokeSelf)
        );
        // A later device learns the current key and the old one for old payloads.
        s.roster
            .approve(&mut s.rng, &request, &laptop, "t")
            .unwrap();
        s.roster.verify().unwrap();
        assert_eq!(s.roster.vault_key_for(&newcomer).unwrap(), new_key);
        assert_eq!(
            s.roster.vault_key_for_epoch(&newcomer, 1),
            Err(EnrollError::NoKeyForEpoch(1))
        );
    }

    #[test]
    fn recovery_material_restores_access_without_any_device() {
        let mut s = setup(5);
        let context = PayloadContext {
            vault_id: &s.vault,
            device_id: &s.founder.device_id(),
            device_sequence: 1,
        };
        let sealed = seal_payload(&mut s.rng, &s.key, 1, &context, b"kept").unwrap();
        // Every device is lost; only the roster (public) and the recovery code remain.
        let roster_json = serde_json::to_string(&s.roster).unwrap();
        let code = s.recovery.display_code();
        drop(s.founder);
        let mut roster: Roster = serde_json::from_str(&roster_json).unwrap();
        let recovered = RecoverySecret::parse_code(&code).unwrap();
        let replacement = DeviceSecrets::generate(&mut s.rng);
        let request = EnrollmentRequest::new(s.vault.clone(), &replacement, "replacement", "t");
        let wrong = RecoverySecret::generate(&mut s.rng);
        assert_eq!(
            roster.approve_with_recovery(&mut s.rng, &request, &wrong, "t"),
            Err(EnrollError::WrongRecoverySecret)
        );
        roster
            .approve_with_recovery(&mut s.rng, &request, &recovered, "t")
            .unwrap();
        roster.verify().unwrap();
        let key = roster.vault_key_for(&replacement).unwrap();
        assert_eq!(&open_payload(&key, &context, &sealed).unwrap()[..], b"kept");
        assert_eq!(
            roster.record(&replacement.device_id()).unwrap().approval,
            Approval::Recovery
        );
    }

    #[test]
    fn the_roster_carries_no_secret_and_rejects_forged_provenance() {
        let mut s = setup(6);
        let phone = DeviceSecrets::generate(&mut s.rng);
        let request = EnrollmentRequest::new(s.vault.clone(), &phone, "phone", "t");
        s.roster
            .approve(&mut s.rng, &request, &s.founder, "t")
            .unwrap();
        s.roster
            .link_principal("https://id.example.org", "sub-123", &s.founder, "t")
            .unwrap();
        let json = serde_json::to_string(&s.roster).unwrap();
        for secret in [
            crate::canonical::to_hex(s.key.as_bytes()),
            crate::canonical::to_hex(&s.founder.to_bytes()[..]),
            crate::canonical::to_hex(&phone.to_bytes()[..32]),
            crate::canonical::to_hex(&phone.to_bytes()[32..]),
            crate::canonical::to_hex(&s.recovery.to_bytes()[..]),
        ] {
            assert!(!json.contains(&secret), "secret material in the roster");
        }
        let restored: Roster = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, s.roster);
        restored.verify().unwrap();
        // Forging an approval: claim the phone approved itself.
        let mut forged = s.roster.clone();
        forged.devices[1].approval = Approval::Device {
            device_id: phone.device_id(),
            signature: match &s.roster.devices[1].approval {
                Approval::Device { signature, .. } => signature.clone(),
                _ => unreachable!(),
            },
        };
        assert!(matches!(forged.verify(), Err(EnrollError::Invalid(_))));
        // Dropping a revocation's signature or claiming an extra founder fails too.
        let mut two_founders = s.roster.clone();
        two_founders.devices[1].approval = Approval::Founder {
            signature: "00".repeat(64),
        };
        assert!(matches!(
            two_founders.verify(),
            Err(EnrollError::Invalid(_))
        ));
        // Principal linking needs an active device and records who did it.
        let principal = s.roster.principal.clone().unwrap();
        assert_eq!(principal.linked_by, s.founder.device_id());
        assert_eq!(principal.subject, "sub-123");
        let stranger = DeviceSecrets::generate(&mut s.rng);
        assert_eq!(
            s.roster
                .link_principal("https://other", "x", &stranger, "t"),
            Err(EnrollError::NotActive(stranger.device_id()))
        );
    }
}
