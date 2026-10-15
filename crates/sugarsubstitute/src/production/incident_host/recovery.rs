// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt only actual closed pending-store data under the current coordinator recovery scope.
use super::*;
use crate::production::diagnostic_helper::recovery::RecoveryService;
#[derive(Debug)]
pub(super) enum RecoveryAdoptionCause {
    Decision(IncidentDecisionError),
    Store(sugarsubstitute_crash::incident::IncidentStoreError),
    Identity,
    Capacity,
}
impl std::fmt::Display for RecoveryAdoptionCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Pending incident recovery adoption rejected")
    }
}
impl Error for RecoveryAdoptionCause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Decision(cause) => Some(cause),
            Self::Store(cause) => Some(cause),
            Self::Identity | Self::Capacity => None,
        }
    }
}
/// One admitted snapshot advances only after the preceding actual report is disposed.
pub(in crate::production) enum RecoveryProgress {
    Pending,
    Adopted(IncidentKey),
    Finished,
}
impl IncidentHost {
    pub(in crate::production) fn begin_recovery(
        &mut self,
        original: IncidentOperationBudget,
    ) -> Result<IncidentRecoveryScope, HostIssue> {
        let state = self.vacant()?;
        let effects = match state.coordinator.begin_recovery(original) {
            Ok(effects) => effects,
            Err(cause) => {
                self.cause.get_or_insert(HostCause::Decision(cause));
                return Err(HostIssue::Decision);
            }
        };
        let scope = state.coordinator.recovery_snapshot().ok_or(HostIssue::Decision)?.scope;
        Self::retain_effects(state, effects);
        Ok(scope)
    }
    pub(in crate::production) fn recovery_service(
        &mut self,
    ) -> Result<&mut RecoveryService, HostIssue> {
        if self.closing {
            return Err(HostIssue::Closed);
        }
        self.retained_recovery_service()
    }
    pub(in crate::production) fn retiring_recovery_service(
        &mut self,
    ) -> Result<&mut RecoveryService, HostIssue> {
        if !self.closing {
            return Err(HostIssue::Started);
        }
        self.retained_recovery_service()
    }
    fn retained_recovery_service(&mut self) -> Result<&mut RecoveryService, HostIssue> {
        match self.completion.as_mut().map(|done| &mut done.outcome) {
            Some(JobOutcome::Success(value)) => Ok(&mut value.policy.recovery),
            _ => Err(HostIssue::Unprepared),
        }
    }
    pub(in crate::production) fn capture_recovery(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<(), HostIssue> {
        match self.completion.as_mut().map(|done| &mut done.outcome) {
            Some(JobOutcome::Success(value)) => {
                value.policy.recovery.capture(batch).map_err(|_| HostIssue::Identity)
            }
            _ => Ok(()),
        }
    }
    /// Current checked reader/writer retirement covers only this recovered data snapshot.
    /// Historical family/capture flags stay false; no original operation is reconstructed.
    pub(in crate::production) fn adopt_recovery(
        &mut self,
        scope: IncidentRecoveryScope,
        cursor: usize,
        active: Option<IncidentKey>,
    ) -> Result<RecoveryProgress, HostIssue> {
        if self.closing {
            return Err(HostIssue::Closed);
        }
        let controller = match self.completion.as_mut().map(|done| &mut done.outcome) {
            Some(JobOutcome::Success(value)) => value,
            _ => return Err(HostIssue::Unprepared),
        };
        let policy = &mut controller.policy;
        if policy.recovery.retain_failure() {
            return Err(HostIssue::Preparation);
        }
        let Some(state) = policy.state.as_mut() else {
            return Ok(RecoveryProgress::Pending);
        };
        if state.state.effects.iter().any(Option::is_some) {
            return Ok(RecoveryProgress::Pending);
        }
        if let Some(key) = active {
            let previous = state.state.coordinator.snapshot(key).ok_or(HostIssue::Identity)?;
            if previous.acknowledgement.is_none() || !previous.reporter_retired {
                return Ok(RecoveryProgress::Pending);
            }
        }
        let applied = policy
            .recovery
            .inspect(|pending| -> Result<RecoveryProgress, RecoveryAdoptionCause> {
                if pending.operation() != scope.operation() {
                    return Err(RecoveryAdoptionCause::Identity);
                }
                if let Some(stored) =
                    pending.summaries().map_err(RecoveryAdoptionCause::Store)?.nth(cursor)
                {
                    let key = stored.summary().seed().key();
                    let effects = state
                        .state
                        .coordinator
                        .observe(IncidentServiceObservation::Recovered {
                            scope,
                            original_operation: None,
                            summary: stored.summary().clone(),
                        })
                        .map_err(RecoveryAdoptionCause::Decision)?;
                    for effect in effects {
                        match effect {
                            IncidentEffect::RetireSource {
                                scope: IncidentSourceScope::Recovered(source),
                            } if source.recovery() == scope
                                && source.historical_operation().is_none() =>
                            {
                                let retired = state
                                    .state
                                    .coordinator
                                    .observe(IncidentServiceObservation::SourcesRetired {
                                        scope: IncidentSourceScope::Recovered(source),
                                    })
                                    .map_err(RecoveryAdoptionCause::Decision)?;
                                for effect in retired {
                                    append(&mut state.state.effects, effect)?;
                                }
                            }
                            other => append(&mut state.state.effects, other)?,
                        }
                    }
                    return Ok(RecoveryProgress::Adopted(key));
                }
                let effects = state
                    .state
                    .coordinator
                    .observe(IncidentServiceObservation::RecoveryFinished { scope })
                    .map_err(RecoveryAdoptionCause::Decision)?;
                for effect in effects {
                    append(&mut state.state.effects, effect)?;
                }
                Ok(RecoveryProgress::Finished)
            })
            .map_err(|_| HostIssue::Preparation)?;
        let progress = match applied {
            Ok(progress) => progress,
            Err(cause) => {
                self.cause.get_or_insert(HostCause::Recovery(cause));
                return Err(HostIssue::Decision);
            }
        };
        if matches!(progress, RecoveryProgress::Finished) {
            policy.recovery.release_adopted().map_err(|_| HostIssue::Pending)?;
        }
        Ok(progress)
    }
}
fn append(
    slots: &mut [Option<IncidentEffect>; MAX_INCIDENT_EFFECTS],
    effect: IncidentEffect,
) -> Result<(), RecoveryAdoptionCause> {
    let slot =
        slots.iter_mut().find(|slot| slot.is_none()).ok_or(RecoveryAdoptionCause::Capacity)?;
    *slot = Some(effect);
    Ok(())
}
