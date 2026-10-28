//! The operation log: the pure state machine of specs/03-data-model.md "Notes and
//! synchronization" and ADR 0004.
//!
//! An [`Operation`] is a signed header: globally unique id (a hash of the header), writing
//! device, that device's sequence number, causal parents, key epoch, the change it makes
//! ([`Change`]) and the hash of its sealed payload. [`OpLog::apply`] accepts an operation
//! exactly once, in any order and after any delay: an operation whose parents or whose
//! device predecessor have not arrived waits as *pending* and is released when they do;
//! a replay is reported and ignored; a second operation claiming an already used
//! `(device, sequence)` slot is rejected as equivocation; an unsigned, missigned or
//! unauthorized operation is rejected. [`OpLog::materialize`] is a function of the set of
//! applied operations only, so two logs that received the same operations in different
//! orders agree (property tests at the bottom of this file).
//!
//! Merge rules (deterministic, order-independent):
//! - **Note bodies**: the heads of a note are its body operations not causally dominated by
//!   another body operation of the same note. One head is the current body; several are
//!   sibling revisions for the resolution UI. A body whose parents cover all heads resolves
//!   them. Nothing is ever silently discarded.
//! - **Highlights and tags**: observed-remove sets. A removal cancels exactly the additions
//!   in its causal past; an addition concurrent with a removal survives.
//! - **Tombstones**: a tombstone wins over every concurrent and later change to its entity,
//!   forever. Compaction ([`OpLog::compact`]) drops the entity's operations once every active
//!   device has acknowledged the tombstone, and keeps the entity id so a stale device's late
//!   operations cannot resurrect it.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};

use crate::SYNC_OPERATION_SCHEMA;
use crate::canonical::Canonical;
use crate::envelope::{self, EnvelopeError};
use crate::keys::{DeviceId, DeviceSecrets, KeyEpoch, OpId, VaultId};

const SIGNING_DOMAIN: &str = "openlogos-sync-op/1";

/// What an operation does. Entity and tag ids are opaque stable ids; names, bodies, anchors
/// and every other user-visible value travel in the sealed payload.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Change {
    /// A new body for a note (payload: the body).
    NoteBody {
        /// The note.
        note_id: String,
    },
    /// A highlight comes into being or is restored (payload: its anchor and style).
    HighlightAdd {
        /// The highlight.
        highlight_id: String,
    },
    /// A highlight is removed.
    HighlightRemove {
        /// The highlight.
        highlight_id: String,
    },
    /// A tag is attached to an entity.
    TagAdd {
        /// The note or highlight.
        entity_id: String,
        /// The tag.
        tag_id: String,
    },
    /// A tag is detached from an entity.
    TagRemove {
        /// The note or highlight.
        entity_id: String,
        /// The tag.
        tag_id: String,
    },
    /// An entity is deleted, permanently.
    Tombstone {
        /// The deleted note or highlight.
        entity_id: String,
    },
    /// The writing device has seen the tombstone and dropped its own copy; once every active
    /// device has said so the entity's history can be compacted.
    TombstoneAck {
        /// The deleted entity.
        entity_id: String,
        /// The tombstone acknowledged.
        tombstone: OpId,
    },
}

impl Change {
    /// The entity the change is about.
    #[must_use]
    pub fn entity_id(&self) -> &str {
        match self {
            Change::NoteBody { note_id } => note_id,
            Change::HighlightAdd { highlight_id } | Change::HighlightRemove { highlight_id } => {
                highlight_id
            }
            Change::TagAdd { entity_id, .. }
            | Change::TagRemove { entity_id, .. }
            | Change::Tombstone { entity_id }
            | Change::TombstoneAck { entity_id, .. } => entity_id,
        }
    }

    fn tag(&self) -> &str {
        match self {
            Change::NoteBody { .. } => "note_body",
            Change::HighlightAdd { .. } => "highlight_add",
            Change::HighlightRemove { .. } => "highlight_remove",
            Change::TagAdd { .. } => "tag_add",
            Change::TagRemove { .. } => "tag_remove",
            Change::Tombstone { .. } => "tombstone",
            Change::TombstoneAck { .. } => "tombstone_ack",
        }
    }

    fn tag_id(&self) -> &str {
        match self {
            Change::TagAdd { tag_id, .. } | Change::TagRemove { tag_id, .. } => tag_id,
            _ => "",
        }
    }

    fn acknowledged(&self) -> &str {
        match self {
            Change::TombstoneAck { tombstone, .. } => tombstone.as_str(),
            _ => "",
        }
    }
}

/// An operation before it is signed: everything but the id and the signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    /// The vault.
    pub vault_id: VaultId,
    /// This device's number for the operation: 1 for its first, then +1 each.
    pub device_sequence: u64,
    /// Operations this one was made in knowledge of (normally the log's heads).
    pub parents: Vec<OpId>,
    /// The vault key epoch the payload is sealed under.
    pub key_epoch: KeyEpoch,
    /// The change.
    pub change: Change,
    /// [`crate::envelope::SealedPayload::hash`] of the payload, if the change carries one.
    pub payload_hash: Option<String>,
}

impl Draft {
    /// Signs the draft as `device`, fixing its id.
    #[must_use]
    pub fn sign(self, device: &DeviceSecrets) -> Operation {
        let mut parents = self.parents;
        parents.sort();
        parents.dedup();
        let mut op = Operation {
            schema_version: SYNC_OPERATION_SCHEMA.to_owned(),
            op_id: OpId::for_signing_input(b""),
            vault_id: self.vault_id,
            device_id: device.device_id(),
            device_sequence: self.device_sequence,
            parents,
            key_epoch: self.key_epoch,
            change: self.change,
            payload_hash: self.payload_hash,
            signature: String::new(),
        };
        let input = op.signing_input();
        op.op_id = OpId::for_signing_input(&input);
        op.signature = envelope::sign(device, &input);
        op
    }
}

/// A signed operation header, as it travels and is stored (schemas/sync-operation.schema.json).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Operation {
    /// `1.0`.
    pub schema_version: String,
    /// `op-` + SHA-256 of the signing input.
    pub op_id: OpId,
    /// The vault.
    pub vault_id: VaultId,
    /// The writing (and signing) device.
    pub device_id: DeviceId,
    /// The device's own number for this operation, from 1 without gaps.
    pub device_sequence: u64,
    /// Causal parents, sorted, without duplicates.
    pub parents: Vec<OpId>,
    /// Vault key epoch of the payload.
    pub key_epoch: KeyEpoch,
    /// The change.
    pub change: Change,
    /// Hash of the sealed payload, or null for a change without one.
    pub payload_hash: Option<String>,
    /// Ed25519 signature over the signing input, hex.
    pub signature: String,
}

impl Operation {
    /// The bytes the id commits to and the signature covers.
    #[must_use]
    pub fn signing_input(&self) -> Vec<u8> {
        let mut c = Canonical::new(SIGNING_DOMAIN);
        c.text(&self.schema_version)
            .text(self.vault_id.as_str())
            .text(self.device_id.as_str())
            .u64(self.device_sequence)
            .u32(self.key_epoch)
            .list(
                &self
                    .parents
                    .iter()
                    .map(|p| p.as_str().to_owned())
                    .collect::<Vec<_>>(),
            )
            .text(self.change.tag())
            .text(self.change.entity_id())
            .text(self.change.tag_id())
            .text(self.change.acknowledged())
            .text(self.payload_hash.as_deref().unwrap_or(""));
        c.finish()
    }

    /// Checks shape, id and signature against `authority`, independent of any log.
    pub fn verify(&self, authority: &impl Authority) -> Result<(), Rejection> {
        if self.device_sequence == 0 {
            return Err(Rejection::ZeroSequence);
        }
        if self.parents.contains(&self.op_id) {
            return Err(Rejection::SelfParent);
        }
        if self.parents.windows(2).any(|w| w[0] >= w[1]) {
            return Err(Rejection::Malformed(
                "parents are not sorted and distinct".to_owned(),
            ));
        }
        let input = self.signing_input();
        if OpId::for_signing_input(&input) != self.op_id {
            return Err(Rejection::IdMismatch);
        }
        let key = authority
            .verifying_key(&self.device_id)
            .ok_or(Rejection::UnknownDevice)?;
        envelope::verify(&key, &input, &self.signature).map_err(|e| match e {
            EnvelopeError::BadSignature => Rejection::BadSignature,
            other => Rejection::Malformed(other.to_string()),
        })?;
        if !authority.authorizes(&self.device_id, self.device_sequence, self.key_epoch) {
            return Err(Rejection::Unauthorized {
                device_id: self.device_id.clone(),
                device_sequence: self.device_sequence,
            });
        }
        Ok(())
    }
}

/// Who may write: answered by the roster (`crates/sync::enrollment::Roster`) or, in tests,
/// by anything that knows the keys.
pub trait Authority {
    /// The verifying key of a device that was ever approved, or `None` for a stranger.
    fn verifying_key(&self, device_id: &DeviceId) -> Option<VerifyingKey>;
    /// Whether the operation numbered `device_sequence` from `device_id`, sealed at
    /// `key_epoch`, may enter the log (an active device: always; a revoked device: only up
    /// to its revocation cutoff).
    fn authorizes(&self, device_id: &DeviceId, device_sequence: u64, key_epoch: KeyEpoch) -> bool;
}

/// Why an operation was rejected. A rejected operation leaves the log unchanged.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Rejection {
    /// Another vault's operation.
    #[error("operation belongs to vault {0}")]
    WrongVault(VaultId),
    /// The id is not the hash of the header.
    #[error("operation id does not match its header")]
    IdMismatch,
    /// Sequence 0.
    #[error("device sequence numbers start at 1")]
    ZeroSequence,
    /// An operation naming itself as a parent.
    #[error("operation lists itself as a parent")]
    SelfParent,
    /// A shape problem.
    #[error("malformed operation: {0}")]
    Malformed(String),
    /// Nobody enrolled this device.
    #[error("device is not enrolled")]
    UnknownDevice,
    /// The signature does not verify under the device's key.
    #[error("signature does not verify")]
    BadSignature,
    /// The device may not write this operation (revoked past its cutoff, wrong epoch).
    #[error("device {device_id} is not authorized for sequence {device_sequence}")]
    Unauthorized {
        /// The device.
        device_id: DeviceId,
        /// The sequence it tried.
        device_sequence: u64,
    },
    /// Two different operations claim one `(device, sequence)` slot.
    #[error("device {device_id} already used sequence {device_sequence} for {existing}")]
    Equivocation {
        /// The device.
        device_id: DeviceId,
        /// The slot.
        device_sequence: u64,
        /// The operation already holding it.
        existing: OpId,
    },
}

/// What [`OpLog::apply`] did with an accepted operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Applied now; `released` lists pending operations it unblocked, in application order.
    Applied {
        /// Pending operations applied as a consequence.
        released: Vec<OpId>,
    },
    /// Already applied or already pending: a replay, ignored.
    Duplicate,
    /// Kept aside until what it depends on arrives.
    Pending(Waiting),
}

/// What a pending operation waits for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Waiting {
    /// The device's previous operation has not arrived.
    SequenceGap {
        /// The device.
        device_id: DeviceId,
        /// The sequence number missing.
        missing: u64,
    },
    /// Parents not yet applied.
    MissingParents(Vec<OpId>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Entry {
    Full(Box<Operation>),
    /// An operation whose entity was compacted: enough remains to keep ids, device
    /// sequences and causal ancestry intact.
    Compacted {
        device_id: DeviceId,
        device_sequence: u64,
        parents: Vec<OpId>,
    },
}

impl Entry {
    fn device_id(&self) -> &DeviceId {
        match self {
            Entry::Full(op) => &op.device_id,
            Entry::Compacted { device_id, .. } => device_id,
        }
    }

    fn device_sequence(&self) -> u64 {
        match self {
            Entry::Full(op) => op.device_sequence,
            Entry::Compacted {
                device_sequence, ..
            } => *device_sequence,
        }
    }

    fn parents(&self) -> &[OpId] {
        match self {
            Entry::Full(op) => &op.parents,
            Entry::Compacted { parents, .. } => parents,
        }
    }
}

/// The merged state the log describes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    /// Live notes and their head body operations (one: current; several: siblings), sorted.
    pub notes: BTreeMap<String, Vec<OpId>>,
    /// Live highlights.
    pub highlights: BTreeSet<String>,
    /// `(entity, tag)` pairs in force.
    pub tags: BTreeSet<(String, String)>,
    /// Entities with a tombstone, compacted or not.
    pub deleted: BTreeSet<String>,
}

/// One vault's log on one device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpLog {
    vault_id: VaultId,
    entries: BTreeMap<OpId, Entry>,
    sequences: BTreeMap<DeviceId, BTreeMap<u64, OpId>>,
    pending: BTreeMap<OpId, Operation>,
    compacted: BTreeSet<String>,
}

impl OpLog {
    /// An empty log for `vault_id`.
    #[must_use]
    pub fn new(vault_id: VaultId) -> OpLog {
        OpLog {
            vault_id,
            entries: BTreeMap::new(),
            sequences: BTreeMap::new(),
            pending: BTreeMap::new(),
            compacted: BTreeSet::new(),
        }
    }

    /// The vault.
    #[must_use]
    pub fn vault_id(&self) -> &VaultId {
        &self.vault_id
    }

    /// Applied operations in id order (compacted ones excluded).
    pub fn operations(&self) -> impl Iterator<Item = &Operation> {
        self.entries.values().filter_map(|e| match e {
            Entry::Full(op) => Some(op.as_ref()),
            Entry::Compacted { .. } => None,
        })
    }

    /// Number of applied operations, compacted stubs included.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing has been applied.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Operations waiting for a predecessor or a parent.
    #[must_use]
    pub fn pending(&self) -> Vec<&Operation> {
        self.pending.values().collect()
    }

    /// Whether `id` has been applied.
    #[must_use]
    pub fn contains(&self, id: &OpId) -> bool {
        self.entries.contains_key(id)
    }

    /// Entities whose history was compacted.
    #[must_use]
    pub fn compacted(&self) -> &BTreeSet<String> {
        &self.compacted
    }

    /// The sequence number `device_id` should give its next operation.
    #[must_use]
    pub fn next_sequence(&self, device_id: &DeviceId) -> u64 {
        self.sequences
            .get(device_id)
            .and_then(|s| s.keys().next_back())
            .map_or(1, |last| last + 1)
    }

    /// Applied operations no applied operation names as a parent: what a new operation
    /// should list as its parents.
    #[must_use]
    pub fn heads(&self) -> Vec<OpId> {
        let mut referenced = BTreeSet::new();
        for e in self.entries.values() {
            referenced.extend(e.parents().iter().cloned());
        }
        self.entries
            .keys()
            .filter(|id| !referenced.contains(*id))
            .cloned()
            .collect()
    }

    /// Applies `op` if `authority` accepts it: see the module documentation.
    pub fn apply(
        &mut self,
        op: Operation,
        authority: &impl Authority,
    ) -> Result<Outcome, Rejection> {
        if op.vault_id != self.vault_id {
            return Err(Rejection::WrongVault(op.vault_id));
        }
        op.verify(authority)?;
        if self.entries.contains_key(&op.op_id) || self.pending.contains_key(&op.op_id) {
            return Ok(Outcome::Duplicate);
        }
        if let Some(existing) = self
            .sequences
            .get(&op.device_id)
            .and_then(|s| s.get(&op.device_sequence))
        {
            return Err(Rejection::Equivocation {
                device_id: op.device_id,
                device_sequence: op.device_sequence,
                existing: existing.clone(),
            });
        }
        if let Some(waiting) = self.waiting_for(&op) {
            self.pending.insert(op.op_id.clone(), op);
            return Ok(Outcome::Pending(waiting));
        }
        self.insert(op);
        Ok(Outcome::Applied {
            released: self.release(),
        })
    }

    fn waiting_for(&self, op: &Operation) -> Option<Waiting> {
        if op.device_sequence > 1 {
            let previous = op.device_sequence - 1;
            let have = self
                .sequences
                .get(&op.device_id)
                .is_some_and(|s| s.contains_key(&previous));
            if !have {
                return Some(Waiting::SequenceGap {
                    device_id: op.device_id.clone(),
                    missing: previous,
                });
            }
        }
        // An acknowledgement also depends on the tombstone it names, parent or ancestor.
        let acknowledged = match &op.change {
            Change::TombstoneAck { tombstone, .. } => Some(tombstone),
            _ => None,
        };
        let missing: Vec<OpId> = op
            .parents
            .iter()
            .chain(acknowledged)
            .filter(|p| !self.entries.contains_key(*p))
            .cloned()
            .collect();
        (!missing.is_empty()).then_some(Waiting::MissingParents(missing))
    }

    fn insert(&mut self, op: Operation) {
        self.sequences
            .entry(op.device_id.clone())
            .or_default()
            .insert(op.device_sequence, op.op_id.clone());
        let id = op.op_id.clone();
        if self.compacted.contains(op.change.entity_id()) {
            self.entries.insert(
                id,
                Entry::Compacted {
                    device_id: op.device_id,
                    device_sequence: op.device_sequence,
                    parents: op.parents,
                },
            );
        } else {
            self.entries.insert(id, Entry::Full(Box::new(op)));
        }
    }

    fn release(&mut self) -> Vec<OpId> {
        let mut released = Vec::new();
        loop {
            let ready: Vec<OpId> = self
                .pending
                .values()
                .filter(|op| self.waiting_for(op).is_none())
                .map(|op| op.op_id.clone())
                .collect();
            if ready.is_empty() {
                return released;
            }
            for id in ready {
                if let Some(op) = self.pending.remove(&id) {
                    // A pending operation can lose its slot to one that arrived meanwhile.
                    let taken = self
                        .sequences
                        .get(&op.device_id)
                        .is_some_and(|s| s.contains_key(&op.device_sequence));
                    if !taken {
                        self.insert(op);
                        released.push(id);
                    }
                }
            }
        }
    }

    /// Ancestor sets, with the device predecessor as an implicit parent.
    fn ancestry(&self) -> HashMap<OpId, BTreeSet<OpId>> {
        let mut memo: HashMap<OpId, BTreeSet<OpId>> = HashMap::new();
        for id in self.entries.keys() {
            self.ancestors_of(id, &mut memo);
        }
        memo
    }

    fn ancestors_of<'a>(
        &self,
        id: &OpId,
        memo: &'a mut HashMap<OpId, BTreeSet<OpId>>,
    ) -> &'a BTreeSet<OpId> {
        if !memo.contains_key(id) {
            let mut set = BTreeSet::new();
            let entry = &self.entries[id];
            let mut direct: Vec<OpId> = entry.parents().to_vec();
            if entry.device_sequence() > 1
                && let Some(prev) = self
                    .sequences
                    .get(entry.device_id())
                    .and_then(|s| s.get(&(entry.device_sequence() - 1)))
            {
                direct.push(prev.clone());
            }
            for parent in direct {
                if self.entries.contains_key(&parent) {
                    set.insert(parent.clone());
                    let inherited = self.ancestors_of(&parent, memo).clone();
                    set.extend(inherited);
                }
            }
            memo.insert(id.clone(), set);
        }
        &memo[id]
    }

    /// The merged state: a function of the set of applied operations only.
    #[must_use]
    pub fn materialize(&self) -> State {
        let ancestry = self.ancestry();
        let dominated = |a: &OpId, b: &OpId| ancestry[b].contains(a);
        let mut state = State {
            deleted: self.compacted.clone(),
            ..State::default()
        };
        let ops: Vec<&Operation> = self.operations().collect();
        for op in &ops {
            if let Change::Tombstone { entity_id } = &op.change {
                state.deleted.insert(entity_id.clone());
            }
        }
        let live = |entity: &str| !state.deleted.contains(entity);
        let mut bodies: BTreeMap<&str, Vec<&Operation>> = BTreeMap::new();
        let mut adds: BTreeMap<(String, String), Vec<&Operation>> = BTreeMap::new();
        let mut removes: BTreeMap<(String, String), Vec<&Operation>> = BTreeMap::new();
        for op in &ops {
            match &op.change {
                Change::NoteBody { note_id } if live(note_id) => {
                    bodies.entry(note_id).or_default().push(op);
                }
                Change::HighlightAdd { highlight_id } if live(highlight_id) => {
                    adds.entry((highlight_id.clone(), String::new()))
                        .or_default()
                        .push(op);
                }
                Change::HighlightRemove { highlight_id } if live(highlight_id) => {
                    removes
                        .entry((highlight_id.clone(), String::new()))
                        .or_default()
                        .push(op);
                }
                Change::TagAdd { entity_id, tag_id } if live(entity_id) => {
                    adds.entry((entity_id.clone(), tag_id.clone()))
                        .or_default()
                        .push(op);
                }
                Change::TagRemove { entity_id, tag_id } if live(entity_id) => {
                    removes
                        .entry((entity_id.clone(), tag_id.clone()))
                        .or_default()
                        .push(op);
                }
                _ => {}
            }
        }
        for (note, candidates) in bodies {
            let heads: Vec<OpId> = candidates
                .iter()
                .filter(|a| !candidates.iter().any(|b| dominated(&a.op_id, &b.op_id)))
                .map(|a| a.op_id.clone())
                .collect();
            state.notes.insert(note.to_owned(), heads);
        }
        for ((entity, tag), added) in adds {
            let cancelled = removes.get(&(entity.clone(), tag.clone()));
            let survives = added.iter().any(|a| {
                !cancelled.is_some_and(|rs| rs.iter().any(|r| dominated(&a.op_id, &r.op_id)))
            });
            if survives {
                if tag.is_empty() {
                    state.highlights.insert(entity);
                } else {
                    state.tags.insert((entity, tag));
                }
            }
        }
        state
    }

    /// Tombstones `device_id` has not acknowledged yet: `(entity, tombstone op)`.
    #[must_use]
    pub fn unacknowledged_tombstones(&self, device_id: &DeviceId) -> Vec<(String, OpId)> {
        let acked: BTreeSet<&str> = self
            .operations()
            .filter(|op| op.device_id == *device_id)
            .filter_map(|op| match &op.change {
                Change::TombstoneAck { entity_id, .. } | Change::Tombstone { entity_id } => {
                    Some(entity_id.as_str())
                }
                _ => None,
            })
            .collect();
        self.operations()
            .filter_map(|op| match &op.change {
                Change::Tombstone { entity_id } if !acked.contains(entity_id.as_str()) => {
                    Some((entity_id.clone(), op.op_id.clone()))
                }
                _ => None,
            })
            .collect()
    }

    /// Drops the history of every tombstoned entity that all of `active_devices` have
    /// acknowledged (the tombstone's own writer counts as acknowledging), keeping ids,
    /// sequences and ancestry as stubs and the entity id in [`OpLog::compacted`]. Returns
    /// the entities compacted. With an empty `active_devices` nothing is compacted.
    pub fn compact(&mut self, active_devices: &[DeviceId]) -> Vec<String> {
        if active_devices.is_empty() {
            return Vec::new();
        }
        let mut acked: BTreeMap<String, BTreeSet<&DeviceId>> = BTreeMap::new();
        for op in self.operations() {
            match &op.change {
                Change::Tombstone { entity_id } | Change::TombstoneAck { entity_id, .. } => {
                    acked
                        .entry(entity_id.clone())
                        .or_default()
                        .insert(&op.device_id);
                }
                _ => {}
            }
        }
        let has_tombstone: BTreeSet<String> = self
            .operations()
            .filter_map(|op| match &op.change {
                Change::Tombstone { entity_id } => Some(entity_id.clone()),
                _ => None,
            })
            .collect();
        let ready: Vec<String> = acked
            .into_iter()
            .filter(|(entity, by)| {
                has_tombstone.contains(entity) && active_devices.iter().all(|d| by.contains(d))
            })
            .map(|(entity, _)| entity)
            .collect();
        for entity in &ready {
            self.compacted.insert(entity.clone());
            for entry in self.entries.values_mut() {
                let stub = match entry {
                    Entry::Full(op) if op.change.entity_id() == entity => Entry::Compacted {
                        device_id: op.device_id.clone(),
                        device_sequence: op.device_sequence,
                        parents: op.parents.clone(),
                    },
                    _ => continue,
                };
                *entry = stub;
            }
        }
        ready
    }
}

#[cfg(test)]
pub(crate) mod testing {
    //! A permissive authority for log tests: every known key is active.

    use std::collections::BTreeMap;

    use ed25519_dalek::VerifyingKey;
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng as _;

    use super::{Authority, Change, Draft, OpLog, Operation};
    use crate::keys::{DeviceId, DeviceSecrets, KeyEpoch, OpId, VaultId};

    pub struct Known {
        pub keys: BTreeMap<DeviceId, VerifyingKey>,
    }

    impl Authority for Known {
        fn verifying_key(&self, device_id: &DeviceId) -> Option<VerifyingKey> {
            self.keys.get(device_id).copied()
        }
        fn authorizes(&self, _: &DeviceId, _: u64, _: KeyEpoch) -> bool {
            true
        }
    }

    pub struct World {
        pub vault: VaultId,
        pub devices: Vec<DeviceSecrets>,
        pub known: Known,
        pub logs: Vec<OpLog>,
    }

    impl World {
        pub fn new(seed: u8, devices: usize) -> World {
            let mut rng = ChaCha20Rng::from_seed([seed; 32]);
            let vault = VaultId::generate(&mut rng);
            let devices: Vec<DeviceSecrets> = (0..devices)
                .map(|_| DeviceSecrets::generate(&mut rng))
                .collect();
            let keys = devices
                .iter()
                .map(|d| (d.device_id(), d.public().verifying_key().unwrap()))
                .collect();
            let logs = devices.iter().map(|_| OpLog::new(vault.clone())).collect();
            World {
                vault,
                devices,
                known: Known { keys },
                logs,
            }
        }

        /// Device `i` makes `change` in knowledge of its own log and applies it locally.
        pub fn make(&mut self, i: usize, change: Change) -> Operation {
            let device = &self.devices[i];
            let log = &mut self.logs[i];
            let op = Draft {
                vault_id: self.vault.clone(),
                device_sequence: log.next_sequence(&device.device_id()),
                parents: log.heads(),
                key_epoch: 1,
                change,
                payload_hash: None,
            }
            .sign(device);
            log.apply(op.clone(), &self.known).unwrap();
            op
        }

        /// Device `i` receives everything device `j` has.
        pub fn sync(&mut self, i: usize, j: usize) {
            let ops: Vec<Operation> = self.logs[j].operations().cloned().collect();
            for op in ops {
                self.logs[i].apply(op, &self.known).unwrap();
            }
        }

        pub fn all_ops(&self) -> Vec<Operation> {
            let mut all: BTreeMap<OpId, Operation> = BTreeMap::new();
            for log in &self.logs {
                for op in log.operations() {
                    all.insert(op.op_id.clone(), op.clone());
                }
            }
            all.into_values().collect()
        }
    }

    pub fn note(n: usize) -> Change {
        Change::NoteBody {
            note_id: format!("note-{n:04}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use rand_core::SeedableRng as _;

    use super::testing::{World, note};
    use super::*;

    fn deliver(vault: &VaultId, ops: &[Operation], known: &impl Authority) -> OpLog {
        let mut log = OpLog::new(vault.clone());
        for op in ops {
            log.apply(op.clone(), known).unwrap();
        }
        log
    }

    #[test]
    fn a_replay_is_a_duplicate_and_changes_nothing() {
        let mut w = World::new(1, 1);
        let op = w.make(0, note(1));
        let before = w.logs[0].clone();
        assert_eq!(w.logs[0].apply(op, &w.known).unwrap(), Outcome::Duplicate);
        assert_eq!(w.logs[0], before);
    }

    #[test]
    fn a_second_operation_in_a_used_slot_is_equivocation() {
        let mut w = World::new(2, 1);
        let first = w.make(0, note(1));
        let other = Draft {
            vault_id: w.vault.clone(),
            device_sequence: 1,
            parents: vec![],
            key_epoch: 1,
            change: note(2),
            payload_hash: None,
        }
        .sign(&w.devices[0]);
        assert_eq!(
            w.logs[0].apply(other, &w.known),
            Err(Rejection::Equivocation {
                device_id: w.devices[0].device_id(),
                device_sequence: 1,
                existing: first.op_id
            })
        );
    }

    #[test]
    fn tampering_and_strangers_are_rejected() {
        let mut w = World::new(3, 2);
        let op = w.make(0, note(1));
        let mut forged = op.clone();
        forged.change = note(9);
        assert_eq!(
            w.logs[1].apply(forged, &w.known),
            Err(Rejection::IdMismatch)
        );
        let mut resigned = op.clone();
        resigned.change = note(9);
        resigned.op_id = OpId::for_signing_input(&resigned.signing_input());
        assert_eq!(
            w.logs[1].apply(resigned, &w.known),
            Err(Rejection::BadSignature)
        );
        let stranger = World::new(4, 1);
        let foreign = Draft {
            vault_id: w.vault.clone(),
            device_sequence: 1,
            parents: vec![],
            key_epoch: 1,
            change: note(1),
            payload_hash: None,
        }
        .sign(&stranger.devices[0]);
        assert_eq!(
            w.logs[1].apply(foreign, &w.known),
            Err(Rejection::UnknownDevice)
        );
        let mut other_vault = w.make(0, note(2));
        other_vault.vault_id = VaultId::generate(&mut rand_chacha::ChaCha20Rng::from_seed([9; 32]));
        assert!(matches!(
            w.logs[1].apply(other_vault, &w.known),
            Err(Rejection::WrongVault(_))
        ));
    }

    #[test]
    fn gaps_and_missing_parents_wait_then_release_in_order() {
        let mut w = World::new(5, 2);
        let a1 = w.make(0, note(1));
        let a2 = w.make(0, note(1));
        w.sync(1, 0);
        let b1 = w.make(1, note(1)); // parents: a2
        let mut log = OpLog::new(w.vault.clone());
        assert_eq!(
            log.apply(b1.clone(), &w.known).unwrap(),
            Outcome::Pending(Waiting::MissingParents(vec![a2.op_id.clone()]))
        );
        assert_eq!(
            log.apply(a2.clone(), &w.known).unwrap(),
            Outcome::Pending(Waiting::SequenceGap {
                device_id: w.devices[0].device_id(),
                missing: 1
            })
        );
        assert_eq!(log.pending().len(), 2);
        assert_eq!(
            log.apply(a1.clone(), &w.known).unwrap(),
            Outcome::Applied {
                released: vec![a2.op_id.clone(), b1.op_id.clone()]
            }
        );
        assert!(log.pending().is_empty());
        assert_eq!(log.len(), 3);
        assert_eq!(log.heads(), vec![b1.op_id.clone()]);
        assert_eq!(log.materialize().notes["note-0001"], vec![b1.op_id]);
        assert_eq!(
            log.apply(a2, &w.known).unwrap(),
            Outcome::Duplicate,
            "a released operation is still a replay afterwards"
        );
    }

    #[test]
    fn concurrent_bodies_become_siblings_and_a_covering_body_resolves_them() {
        let mut w = World::new(6, 2);
        let base = w.make(0, note(1));
        w.sync(1, 0);
        let a = w.make(0, note(1));
        let b = w.make(1, note(1));
        w.sync(0, 1);
        let siblings = w.logs[0].materialize();
        assert_eq!(siblings.notes["note-0001"], {
            let mut v = vec![a.op_id.clone(), b.op_id.clone()];
            v.sort();
            v
        });
        assert!(!siblings.notes["note-0001"].contains(&base.op_id));
        let resolved = w.make(0, note(1));
        assert_eq!(
            w.logs[0].materialize().notes["note-0001"],
            vec![resolved.op_id]
        );
    }

    #[test]
    fn highlights_and_tags_are_observed_remove_sets() {
        let mut w = World::new(7, 2);
        let h = |n: usize| format!("hl-{n:04}");
        w.make(0, Change::HighlightAdd { highlight_id: h(1) });
        w.sync(1, 0);
        w.make(1, Change::HighlightRemove { highlight_id: h(1) });
        w.make(0, Change::HighlightAdd { highlight_id: h(1) }); // concurrent with the removal: survives
        w.make(
            0,
            Change::TagAdd {
                entity_id: h(1),
                tag_id: "tag-faith".to_owned(),
            },
        );
        w.sync(0, 1);
        w.sync(1, 0);
        let s = w.logs[0].materialize();
        assert_eq!(s, w.logs[1].materialize());
        assert!(s.highlights.contains(&h(1)));
        assert!(s.tags.contains(&(h(1), "tag-faith".to_owned())));
        w.make(
            1,
            Change::TagRemove {
                entity_id: h(1),
                tag_id: "tag-faith".to_owned(),
            },
        );
        w.make(1, Change::HighlightRemove { highlight_id: h(1) });
        w.sync(0, 1);
        let s = w.logs[0].materialize();
        assert!(s.highlights.is_empty());
        assert!(s.tags.is_empty());
    }

    #[test]
    fn a_tombstone_wins_and_a_stale_device_cannot_resurrect() {
        let mut w = World::new(8, 3);
        w.make(0, note(1));
        w.sync(1, 0);
        w.sync(2, 0);
        w.make(
            0,
            Change::Tombstone {
                entity_id: "note-0001".to_owned(),
            },
        );
        // Device 2 is stale: it edits the note without having seen the tombstone.
        w.make(2, note(1));
        w.make(
            2,
            Change::TagAdd {
                entity_id: "note-0001".to_owned(),
                tag_id: "tag-x".to_owned(),
            },
        );
        w.sync(0, 2);
        w.sync(2, 0);
        for log in &w.logs[..1] {
            let s = log.materialize();
            assert!(s.deleted.contains("note-0001"));
            assert!(!s.notes.contains_key("note-0001"));
            assert!(s.tags.is_empty());
        }
        assert_eq!(w.logs[0].materialize(), w.logs[2].materialize());
    }

    #[test]
    fn compaction_needs_every_active_acknowledgement_and_survives_late_operations() {
        let mut w = World::new(9, 3);
        w.make(0, note(1));
        w.make(0, note(2));
        w.sync(1, 0);
        w.sync(2, 0);
        let late = w.make(2, note(1)); // device 2 edits before learning of the deletion
        let tomb = w.make(
            0,
            Change::Tombstone {
                entity_id: "note-0001".to_owned(),
            },
        );
        w.sync(1, 0);
        let active: Vec<DeviceId> = w.devices.iter().map(|d| d.device_id()).collect();
        assert_eq!(
            w.logs[1].unacknowledged_tombstones(&active[1]),
            vec![("note-0001".to_owned(), tomb.op_id.clone())]
        );
        assert!(
            w.logs[1].compact(&active).is_empty(),
            "nobody else has acknowledged"
        );
        w.make(
            1,
            Change::TombstoneAck {
                entity_id: "note-0001".to_owned(),
                tombstone: tomb.op_id.clone(),
            },
        );
        assert!(w.logs[1].unacknowledged_tombstones(&active[1]).is_empty());
        w.sync(2, 1); // device 2 learns of the tombstone, acknowledges
        w.make(
            2,
            Change::TombstoneAck {
                entity_id: "note-0001".to_owned(),
                tombstone: tomb.op_id.clone(),
            },
        );
        w.sync(1, 2);
        let before = w.logs[1].len();
        assert_eq!(w.logs[1].compact(&active), vec!["note-0001".to_owned()]);
        assert_eq!(w.logs[1].len(), before, "stubs keep ids and sequences");
        assert!(
            w.logs[1]
                .operations()
                .all(|op| op.change.entity_id() != "note-0001")
        );
        let s = w.logs[1].materialize();
        assert!(s.deleted.contains("note-0001"));
        assert_eq!(s.notes.len(), 1);
        // The late edit arrives again from a stale copy: a duplicate, no resurrection.
        assert_eq!(w.logs[1].apply(late, &w.known).unwrap(), Outcome::Duplicate);
        // A brand new late operation for the compacted entity is accepted but inert.
        let fresh_late = w.make(2, note(1));
        let outcome = w.logs[1].apply(fresh_late.clone(), &w.known).unwrap();
        assert!(matches!(outcome, Outcome::Applied { .. }));
        assert!(
            !w.logs[1]
                .operations()
                .any(|op| op.op_id == fresh_late.op_id)
        );
        assert!(!w.logs[1].materialize().notes.contains_key("note-0001"));
        // Compaction with no acknowledgement from a partially synced device would not happen.
        let mut fresh = OpLog::new(w.vault.clone());
        fresh.apply(tomb, &w.known).ok();
        assert!(fresh.compact(&active).is_empty());
    }

    #[test]
    fn malformed_shapes_are_rejected() {
        let w = World::new(10, 1);
        let device = &w.devices[0];
        let zero = Draft {
            vault_id: w.vault.clone(),
            device_sequence: 0,
            parents: vec![],
            key_epoch: 1,
            change: note(1),
            payload_hash: None,
        }
        .sign(device);
        assert_eq!(zero.verify(&w.known), Err(Rejection::ZeroSequence));
        let ack_without_parent = Draft {
            vault_id: w.vault.clone(),
            device_sequence: 1,
            parents: vec![],
            key_epoch: 1,
            change: Change::TombstoneAck {
                entity_id: "note-0001".to_owned(),
                tombstone: OpId::for_signing_input(b"x"),
            },
            payload_hash: None,
        }
        .sign(device);
        ack_without_parent.verify(&w.known).unwrap();
        let mut log = OpLog::new(w.vault.clone());
        assert!(matches!(
            log.apply(ack_without_parent, &w.known).unwrap(),
            Outcome::Pending(Waiting::MissingParents(_))
        ));
        let json = serde_json::to_value(&zero).unwrap();
        assert_eq!(json["change"]["kind"], "note_body");
        assert_eq!(json["payload_hash"], serde_json::Value::Null);
    }

    #[derive(Debug, Clone)]
    enum Step {
        Make(usize, Change),
        Sync(usize, usize),
    }

    fn change_strategy() -> impl Strategy<Value = Change> {
        let notes = 1..4usize;
        let hl = 1..3usize;
        let tag = 1..3usize;
        prop_oneof![
            4 => notes.clone().prop_map(note),
            2 => hl.clone().prop_map(|n| Change::HighlightAdd { highlight_id: format!("hl-{n:04}") }),
            2 => hl.clone().prop_map(|n| Change::HighlightRemove { highlight_id: format!("hl-{n:04}") }),
            2 => (notes.clone(), tag.clone()).prop_map(|(n, t)| Change::TagAdd { entity_id: format!("note-{n:04}"), tag_id: format!("tag-{t}") }),
            2 => (notes.clone(), tag).prop_map(|(n, t)| Change::TagRemove { entity_id: format!("note-{n:04}"), tag_id: format!("tag-{t}") }),
            1 => notes.prop_map(|n| Change::Tombstone { entity_id: format!("note-{n:04}") }),
            1 => hl.prop_map(|n| Change::Tombstone { entity_id: format!("hl-{n:04}") }),
        ]
    }

    fn step_strategy(devices: usize) -> impl Strategy<Value = Step> {
        prop_oneof![
            3 => (0..devices, change_strategy()).prop_map(|(d, c)| Step::Make(d, c)),
            1 => (0..devices, 0..devices).prop_map(|(i, j)| Step::Sync(i, j)),
        ]
    }

    fn run(seed: u8, devices: usize, steps: &[Step]) -> World {
        let mut w = World::new(seed, devices);
        for step in steps {
            match step {
                Step::Make(d, c) => {
                    w.make(*d, c.clone());
                }
                Step::Sync(i, j) => w.sync(*i, *j),
            }
        }
        w
    }

    fn permute<T: Clone>(items: &[T], keys: &[u32]) -> Vec<T> {
        let mut indexed: Vec<(u32, usize)> = (0..items.len())
            .map(|i| (keys.get(i).copied().unwrap_or(0), i))
            .collect();
        indexed.sort();
        indexed.into_iter().map(|(_, i)| items[i].clone()).collect()
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(48))]

        /// Any delivery order, with replays, of any set of operations converges on the same
        /// applied set and the same state, with nothing left pending.
        #[test]
        fn convergence_under_reordering_duplication_and_delay(
            steps in prop::collection::vec(step_strategy(3), 1..30),
            keys_a in prop::collection::vec(any::<u32>(), 30),
            keys_b in prop::collection::vec(any::<u32>(), 30),
            dupes in prop::collection::vec(any::<prop::sample::Index>(), 0..6),
            split in 0..30usize,
        ) {
            let w = run(11, 3, &steps);
            let all = w.all_ops();
            prop_assume!(!all.is_empty());
            let order_a = permute(&all, &keys_a);
            let mut order_b = permute(&all, &keys_b);
            for d in &dupes {
                let op = all[d.index(all.len())].clone();
                order_b.insert(d.index(order_b.len() + 1), op);
            }
            let log_a = deliver(&w.vault, &order_a, &w.known);
            // Delayed: the second half arrives only after the first was fully processed.
            let cut = split.min(order_b.len());
            let mut log_b = OpLog::new(w.vault.clone());
            for op in &order_b[..cut] { log_b.apply(op.clone(), &w.known).unwrap(); }
            let _ = log_b.materialize();
            for op in &order_b[cut..] { log_b.apply(op.clone(), &w.known).unwrap(); }
            prop_assert!(log_a.pending().is_empty());
            prop_assert!(log_b.pending().is_empty());
            prop_assert_eq!(log_a.len(), all.len());
            prop_assert_eq!(&log_a, &log_b);
            prop_assert_eq!(log_a.materialize(), log_b.materialize());
            // Every device's own view, once it has everything, agrees too.
            let mut full = w.logs[0].clone();
            for op in &all { full.apply(op.clone(), &w.known).unwrap(); }
            prop_assert_eq!(full.materialize(), log_a.materialize());
        }

        /// A tombstone is final whatever arrives, and the per-entity rules hold.
        #[test]
        fn tombstones_are_final_and_rules_hold(
            steps in prop::collection::vec(step_strategy(3), 1..30),
            keys in prop::collection::vec(any::<u32>(), 30),
        ) {
            let w = run(12, 3, &steps);
            let all = w.all_ops();
            prop_assume!(!all.is_empty());
            let log = deliver(&w.vault, &permute(&all, &keys), &w.known);
            let state = log.materialize();
            for op in &all {
                if let Change::Tombstone { entity_id } = &op.change {
                    prop_assert!(state.deleted.contains(entity_id));
                    prop_assert!(!state.notes.contains_key(entity_id));
                    prop_assert!(!state.highlights.contains(entity_id));
                    prop_assert!(state.tags.iter().all(|(e, _)| e != entity_id));
                }
            }
            for heads in state.notes.values() {
                prop_assert!(!heads.is_empty());
                let mut sorted = heads.clone();
                sorted.dedup();
                prop_assert_eq!(&sorted, heads);
            }
        }

        /// Compaction after every active device acknowledged keeps the state identical and
        /// removes the entity's operations; late arrivals do not bring it back.
        #[test]
        fn compaction_preserves_state_and_blocks_resurrection(
            steps in prop::collection::vec(step_strategy(2), 1..20),
            keys in prop::collection::vec(any::<u32>(), 40),
        ) {
            let mut w = run(13, 2, &steps);
            w.sync(0, 1);
            w.sync(1, 0);
            let active: Vec<DeviceId> = w.devices.iter().map(|d| d.device_id()).collect();
            for (d, device) in active.iter().enumerate() {
                for (entity, tomb) in w.logs[d].unacknowledged_tombstones(device) {
                    w.make(d, Change::TombstoneAck { entity_id: entity, tombstone: tomb });
                }
            }
            w.sync(0, 1);
            w.sync(1, 0);
            let all = w.all_ops();
            let before = w.logs[0].materialize();
            let compacted = w.logs[0].compact(&active);
            prop_assert_eq!(w.logs[0].materialize(), before.clone());
            for entity in &compacted {
                prop_assert!(before.deleted.contains(entity));
                prop_assert!(w.logs[0].operations().all(|op| op.change.entity_id() != entity));
            }
            let expected: BTreeSet<String> = before.deleted.iter().cloned().collect();
            prop_assert_eq!(BTreeSet::from_iter(compacted.iter().cloned()), expected);
            for op in permute(&all, &keys) {
                prop_assert_eq!(w.logs[0].apply(op, &w.known).unwrap(), Outcome::Duplicate);
            }
            prop_assert_eq!(w.logs[0].materialize(), before);
        }
    }
}
