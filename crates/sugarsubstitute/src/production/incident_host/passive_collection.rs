// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe restored passive custody once without inventing file retirement.
use super::*;
impl IncidentHost {
    /// Deliver the restored scope once, retaining any rejected pure decision cause.
    pub(super) fn observe_passive_collection_restoration(&mut self) -> Result<(), HostIssue> {
        let controller = match self.completion.as_mut().map(|done| &mut done.outcome) {
            Some(JobOutcome::Success(value)) => value,
            _ => return Ok(()),
        };
        let policy = &mut controller.policy;
        let state = policy.state.as_mut().ok_or(HostIssue::Pending)?;
        if state.state.effects.iter().any(Option::is_some) {
            return Ok(());
        }
        let Some(scope) = policy.collection.take_passive_restoration_observation() else {
            return Ok(());
        };
        match state.state.coordinator.observe(IncidentServiceObservation::Failed {
            scope,
            failure: IncidentServiceFailure::Collection,
        }) {
            Ok(effects) => Self::retain_effects(&mut state.state, effects),
            Err(cause) => {
                self.cause.get_or_insert(HostCause::Decision(cause));
            }
        }
        Ok(())
    }
}
