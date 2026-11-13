// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Canonical prelaunch origin bytes grant no live or historical native opening authority.
use super::*;
use sugarsubstitute_contracts::crash::RecoveryCaptureIdentity;
/// Durable prelaunch matching facts for exactly one genuine Root-issued successor grant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryCaptureOriginRecord {
    operation: sugarsubstitute_contracts::incident::LogicalStoreOperationKey,
    origin: RecoveryCaptureIdentity,
    root_identity: [u8; 24],
    capture_key: CoordinatorKey,
    build: BuildIdentity,
}
impl RecoveryCaptureOriginRecord {
    pub(crate) fn new(
        operation: sugarsubstitute_contracts::incident::LogicalStoreOperationKey,
        origin: RecoveryCaptureIdentity,
        root_identity: [u8; 24],
        build: &BuildIdentity,
    ) -> Result<Self, IncidentStoreError> {
        if operation.owner() != origin.run().owner || root_identity == [0; 24] {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        let capture_key = CaptureGenerationOrigin::Recovery(origin).capture_key()?;
        Ok(Self { operation, origin, root_identity, capture_key, build: build.clone() })
    }
    /// Observe the original preparation operation, never a renewed current operation.
    pub const fn operation(&self) -> sugarsubstitute_contracts::incident::LogicalStoreOperationKey {
        self.operation
    }
    /// Observe exact historical Root/run/grant correlation.
    pub const fn origin(&self) -> RecoveryCaptureIdentity {
        self.origin
    }
    /// Borrow the exact producer build durably retained before launch.
    pub const fn build(&self) -> &BuildIdentity {
        &self.build
    }
    /// Observe the actual protected root identity recorded before launch.
    pub const fn root_identity(&self) -> [u8; 24] {
        self.root_identity
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::incident) struct OriginJournalRecord {
    operation: OperationRecord,
    origin: super::recovery::OriginRecord,
    root_identity: [u8; 24],
    capture_key: OwnerRecord,
    build: String,
}
impl From<&RecoveryCaptureOriginRecord> for OriginJournalRecord {
    fn from(record: &RecoveryCaptureOriginRecord) -> Self {
        Self {
            operation: record.operation.into(),
            origin: record.origin.into(),
            root_identity: record.root_identity,
            capture_key: record.capture_key.into(),
            build: record.build.as_str().into(),
        }
    }
}
impl OriginJournalRecord {
    pub(in crate::incident) fn into_record(
        self,
    ) -> Result<RecoveryCaptureOriginRecord, IncidentStoreError> {
        let CaptureGenerationOrigin::Recovery(origin) = self.origin.into_origin()? else {
            return Err(IncidentStoreError::RecoveryConflict);
        };
        let record = RecoveryCaptureOriginRecord::new(
            self.operation.into_operation()?,
            origin,
            self.root_identity,
            &BuildIdentity::new(&self.build)?,
        )?;
        if self.capture_key.into_owner()?
            != CaptureGenerationOrigin::Recovery(origin).capture_key()?
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        Ok(record)
    }
}

#[cfg(test)]
mod tests;
