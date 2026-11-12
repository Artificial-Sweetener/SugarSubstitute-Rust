// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Distinguish physical capture correlation from the unchanged actual Root authority.
use crate::{
    incident::IncidentSubject,
    lifecycle::{CoordinatorKey, IdentityError, RunKey},
    recovery_supervisor::{RecoveryGrantId, RecoveryGrantKey},
};
use sha2::{Digest, Sha256};
/// Complete inert identity of one Root-granted fresh recovery capture producer.
/// Actual grant/session and native creator/handler custody authenticate these facts.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RecoveryCaptureIdentity {
    run: RunKey,
    grant: RecoveryGrantId,
}
impl RecoveryCaptureIdentity {
    /// Retain validated matching facts without issuing capture, election or file privilege.
    pub const fn new(run: RunKey, grant: RecoveryGrantId) -> Self {
        Self { run, grant }
    }
    /// Project the exact successor from the complete original Root grant correlation.
    pub const fn from_grant(key: RecoveryGrantKey) -> Self {
        Self::new(key.run(), key.id())
    }
    /// Return the exact recovered instance/generation under its unchanged Root key.
    pub const fn run(self) -> RunKey {
        self.run
    }
    /// Return the exact once-only Root grant within that coordinator lifetime.
    pub const fn grant(self) -> RecoveryGrantId {
        self.grant
    }
}
/// Exact physical capture origin, independently persisted with each disposition plan.
/// The native capture key identifies annotations/directories only. Every logical
/// operation and live coordinator remains bound to the real Root returned by coordinator.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CaptureGenerationOrigin {
    /// Original Root registration and unchanged capture-v1 directory spelling.
    Coordinator(CoordinatorKey),
    /// Fresh process/database/metrics origin for exactly one authenticated successor grant.
    Recovery(RecoveryCaptureIdentity),
}
impl CaptureGenerationOrigin {
    /// Return the actual Root operation/entry authority; recovery never transfers epochs.
    pub const fn coordinator(self) -> CoordinatorKey {
        match self {
            Self::Coordinator(owner) => owner,
            Self::Recovery(recovery) => recovery.run.owner,
        }
    }
    /// Derive immutable physical-generation/annotation correlation, never coordinator authority.
    /// Recovery uses the first16 SHA256 bytes over the fixed domain, original Root
    /// namespace/epoch and little-endian recovered instance/generation/grant ID.
    /// # Errors
    /// Rejects an all-zero derived epoch without substituting another identity.
    pub fn capture_key(self) -> Result<CoordinatorKey, IdentityError> {
        match self {
            Self::Coordinator(owner) => Ok(owner),
            Self::Recovery(recovery) => {
                let mut hash = Sha256::new();
                hash.update(b"SugarSubstitute.recovery-supervisor.capture.v1\0");
                hash.update(recovery.run.owner.namespace.bytes());
                hash.update(recovery.run.owner.epoch.bytes());
                hash.update(recovery.run.instance.get().to_le_bytes());
                hash.update(recovery.run.generation.get().to_le_bytes());
                hash.update(recovery.grant.get().to_le_bytes());
                let digest = hash.finalize();
                let mut epoch = [0; 16];
                epoch.copy_from_slice(&digest[..16]);
                Ok(CoordinatorKey {
                    namespace: recovery.run.owner.namespace,
                    epoch: crate::lifecycle::CoordinatorEpoch::new(epoch)?,
                })
            }
        }
    }
    /// Return the strict existing Supervisor subject used solely for producer correlation.
    /// # Errors
    /// Preserves an invalid derived capture epoch.
    pub fn producer_subject(self) -> Result<IncidentSubject, IdentityError> {
        Ok(IncidentSubject::Supervisor(self.capture_key()?))
    }
    /// Match the exact origin's producer or permitted Desktop family.
    /// Held directory identities and genuine grant/current operation custody must
    /// independently validate this inert relation before query or disposition effects.
    /// # Errors
    /// Preserves an invalid derived capture epoch.
    pub fn contains(self, subject: IncidentSubject) -> Result<bool, IdentityError> {
        Ok(match (self, subject) {
            (Self::Coordinator(owner), IncidentSubject::Supervisor(actual)) => owner == actual,
            (Self::Coordinator(owner), IncidentSubject::Desktop(run)) => owner == run.owner,
            (Self::Recovery(recovery), IncidentSubject::Desktop(run)) => recovery.run == run,
            (Self::Recovery(_), IncidentSubject::Supervisor(actual)) => {
                self.capture_key()? == actual
            }
        })
    }
    /// Write the canonical single-component create-only capture-v1 name.
    /// Legacy Root names stay byte-identical; recovery has a domain-separated key
    /// under the same protected Root location. No name is accepted from the child.
    /// # Errors
    /// Preserves exact derivation or bounded destination failure.
    pub fn write_name(
        self,
        output: &mut impl std::fmt::Write,
    ) -> Result<(), CaptureOriginNameError> {
        let capture = self.capture_key().map_err(CaptureOriginNameError::Identity)?;
        output.write_str("capture-v1-").map_err(CaptureOriginNameError::Format)?;
        for byte in capture.namespace.bytes().iter().chain(capture.epoch.bytes()) {
            write!(output, "{byte:02x}").map_err(CaptureOriginNameError::Format)?;
        }
        Ok(())
    }
}
/// Preserve identity derivation and bounded name destination failures independently.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureOriginNameError {
    /// The complete origin derived an invalid all-zero capture epoch.
    Identity(IdentityError),
    /// The caller-owned destination rejected the canonical component.
    Format(std::fmt::Error),
}
impl std::fmt::Display for CaptureOriginNameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Capture origin name rejected")
    }
}
impl std::error::Error for CaptureOriginNameError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(e) => Some(e),
            Self::Format(e) => Some(e),
        }
    }
}
#[cfg(test)]
mod tests;
