// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain complete namespace, historical coordinator epoch and positive run/operation identities.
use super::IncidentStoreError;
use serde::{Deserialize, Serialize};
use sugarsubstitute_contracts::{
    incident::{IncidentKey, IncidentOperationId, IncidentSubject, LogicalStoreOperationKey},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OwnerRecord {
    namespace: [u8; 32],
    epoch: [u8; 16],
}
impl OwnerRecord {
    pub(super) fn into_owner(self) -> Result<CoordinatorKey, IncidentStoreError> {
        Ok(CoordinatorKey {
            namespace: NamespaceId::new(self.namespace),
            epoch: CoordinatorEpoch::new(self.epoch)?,
        })
    }
}
impl From<CoordinatorKey> for OwnerRecord {
    fn from(owner: CoordinatorKey) -> Self {
        Self { namespace: *owner.namespace.bytes(), epoch: *owner.epoch.bytes() }
    }
}
mod subject;
pub(super) use subject::SubjectRecord;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OperationRecord {
    owner: OwnerRecord,
    id: u64,
}
impl OperationRecord {
    pub(super) fn into_operation(self) -> Result<LogicalStoreOperationKey, IncidentStoreError> {
        Ok(LogicalStoreOperationKey::new(
            self.owner.into_owner()?,
            IncidentOperationId::new(self.id)?,
        ))
    }
}
impl From<LogicalStoreOperationKey> for OperationRecord {
    fn from(key: LogicalStoreOperationKey) -> Self {
        Self { owner: key.owner().into(), id: key.id().get() }
    }
}
