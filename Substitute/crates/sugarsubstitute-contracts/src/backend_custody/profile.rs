// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Correlate profile custody with a genuine desktop run and prepared registry family.
use crate::lifecycle::{CoordinatorKey, IdentityError, InstanceId, RunGeneration, RunKey};
use std::num::NonZeroU64;

/// Immutable matching facts; only native custody owners create/adopt the actual Job.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileCustodyKey {
    run: RunKey,
    registry_process: NonZeroU64,
}
/// Positive profile coordinates within a separately retained coordinator owner.
/// These matching facts grant no native ownership or authenticated receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileCustodyIdentity {
    instance: InstanceId,
    generation: RunGeneration,
    registry_process: NonZeroU64,
}
impl ProfileCustodyIdentity {
    /// Restore the unchanged complete matching key using its retained frame owner.
    pub const fn key(self, owner: CoordinatorKey) -> ProfileCustodyKey {
        ProfileCustodyKey {
            run: RunKey { owner, instance: self.instance, generation: self.generation },
            registry_process: self.registry_process,
        }
    }
}
impl ProfileCustodyKey {
    /// Project positive coordinates while retaining the owner with its authenticated frame.
    pub const fn identity(self) -> ProfileCustodyIdentity {
        ProfileCustodyIdentity {
            instance: self.run.instance,
            generation: self.run.generation,
            registry_process: self.registry_process,
        }
    }
    /// Validate the prepared registry identity without projecting native authority.
    /// # Errors
    /// A zero registry identity is rejected.
    pub const fn new(run: RunKey, registry_process: u64) -> Result<Self, IdentityError> {
        match NonZeroU64::new(registry_process) {
            Some(registry_process) => Ok(Self { run, registry_process }),
            None => Err(IdentityError::Zero),
        }
    }
    /// Observe the exact desktop generation.
    pub const fn run(self) -> RunKey {
        self.run
    }
    /// Observe the registry family identity, distinct from a kernel PID.
    pub const fn registry_process(self) -> u64 {
        self.registry_process.get()
    }
}
