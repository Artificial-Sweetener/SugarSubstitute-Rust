// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Accept complete actual outage/replacement events transactionally before routing request replies.
use super::*;
use crate::production::entry_activation::forwarding::RecoveryContexts;
use sugarsubstitute_contracts::lifecycle::ResourceTransition;
impl Recovery {
    /// Every context/identity/count check precedes committing any native binding-ledger slot.
    /// The external queries borrow actual producer state; they manufacture no Ready or App adoption.
    pub(in crate::production::parent_lifecycle) fn transition(
        self,
        run: RunKey,
        transition: &ResourceTransition,
        mut document: impl FnMut(GrantedResourceReplacement) -> Result<MountedDocument>,
        mut count: impl FnMut(LeaseKey) -> Result<u16>,
    ) -> Result<Self> {
        let mut next = self;
        // A transition may contain both A's death and B's genuine Ready event followed by B's death.
        // Process the original mounted binding first, then the historical pair, then its successor notice.
        for notice in transition.invalidations.iter().filter(|notice| notice.lease.run == run) {
            if next.running.is_some_and(|running| running.lease == notice.lease) {
                next.invalidate(*notice, count(notice.lease)?)?;
            }
        }
        for pair in transition.replacements.iter().filter(|pair| pair.previous.run == run) {
            next.check_replacement(*pair)?;
            if next.running.ok_or(ParentError::WrongPhase)?.document != document(*pair)? {
                return Err(ParentError::ProfileMismatch.into());
            }
            next.replace(*pair)?;
        }
        for notice in transition.invalidations.iter().filter(|notice| notice.lease.run == run) {
            if next.running.is_none_or(|running| running.lease != notice.lease) {
                next.invalidate(*notice, count(notice.lease)?)?;
            }
        }
        Ok(next)
    }
}
impl ParentLifecycle {
    /// Compare producer-captured fixed metadata and the exact original installed selection.
    pub(in crate::production::parent_lifecycle) fn checked_recovery_transition(
        &self,
        transition: &ResourceTransition,
        contexts: Option<&RecoveryContexts>,
        resources: &ParentResources<'_, '_>,
    ) -> Result<Recovery> {
        self.recovery.transition(
            self.run,
            transition,
            |pair| {
                let document = super::replacement_document(self.binding()?, pair, resources)?;
                if let Some(contexts) = contexts
                    && contexts.document(pair)? != document
                {
                    return Err(ParentError::ProfileMismatch.into());
                }
                Ok(document)
            },
            |failed| resources.backend.affected_clients(self.binding()?, failed),
        )
    }
}
