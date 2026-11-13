// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Lend the actual funded policy through its authoritative source owner without native input.
use super::super::service::CollectionIssue;
use super::*;
/// A pure policy loan exercises real service completion routing, granting no native input or cleanup.
pub(in crate::production) struct PolicyProbe {
    source: Source,
    policy: Option<FundedState>,
}
impl Drop for PolicyProbe {
    fn drop(&mut self) {
        if let Some(policy) = self.policy.take() {
            self.source
                .shared
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .policy
                .return_issued(policy);
        }
    }
}

impl Source {
    /// Exercise the same policy owner without constructing input or executing capture/storage work.
    pub(in crate::production) fn policy_probe(
        &self,
        policy: &mut Option<FundedState>,
    ) -> Result<PolicyProbe, CollectionIssue> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.started || state.input.is_some() || state.operation.is_none() {
            return Err(CollectionIssue::Busy);
        }
        state.policy.install(policy)?;
        let policy = Some(state.issue_policy()?);
        Ok(PolicyProbe { source: self.alias(), policy })
    }
}
