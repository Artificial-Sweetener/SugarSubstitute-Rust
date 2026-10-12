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
#[derive(Serialize, Deserialize)]
#[serde(tag = "subject", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum SubjectRecord {
    Desktop { owner: OwnerRecord, instance: u64, generation: u64 },
    Supervisor { owner: OwnerRecord },
}
impl SubjectRecord {
    pub(super) fn into_key(self) -> Result<IncidentKey, IncidentStoreError> {
        Ok(IncidentKey::new(match self {
            Self::Supervisor { owner } => IncidentSubject::Supervisor(owner.into_owner()?),
            Self::Desktop { owner, instance, generation } => IncidentSubject::Desktop(RunKey {
                owner: owner.into_owner()?,
                instance: InstanceId::new(instance)?,
                generation: RunGeneration::new(generation)?,
            }),
        }))
    }
}
impl From<IncidentKey> for SubjectRecord {
    fn from(key: IncidentKey) -> Self {
        match key.subject() {
            IncidentSubject::Supervisor(owner) => Self::Supervisor { owner: owner.into() },
            IncidentSubject::Desktop(run) => Self::Desktop {
                owner: run.owner.into(),
                instance: run.instance.get(),
                generation: run.generation.get(),
            },
        }
    }
}
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
