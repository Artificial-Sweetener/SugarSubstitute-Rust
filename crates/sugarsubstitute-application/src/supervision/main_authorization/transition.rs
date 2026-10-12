// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Apply checked request ordering once; trusted continuations keep their real request ID.
use super::{
    Fingerprint, MainAuthorizationEffect as Effect, MainAuthorizationError,
    MainAuthorizationObservation as Observation, MainAuthorizationSession,
    MainAuthorizationStage as Stage, history::Progress,
};
use sugarsubstitute_contracts::lifecycle::{LifecycleRejection as Rejection, RequestId};

impl MainAuthorizationSession {
    pub(super) fn transition(
        &mut self,
        request: RequestId,
        observation: &Observation,
    ) -> Result<Option<Effect>, MainAuthorizationError> {
        let identity = observation.identity();
        let fingerprint = match observation {
            Observation::RequestedFolderAccepted { next_offset_units, .. } => {
                Fingerprint::Folder(*next_offset_units)
            }
            Observation::EnvironmentFolderComplete { .. } => Fingerprint::Environment,
            Observation::ControllerObserved { browser_process_id, .. } => {
                Fingerprint::Controller(*browser_process_id)
            }
            Observation::MainRetired { .. } => Fingerprint::Retired,
            Observation::Failed { failure, .. } => {
                if self.failure_seen || self.stage == Stage::Retired {
                    return Err(Rejection::WrongPhase.into());
                }
                if !self.history.contains(request) {
                    self.history.check(request, Fingerprint::Failure(failure.code()))?;
                    self.history.record(request, Fingerprint::Failure(failure.code()));
                }
                if self.stage != Stage::Retiring {
                    self.stage = Stage::Failed;
                }
                return Ok(None);
            }
            _ => return self.continue_request(request, observation),
        };
        if self.history.check(request, fingerprint)? {
            return Ok(None);
        }
        let effect = match observation {
            Observation::RequestedFolderAccepted { next_offset_units: offset, .. } => {
                if self.stage != Stage::RequestedFolder
                    || *offset == 0
                    || *offset > 32766
                    || *offset <= self.folder_offset
                    || (self.folder_offset == 0 && *offset > 16384)
                    || (self.folder_offset != 0 && self.folder_offset != 16384)
                {
                    return Err(Rejection::WrongPhase.into());
                }
                self.folder_offset = *offset;
                None
            }
            Observation::EnvironmentFolderComplete { .. } => {
                if self.stage != Stage::RequestedFolder || self.folder_offset == 0 {
                    return Err(Rejection::WrongPhase.into());
                }
                self.stage = Stage::Environment;
                self.environment_request = Some(request);
                Some(Effect::ValidateEnvironment(identity))
            }
            Observation::ControllerObserved { browser_process_id, .. } => {
                if self.stage != Stage::Environment
                    || self.environment_request.is_none_or(|request| {
                        self.history.progress(request, Fingerprint::Environment)
                            != Some(Progress::Issued)
                    })
                {
                    return Err(Rejection::WrongPhase.into());
                }
                self.stage = Stage::Browser;
                self.browser_pid = Some(*browser_process_id);
                self.browser_request = Some(request);
                Some(Effect::ValidateBrowser { identity, browser_process_id: *browser_process_id })
            }
            Observation::MainRetired { .. } => {
                if self.stage != Stage::Retiring {
                    return Err(Rejection::WrongPhase.into());
                }
                self.retirement_observed = true;
                Some(Effect::AcknowledgeRetirement(identity))
            }
            _ => return Err(Rejection::WrongPhase.into()),
        };
        self.history.record(request, fingerprint);
        Ok(effect)
    }

    fn continue_request(
        &mut self,
        request: RequestId,
        observation: &Observation,
    ) -> Result<Option<Effect>, MainAuthorizationError> {
        let identity = observation.identity();
        let (fingerprint, expected_request, next, effect) = match observation {
            Observation::EnvironmentVerified { .. } => (
                Fingerprint::Environment,
                self.environment_request,
                Progress::Verified,
                Some(Effect::AuthorizeEnvironment(identity)),
            ),
            Observation::EnvironmentAuthorizationIssued { .. } => {
                (Fingerprint::Environment, self.environment_request, Progress::Issued, None)
            }
            Observation::BrowserVerified { browser_process_id, .. } => (
                Fingerprint::Controller(*browser_process_id),
                self.browser_request,
                Progress::Verified,
                Some(Effect::AuthorizeBrowser {
                    identity,
                    browser_process_id: *browser_process_id,
                }),
            ),
            Observation::BrowserAuthorizationIssued { browser_process_id, .. } => (
                Fingerprint::Controller(*browser_process_id),
                self.browser_request,
                Progress::Issued,
                None,
            ),
            _ => return Err(Rejection::WrongPhase.into()),
        };
        if expected_request != Some(request) {
            return Err(Rejection::StaleRequest.into());
        }
        let progress =
            self.history.progress(request, fingerprint).ok_or(Rejection::ChangedDuplicate)?;
        if progress >= next {
            return Ok(None);
        }
        if next == Progress::Issued && progress != Progress::Verified {
            return Err(Rejection::WrongPhase.into());
        }
        self.history.advance(request, next);
        if matches!(observation, Observation::BrowserAuthorizationIssued { .. }) {
            self.stage = Stage::Authorized;
        }
        Ok(effect)
    }
}
