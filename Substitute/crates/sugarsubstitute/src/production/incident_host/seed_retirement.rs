// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Accept exact failed-seed retirement from real worker and checked storage custody.
use super::*;
use crate::production::diagnostic_helper::RetiredRecoverySeed;
impl IncidentHost {
    /// Free only the matching stopped seed duty after physical cleanup, retaining all original causes.
    /// The new domain observation grants no commit, source deletion or family settlement.
    pub(in crate::production) fn retire_failed_seed_service(
        &mut self,
        scope: IncidentOperationScope,
    ) -> Result<RetiredRecoverySeed, HostIssue> {
        // Global close retains its existing close owner; this path isolates one live-Root run.
        if self.closing {
            return Err(HostIssue::Closed);
        }
        let controller = match self.completion.as_mut().map(|value| &mut value.outcome) {
            Some(JobOutcome::Success(controller)) => controller,
            _ => return Err(HostIssue::Unprepared),
        };
        let policy = &mut controller.policy;
        let state = policy.state.as_mut().ok_or(HostIssue::Pending)?;
        if state.state.effects.iter().any(Option::is_some) {
            return Err(HostIssue::Pending);
        }
        if !policy.seed.failed_retirement_matches(
            scope.operation(),
            scope.incident(),
            scope.expected_revision(),
        ) {
            return Err(HostIssue::Pending);
        }
        policy.seed.prepare_failed_retirement().map_err(|_| HostIssue::Seed)?;
        match state
            .state
            .coordinator
            .observe(IncidentServiceObservation::SeedPublicationRetired { scope })
        {
            Ok(effects) => {
                Self::retain_effects(&mut state.state, effects);
                policy.seed.take_failed_retirement().map_err(|_| HostIssue::Seed)
            }
            Err(cause) => {
                self.cause.get_or_insert(HostCause::Decision(cause));
                Err(HostIssue::Decision)
            }
        }
    }
}
