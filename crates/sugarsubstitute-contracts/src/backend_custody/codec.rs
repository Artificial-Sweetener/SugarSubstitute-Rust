// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode fixed custody messages as matching facts; only authenticated native owners adopt handles.
use super::*;
use crate::lifecycle::{
    CoordinatorEpoch, NamespaceId, ResourceGeneration, ResourceId, ResourceKey,
};
use std::{error::Error, fmt};

/// Complete schema-one custody frame size, in bytes; no variable payload allocation.
/// Profile, heartbeat and trial enrollment extend the tags without changing existing bytes.
/// Older peers reject unknown tags; exact selected bundle/source pairing is required.
pub const BACKEND_CUSTODY_FRAME_BYTES: usize = 384;

mod command;
pub use command::{BackendCustodyCommand, BackendCustodyReceipt, InstallationTrialCustody};

/// Keep one complete purpose/key/nonce association with the bounded command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackendCustodyFrame {
    /// Immutable purpose-specific coordinator/session identity.
    pub key: BackendCustodyKey,
    /// Positive strictly increasing request correlation; responses use that request.
    pub nonce: u64,
    /// Complete fixed command.
    pub command: BackendCustodyCommand,
}

/// Reject malformed or foreign wire facts without allocating error storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackendCustodyWireError {
    /// Version, exact length, magic or reserved bytes were changed.
    Shape,
    /// Zero/foreign identity, invalid handle facts or impossible counter facts.
    Identity,
}
impl fmt::Display for BackendCustodyWireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Backend custody frame rejected: {self:?}")
    }
}
impl Error for BackendCustodyWireError {}

impl BackendCustodyFrame {
    /// Encode exact fields into fixed initialized storage without heap allocation.
    /// # Errors
    /// Rejects invalid correlation/handle/counter facts or a foreign resource owner.
    pub fn encode(self) -> Result<[u8; BACKEND_CUSTODY_FRAME_BYTES], BackendCustodyWireError> {
        self.validate()?;
        let mut bytes = [0; BACKEND_CUSTODY_FRAME_BYTES];
        bytes[..8].copy_from_slice(b"SSCUST01");
        bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
        bytes[16..48].copy_from_slice(self.key.owner().namespace.bytes());
        bytes[48..64].copy_from_slice(self.key.owner().epoch.bytes());
        put(&mut bytes, 64, self.key.session().get());
        put(&mut bytes, 72, self.nonce);
        match self.command {
            BackendCustodyCommand::Heartbeat => bytes[10] = 21,
            BackendCustodyCommand::ProfilePublished { profile } => {
                bytes[10] = 18;
                profile_put(&mut bytes, profile);
            }
            BackendCustodyCommand::ProfilePrepare {
                profile,
                recovery_grant,
                job,
                directory,
                record,
                claim_identity,
                until_ticks,
                frequency,
            } => {
                bytes[10] = if recovery_grant.is_some() { 20 } else { 12 };
                if let Some(grant) = recovery_grant {
                    put(&mut bytes, 320, grant.get());
                }
                profile_put(&mut bytes, profile);
                put(&mut bytes, 104, job);
                put(&mut bytes, 112, directory);
                put(&mut bytes, 120, until_ticks as u64);
                put(&mut bytes, 128, frequency as u64);
                bytes[136..296].copy_from_slice(&record);
                bytes[296..320].copy_from_slice(&claim_identity);
            }
            BackendCustodyCommand::ProfileRoot { profile, process, pid, birth } => {
                bytes[10] = 13;
                profile_put(&mut bytes, profile);
                put(&mut bytes, 104, process);
                put(&mut bytes, 112, u64::from(pid));
                put(&mut bytes, 120, birth);
            }
            BackendCustodyCommand::ProfileStop { profile, until_ticks, frequency } => {
                bytes[10] = 14;
                profile_put(&mut bytes, profile);
                put(&mut bytes, 104, until_ticks as u64);
                put(&mut bytes, 112, frequency as u64);
            }
            BackendCustodyCommand::Hello { capability } => {
                bytes[10] = 1;
                bytes[80..112].copy_from_slice(&capability);
            }
            BackendCustodyCommand::Establish { parent, pid, birth, containment, trial } => {
                bytes[10] = if trial.is_some() { 23 } else { 2 };
                put(&mut bytes, 80, parent);
                put(&mut bytes, 88, u64::from(pid));
                put(&mut bytes, 96, birth);
                put(&mut bytes, 104, containment);
                if let Some(trial) = trial {
                    put(&mut bytes, 112, trial.run);
                    put(&mut bytes, 120, trial.snapshot);
                    put(&mut bytes, 128, trial.until_ticks as u64);
                    put(&mut bytes, 136, trial.frequency as u64);
                }
            }
            BackendCustodyCommand::Prepare {
                resource,
                digest,
                job,
                until_ticks,
                frequency,
                record,
                claim_identity,
                registry_process,
            } => {
                bytes[10] = 3;
                resource_put(&mut bytes, resource);
                bytes[96..128].copy_from_slice(&digest);
                put(&mut bytes, 128, job);
                put(&mut bytes, 136, until_ticks as u64);
                put(&mut bytes, 144, frequency as u64);
                bytes[152..280].copy_from_slice(&record);
                bytes[280..304].copy_from_slice(&claim_identity);
                put(&mut bytes, 304, registry_process);
            }
            BackendCustodyCommand::Root { resource, process, pid, birth, registry_process } => {
                bytes[10] = 4;
                resource_put(&mut bytes, resource);
                put(&mut bytes, 96, process);
                put(&mut bytes, 104, u64::from(pid));
                put(&mut bytes, 112, birth);
                put(&mut bytes, 120, registry_process);
            }
            BackendCustodyCommand::Stop { resource, until_ticks, frequency } => {
                bytes[10] = 5;
                resource_put(&mut bytes, resource);
                put(&mut bytes, 96, until_ticks as u64);
                put(&mut bytes, 104, frequency as u64);
            }
            BackendCustodyCommand::Finish { until_ticks, frequency } => {
                bytes[10] = 10;
                put(&mut bytes, 80, until_ticks as u64);
                put(&mut bytes, 88, frequency as u64);
            }
            BackendCustodyCommand::Receipt(receipt) => {
                bytes[10] = match receipt {
                    BackendCustodyReceipt::Heartbeat => 22,
                    BackendCustodyReceipt::ProfilePublished(_) => 19,
                    BackendCustodyReceipt::ProfilePrepared(_) => 15,
                    BackendCustodyReceipt::ProfileRoot(_) => 16,
                    BackendCustodyReceipt::ProfileReleased(_) => 17,
                    BackendCustodyReceipt::Established => 6,
                    BackendCustodyReceipt::Prepared(_) => 7,
                    BackendCustodyReceipt::Root(_) => 8,
                    BackendCustodyReceipt::Released(_) => 9,
                    BackendCustodyReceipt::Finished => 11,
                };
                match receipt {
                    BackendCustodyReceipt::ProfilePublished(profile) => {
                        profile_put(&mut bytes, profile)
                    }
                    BackendCustodyReceipt::ProfilePrepared(profile)
                    | BackendCustodyReceipt::ProfileRoot(profile)
                    | BackendCustodyReceipt::ProfileReleased(profile) => {
                        profile_put(&mut bytes, profile)
                    }
                    BackendCustodyReceipt::Heartbeat
                    | BackendCustodyReceipt::Established
                    | BackendCustodyReceipt::Finished => {}
                    BackendCustodyReceipt::Prepared(resource)
                    | BackendCustodyReceipt::Root(resource)
                    | BackendCustodyReceipt::Released(resource) => {
                        resource_put(&mut bytes, resource)
                    }
                }
            }
        }
        Ok(bytes)
    }
    /// Decode one exact frame and require byte-for-byte canonical representation.
    /// # Errors
    /// Rejects changed padding, versions, lengths and invalid complete identities.
    pub fn decode(bytes: &[u8]) -> Result<Self, BackendCustodyWireError> {
        let bytes: &[u8; BACKEND_CUSTODY_FRAME_BYTES] =
            bytes.try_into().map_err(|_| BackendCustodyWireError::Shape)?;
        if &bytes[..8] != b"SSCUST01" || bytes[8..10] != [1, 0] {
            return Err(BackendCustodyWireError::Shape);
        }
        let owner = CoordinatorKey {
            namespace: NamespaceId::new(std::array::from_fn(|i| bytes[16 + i])),
            epoch: CoordinatorEpoch::new(std::array::from_fn(|i| bytes[48 + i]))
                .map_err(|_| BackendCustodyWireError::Identity)?,
        };
        let key = BackendCustodyKey::new(
            owner,
            BackendCustodySession::new(get(bytes, 64))
                .map_err(|_| BackendCustodyWireError::Identity)?,
        );
        let resource = || -> Result<ResourceKey, BackendCustodyWireError> {
            Ok(ResourceKey {
                owner,
                id: ResourceId::new(get(bytes, 80))
                    .map_err(|_| BackendCustodyWireError::Identity)?,
                generation: ResourceGeneration::new(get(bytes, 88))
                    .map_err(|_| BackendCustodyWireError::Identity)?,
            })
        };
        let pid = |at| u32::try_from(get(bytes, at)).map_err(|_| BackendCustodyWireError::Identity);
        let profile = || -> Result<ProfileCustodyKey, BackendCustodyWireError> {
            use crate::lifecycle::{InstanceId, RunGeneration, RunKey};
            ProfileCustodyKey::new(
                RunKey {
                    owner,
                    instance: InstanceId::new(get(bytes, 80))
                        .map_err(|_| BackendCustodyWireError::Identity)?,
                    generation: RunGeneration::new(get(bytes, 88))
                        .map_err(|_| BackendCustodyWireError::Identity)?,
                },
                get(bytes, 96),
            )
            .map_err(|_| BackendCustodyWireError::Identity)
        };
        let command = match bytes[10] {
            21 => BackendCustodyCommand::Heartbeat,
            22 => BackendCustodyCommand::Receipt(BackendCustodyReceipt::Heartbeat),
            18 => BackendCustodyCommand::ProfilePublished { profile: profile()? },
            19 => {
                BackendCustodyCommand::Receipt(BackendCustodyReceipt::ProfilePublished(profile()?))
            }
            12 | 20 => BackendCustodyCommand::ProfilePrepare {
                profile: profile()?,
                recovery_grant: if bytes[10] == 20 {
                    Some(
                        crate::recovery_supervisor::RecoveryGrantId::new(get(bytes, 320))
                            .map_err(|_| BackendCustodyWireError::Identity)?,
                    )
                } else {
                    None
                },
                job: get(bytes, 104),
                directory: get(bytes, 112),
                until_ticks: get(bytes, 120) as i64,
                frequency: get(bytes, 128) as i64,
                record: std::array::from_fn(|i| bytes[136 + i]),
                claim_identity: std::array::from_fn(|i| bytes[296 + i]),
            },
            13 => BackendCustodyCommand::ProfileRoot {
                profile: profile()?,
                process: get(bytes, 104),
                pid: pid(112)?,
                birth: get(bytes, 120),
            },
            14 => BackendCustodyCommand::ProfileStop {
                profile: profile()?,
                until_ticks: get(bytes, 104) as i64,
                frequency: get(bytes, 112) as i64,
            },
            15 => {
                BackendCustodyCommand::Receipt(BackendCustodyReceipt::ProfilePrepared(profile()?))
            }
            16 => BackendCustodyCommand::Receipt(BackendCustodyReceipt::ProfileRoot(profile()?)),
            17 => {
                BackendCustodyCommand::Receipt(BackendCustodyReceipt::ProfileReleased(profile()?))
            }
            1 => {
                BackendCustodyCommand::Hello { capability: std::array::from_fn(|i| bytes[80 + i]) }
            }
            2 | 23 => BackendCustodyCommand::Establish {
                parent: get(bytes, 80),
                pid: pid(88)?,
                birth: get(bytes, 96),
                containment: get(bytes, 104),
                trial: (bytes[10] == 23).then(|| InstallationTrialCustody {
                    run: get(bytes, 112),
                    snapshot: get(bytes, 120),
                    until_ticks: get(bytes, 128) as i64,
                    frequency: get(bytes, 136) as i64,
                }),
            },
            3 => BackendCustodyCommand::Prepare {
                resource: resource()?,
                digest: std::array::from_fn(|i| bytes[96 + i]),
                job: get(bytes, 128),
                until_ticks: get(bytes, 136) as i64,
                frequency: get(bytes, 144) as i64,
                record: std::array::from_fn(|i| bytes[152 + i]),
                claim_identity: std::array::from_fn(|i| bytes[280 + i]),
                registry_process: get(bytes, 304),
            },
            4 => BackendCustodyCommand::Root {
                resource: resource()?,
                process: get(bytes, 96),
                pid: pid(104)?,
                birth: get(bytes, 112),
                registry_process: get(bytes, 120),
            },
            5 => BackendCustodyCommand::Stop {
                resource: resource()?,
                until_ticks: get(bytes, 96) as i64,
                frequency: get(bytes, 104) as i64,
            },
            6 => BackendCustodyCommand::Receipt(BackendCustodyReceipt::Established),
            7 => BackendCustodyCommand::Receipt(BackendCustodyReceipt::Prepared(resource()?)),
            8 => BackendCustodyCommand::Receipt(BackendCustodyReceipt::Root(resource()?)),
            9 => BackendCustodyCommand::Receipt(BackendCustodyReceipt::Released(resource()?)),
            11 => BackendCustodyCommand::Receipt(BackendCustodyReceipt::Finished),
            10 => BackendCustodyCommand::Finish {
                until_ticks: get(bytes, 80) as i64,
                frequency: get(bytes, 88) as i64,
            },
            _ => return Err(BackendCustodyWireError::Shape),
        };
        let value = Self { key, nonce: get(bytes, 72), command };
        if value.encode()? != *bytes {
            return Err(BackendCustodyWireError::Shape);
        }
        Ok(value)
    }
    fn validate(&self) -> Result<(), BackendCustodyWireError> {
        let handle = |value| value != 0 && value < u64::MAX - 15;
        let mut valid = self.nonce != 0;
        let profile = match self.command {
            BackendCustodyCommand::ProfilePublished { profile }
            | BackendCustodyCommand::Receipt(BackendCustodyReceipt::ProfilePublished(profile)) => {
                Some(profile)
            }
            BackendCustodyCommand::ProfilePrepare {
                profile,
                job,
                directory,
                until_ticks,
                frequency,
                claim_identity,
                ..
            } => {
                valid &= handle(job)
                    && handle(directory)
                    && until_ticks > 0
                    && frequency > 0
                    && claim_identity != [0; 24];
                Some(profile)
            }
            BackendCustodyCommand::ProfileRoot { profile, process, pid, birth } => {
                valid &= handle(process) && pid != 0 && birth != 0;
                Some(profile)
            }
            BackendCustodyCommand::ProfileStop { profile, until_ticks, frequency } => {
                valid &= until_ticks > 0 && frequency > 0;
                Some(profile)
            }
            BackendCustodyCommand::Receipt(
                BackendCustodyReceipt::ProfilePrepared(profile)
                | BackendCustodyReceipt::ProfileRoot(profile)
                | BackendCustodyReceipt::ProfileReleased(profile),
            ) => Some(profile),
            _ => None,
        };
        valid &= profile.is_none_or(|profile| profile.run().owner == self.key.owner());
        let resource = match self.command {
            BackendCustodyCommand::Heartbeat => None,
            BackendCustodyCommand::ProfilePublished { .. }
            | BackendCustodyCommand::Receipt(BackendCustodyReceipt::ProfilePublished(_)) => None,
            BackendCustodyCommand::ProfilePrepare { .. }
            | BackendCustodyCommand::ProfileRoot { .. }
            | BackendCustodyCommand::ProfileStop { .. }
            | BackendCustodyCommand::Receipt(
                BackendCustodyReceipt::ProfilePrepared(_)
                | BackendCustodyReceipt::ProfileRoot(_)
                | BackendCustodyReceipt::ProfileReleased(_),
            ) => None,
            BackendCustodyCommand::Hello { capability } => {
                valid &= capability.iter().any(|b| *b != 0);
                None
            }
            BackendCustodyCommand::Establish { parent, pid, birth, containment, trial } => {
                valid &= handle(parent)
                    && handle(containment)
                    && parent != containment
                    && pid != 0
                    && birth != 0;
                if let Some(trial) = trial {
                    let handles = [parent, containment, trial.run, trial.snapshot];
                    valid &= trial.until_ticks > 0
                        && trial.frequency > 0
                        && handles.iter().all(|value| handle(*value))
                        && handles
                            .iter()
                            .enumerate()
                            .all(|(index, value)| !handles[..index].contains(value));
                }
                None
            }
            BackendCustodyCommand::Prepare {
                resource,
                job,
                until_ticks,
                frequency,
                claim_identity,
                registry_process,
                ..
            } => {
                valid &= handle(job)
                    && until_ticks > 0
                    && frequency > 0
                    && claim_identity != [0; 24]
                    && registry_process != 0;
                Some(resource)
            }
            BackendCustodyCommand::Root { resource, process, pid, birth, registry_process } => {
                valid &= handle(process) && pid != 0 && birth != 0 && registry_process != 0;
                Some(resource)
            }
            BackendCustodyCommand::Stop { resource, until_ticks, frequency } => {
                valid &= until_ticks > 0 && frequency > 0;
                Some(resource)
            }
            BackendCustodyCommand::Finish { until_ticks, frequency } => {
                valid &= until_ticks > 0 && frequency > 0;
                None
            }
            BackendCustodyCommand::Receipt(
                BackendCustodyReceipt::Heartbeat
                | BackendCustodyReceipt::Established
                | BackendCustodyReceipt::Finished,
            ) => None,
            BackendCustodyCommand::Receipt(
                BackendCustodyReceipt::Prepared(resource)
                | BackendCustodyReceipt::Root(resource)
                | BackendCustodyReceipt::Released(resource),
            ) => Some(resource),
        };
        valid &= resource.is_none_or(|resource| resource.owner == self.key.owner());
        if valid { Ok(()) } else { Err(BackendCustodyWireError::Identity) }
    }
}
impl fmt::Debug for BackendCustodyCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Heartbeat => "Heartbeat",
            Self::ProfilePublished { .. } => "ProfilePublished",
            Self::ProfilePrepare { .. } => "ProfilePrepare { owned Job and directory }",
            Self::ProfileRoot { .. } => "ProfileRoot { owned duplicate }",
            Self::ProfileStop { .. } => "ProfileStop",
            Self::Hello { .. } => "Hello { redacted }",
            Self::Establish { .. } => "Establish { owned duplicates }",
            Self::Prepare { .. } => "Prepare { owned Job }",
            Self::Root { .. } => "Root { owned duplicate }",
            Self::Stop { .. } => "Stop",
            Self::Finish { .. } => "Finish",
            Self::Receipt(_) => "Receipt",
        })
    }
}
fn get(bytes: &[u8; BACKEND_CUSTODY_FRAME_BYTES], at: usize) -> u64 {
    u64::from_le_bytes(std::array::from_fn(|i| bytes[at + i]))
}
fn put(bytes: &mut [u8; BACKEND_CUSTODY_FRAME_BYTES], at: usize, number: u64) {
    bytes[at..at + 8].copy_from_slice(&number.to_le_bytes());
}
fn resource_put(bytes: &mut [u8; BACKEND_CUSTODY_FRAME_BYTES], resource: ResourceKey) {
    put(bytes, 80, resource.id.get());
    put(bytes, 88, resource.generation.get());
}
fn profile_put(bytes: &mut [u8; BACKEND_CUSTODY_FRAME_BYTES], profile: ProfileCustodyKey) {
    put(bytes, 80, profile.run().instance.get());
    put(bytes, 88, profile.run().generation.get());
    put(bytes, 96, profile.registry_process());
}

#[cfg(test)]
mod tests;
