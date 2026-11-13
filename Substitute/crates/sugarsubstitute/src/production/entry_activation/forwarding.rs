// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep actual producer replies through failed recipient delivery without replaying effects.
use super::{Result, retirement::RetiredDesktop};
use crate::production::{
    coordinator_backend::CoordinatorBackend, parent_lifecycle::recovery::MountedDocument,
};
use sugarsubstitute_contracts::lifecycle::{
    ClientBinding, GrantedResourceReplacement, ResourceKey, ResourceTransition, RunKey,
};

/// Producer-captured immutable Ready event data. This grants no current readiness or native privilege.
#[derive(Clone, Copy)]
struct ReadyContext {
    resource: ResourceKey,
    document: MountedDocument,
}
/// Fixed source-owned contexts for at most64 resources shared by at most256 replacement pairs.
pub(in crate::production) struct RecoveryContexts {
    values: [Option<ReadyContext>; 64],
}
impl RecoveryContexts {
    /// Borrow the original producer event for this exact successor generation.
    pub(in crate::production) fn document(
        &self,
        pair: GrantedResourceReplacement,
    ) -> std::result::Result<MountedDocument, crate::production::parent_lifecycle::ParentError>
    {
        self.values
            .iter()
            .flatten()
            .find(|value| value.resource == pair.current.resource)
            .map(|value| value.document)
            .ok_or(crate::production::parent_lifecycle::ParentError::WrongPhase)
    }
}

/// A whole returned transition remains charged by its source record until every
/// correlated recipient accepts it or its actual desktop retirement is observed.
pub(super) struct Forwarded {
    transition: ResourceTransition,
    delivered: [Option<RunKey>; 64],
    contexts: RecoveryContexts,
}
impl Forwarded {
    pub(super) fn new(transition: ResourceTransition) -> Self {
        Self {
            transition,
            delivered: [None; 64],
            contexts: RecoveryContexts { values: [None; 64] },
        }
    }
    /// Capture actual producer metadata before recipients accept context or another generation starts.
    /// Root gates new Acquire/RetryDead while this source or any native handoff remains pending.
    pub(super) fn capture_contexts(&mut self, backend: &CoordinatorBackend) -> Result<()> {
        self.capture_contexts_with(|pair| {
            let binding =
                ClientBinding { run: pair.previous.run, connection: pair.previous.connection };
            let metadata = match backend.invalidated_lease(binding,pair.current) {
                Ok(_) => {
                    match backend.invalidated_lease(binding,pair.previous) {
                        Err(cause) if cause.downcast_ref::<sugarsubstitute_contracts::lifecycle::LifecycleRejection>()
                            == Some(&sugarsubstitute_contracts::lifecycle::LifecycleRejection::StaleIdentity) => {},
                        Err(cause) => return Err(super::outer(cause)),
                        Ok(_) => return Err("Old replacement outage still retained after actual successor death".into()),
                    }
                    backend.historical_replacement_metadata(binding,pair.current).map_err(super::outer)?
                }
                Err(cause) if cause.downcast_ref::<sugarsubstitute_contracts::lifecycle::LifecycleRejection>()
                    == Some(&sugarsubstitute_contracts::lifecycle::LifecycleRejection::StaleIdentity) => {
                    backend.replacement_context(binding,pair.previous,pair.current).map_err(super::outer)?.map(|context| context.metadata())
                }
                Err(cause) => return Err(super::outer(cause)),
            };
            let Some(metadata) = metadata else { return Ok(None) };
            Ok(Some(ReadyContext {
                resource: metadata.resource(),
                document: MountedDocument::new(
                    metadata.origin(),
                    metadata.frontend_version()?,
                    metadata.sugarcubes_version()?,
                ).map_err(super::outer)?,
            }))
        })
    }
    /// Fake only immutable external metadata in owner tests; raw transition and capture remain here.
    fn capture_contexts_with(
        &mut self,
        mut source: impl FnMut(GrantedResourceReplacement) -> Result<Option<ReadyContext>>,
    ) -> Result<()> {
        for pair in &self.transition.replacements {
            if self
                .contexts
                .values
                .iter()
                .flatten()
                .any(|value| value.resource == pair.current.resource)
            {
                continue;
            }
            let context = source(*pair)?.ok_or("Genuine actual Ready metadata unavailable")?;
            if context.resource != pair.current.resource {
                return Err("Foreign actual Ready metadata".into());
            }
            let at = self
                .contexts
                .values
                .iter()
                .position(Option::is_none)
                .ok_or("Actual Ready resource context bound occupied")?;
            self.contexts.values[at] = Some(context);
        }
        Ok(())
    }
    /// Invoke the real recipient only once after acceptance. A rejected recipient
    /// leaves both the complete original transition and its remaining work owned.
    pub(super) fn deliver(
        &mut self,
        run: RunKey,
        receive: impl FnOnce(&ResourceTransition, &RecoveryContexts) -> Result<()>,
    ) -> Result<()> {
        if self.delivered.contains(&Some(run))
            || !self.recipients().any(|recipient| recipient == run)
        {
            return Ok(());
        }
        let at = self
            .delivered
            .iter()
            .position(Option::is_none)
            .ok_or("Forwarded recipient bound occupied")?;
        if self.transition.replacements.iter().any(|pair| self.contexts.document(*pair).is_err()) {
            return Err("Original replacement context remains uncaptured".into());
        }
        match receive(&self.transition, &self.contexts) {
            Ok(()) => self.delivered[at] = Some(run),
            Err(cause)
                if cause.downcast_ref::<crate::production::parent_lifecycle::ParentError>()
                    == Some(
                        &crate::production::parent_lifecycle::ParentError::TransitionPending,
                    ) => {}
            Err(cause) => return Err(cause),
        }
        Ok(())
    }
    /// Actual same-run native retirement discharges only the unreachable reply.
    pub(super) fn retire(&mut self, retired: &RetiredDesktop) -> Result<()> {
        self.deliver(retired.run(), |_, _| Ok(()))
    }
    pub(super) fn complete(&self) -> bool {
        self.transition.replacements.iter().all(|pair| self.contexts.document(*pair).is_ok())
            && self.recipients().all(|run| self.delivered.contains(&Some(run)))
    }
    /// Discover every correlated recipient without another set, allocation or queue.
    fn recipients(&self) -> impl Iterator<Item = RunKey> + '_ {
        self.transition
            .replies
            .iter()
            .map(|reply| reply.run)
            .chain(self.transition.invalidations.iter().map(|notice| notice.lease.run))
            .chain(self.transition.replacements.iter().map(|pair| pair.previous.run))
    }
}

#[cfg(test)]
#[path = "forwarding_tests.rs"]
mod tests;
