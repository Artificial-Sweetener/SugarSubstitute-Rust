// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Seal genuine custody or authenticated recovery provenance at final creation admission.
use crate::{
    admission::{Core, State},
    backend_custody::BackendCustodyParentStorage,
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{AdmissionError, JobSpec, Moment};

/// Construction is private; matching facts never manufacture either actual source.
#[derive(Clone)]
#[expect(
    clippy::large_enum_variant,
    reason = "ProcessCreationMemory counts this inline source in its original 128 KiB claim; no extra owner allocation is admitted"
)]
pub(crate) enum Source {
    Custody { parent: BackendCustodyParentStorage, grant: Option<crate::RegisteredRecoveryGrant> },
    Recovery(crate::recovery_supervisor::startup::SourceRef),
}
impl Source {
    /// Validate actual source liveness, original deadline and once-only held admission.
    pub(crate) fn admit_locked(
        &self,
        core: &Arc<Core>,
        state: &State,
        spec: JobSpec,
        now: Moment,
    ) -> Result<(), AdmissionError> {
        self.check_locked(core, state, now)?;
        match self {
            Self::Custody { parent, grant: Some(grant) } => {
                let grant = grant.state()?;
                let stages = crate::scope::lock(&grant.stages);
                if parent.key().owner() != grant.key.run().owner
                    || stages.creation.is_some()
                    || stages.committed
                    || stages.closed
                    || stages.pipe.is_none()
                    || spec.deadline != Some(grant.original_until())
                {
                    return Err(AdmissionError::InvalidCaptureAdmission);
                }
            }
            Self::Recovery(source) => {
                if spec.deadline != Some(source.until)
                    || source.creation_taken.load(std::sync::atomic::Ordering::Acquire)
                {
                    return Err(AdmissionError::InvalidCaptureAdmission);
                }
            }
            Self::Custody { grant: None, .. } => {}
        }
        Ok(())
    }
    /// Observe the identical original source before effects; never renew admission time.
    pub(crate) fn check_locked(
        &self,
        core: &Arc<Core>,
        state: &State,
        now: Moment,
    ) -> Result<(), AdmissionError> {
        match self {
            Self::Custody { parent, grant } => {
                parent.validate_locked(core, state, now)?;
                if let Some(grant) = grant {
                    grant.state()?.validate_locked(core, state, now)?;
                }
            }
            Self::Recovery(source) => {
                source.validate_locked(core, state, source.key.run(), now)?;
            }
        }
        Ok(())
    }
    pub(crate) fn held(&self) -> bool {
        matches!(self, Self::Custody { grant: Some(_), .. })
    }
    /// Latch only after the original claims/input have transferred under Core's same guard.
    pub(crate) fn admitted(
        &self,
        id: sugarsubstitute_contracts::execution::JobId,
    ) -> Result<(), AdmissionError> {
        match self {
            Self::Custody { grant: Some(grant), .. } => {
                crate::scope::lock(&grant.state()?.stages).creation = Some(id)
            }
            Self::Recovery(source) => {
                source.creation_taken.store(true, std::sync::atomic::Ordering::Release)
            }
            Self::Custody { grant: None, .. } => {}
        }
        Ok(())
    }
}
