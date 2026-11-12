// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve exact physical recovery origin in the strict schema2 capture journal.
use super::*;
use sugarsubstitute_contracts::{
    crash::RecoveryCaptureIdentity,
    lifecycle::{InstanceId, RunGeneration, RunKey},
    recovery_supervisor::RecoveryGrantId,
};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OriginRecord {
    root: OwnerRecord,
    instance: u64,
    generation: u64,
    grant: u64,
}
impl OriginRecord {
    pub(super) fn into_origin(self) -> Result<CaptureGenerationOrigin, IncidentStoreError> {
        let run = RunKey {
            owner: self.root.into_owner()?,
            instance: InstanceId::new(self.instance)?,
            generation: RunGeneration::new(self.generation)?,
        };
        let grant =
            RecoveryGrantId::new(self.grant).map_err(|_| IncidentStoreError::RecoveryConflict)?;
        Ok(CaptureGenerationOrigin::Recovery(RecoveryCaptureIdentity::new(run, grant)))
    }
}
/// Schema2 requires full origin in addition to independently verified canonical file facts.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::incident) struct RecoveryCaptureJournalRecord {
    origin: OriginRecord,
    capture: CaptureJournalRecord,
}
impl RecoveryCaptureJournalRecord {
    pub(in crate::incident) fn from_plan(plan: &CaptureDispositionPlan) -> Option<Self> {
        let CaptureGenerationOrigin::Recovery(origin) = plan.capture_origin else {
            return None;
        };
        let run = origin.run();
        Some(Self {
            origin: OriginRecord {
                root: run.owner.into(),
                instance: run.instance.get(),
                generation: run.generation.get(),
                grant: origin.grant().get(),
            },
            capture: plan.into(),
        })
    }
    pub(in crate::incident) fn into_plan(
        self,
    ) -> Result<CaptureDispositionPlan, IncidentStoreError> {
        self.capture.into_origin_plan(Some(self.origin.into_origin()?))
    }
}

impl From<sugarsubstitute_contracts::crash::RecoveryCaptureIdentity> for OriginRecord {
    fn from(origin: sugarsubstitute_contracts::crash::RecoveryCaptureIdentity) -> Self {
        let run = origin.run();
        Self {
            root: run.owner.into(),
            instance: run.instance.get(),
            generation: run.generation.get(),
            grant: origin.grant().get(),
        }
    }
}
