// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one mounted Main's actual backend lineage independently of its creation authorization.
use super::*;
use sugarsubstitute_contracts::{
    configuration::LoopbackOrigin,
    lifecycle::{GrantedResourceInvalidation, GrantedResourceReplacement},
    startup::MainIdentity,
};
use sugarsubstitute_supervision::lifecycle_protocol::{VersionLabel, WireHeader};
#[path = "recovery/closing.rs"]
mod closing;
#[path = "recovery/output.rs"]
mod output;
#[path = "recovery/requests.rs"]
mod requests;
#[cfg(test)]
#[path = "recovery/tests.rs"]
mod tests;
#[path = "recovery/transitions.rs"]
mod transitions;

/// Original document selection, copied only after actual verified readiness.
/// Fixed arrays preserve both installed version labels without another allocation.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(in crate::production) struct MountedDocument {
    origin: LoopbackOrigin,
    frontend: [u8; 64],
    frontend_bytes: u8,
    sugarcubes: [u8; 64],
    sugarcubes_bytes: u8,
}
impl MountedDocument {
    /// Retain the exact actual origin and the existing version grammar before exposing Grant.
    pub(in crate::production) fn new(
        origin: LoopbackOrigin,
        frontend: &str,
        sugarcubes: &str,
    ) -> Result<Self> {
        let frontend_label = VersionLabel::new(frontend)?;
        let sugarcubes_label = VersionLabel::new(sugarcubes)?;
        let frontend = frontend_label.as_str().as_bytes();
        let sugarcubes = sugarcubes_label.as_str().as_bytes();
        let mut value = Self {
            origin,
            frontend: [0; 64],
            frontend_bytes: frontend.len() as u8,
            sugarcubes: [0; 64],
            sugarcubes_bytes: sugarcubes.len() as u8,
        };
        value.frontend[..frontend.len()].copy_from_slice(frontend);
        value.sugarcubes[..sugarcubes.len()].copy_from_slice(sugarcubes);
        Ok(value)
    }
}
/// The changing backend interest; MainAuthorization and Acquisition keep their original lease.
#[derive(Clone, Copy)]
pub(super) struct RunningBackendBinding {
    pub(super) lease: LeaseKey,
    pub(super) document: MountedDocument,
}
/// One actual invalidation under this mounted Main's current complete binding.
#[derive(Clone, Copy)]
pub(super) struct Outage {
    pub(super) failed: LeaseKey,
    pub(super) status: u32,
}
/// Actual emitted replacement data remains owned until the child consumes its new binding.
#[derive(Clone, Copy)]
pub(super) struct Handoff {
    pub(super) pair: GrantedResourceReplacement,
    pub(super) header: Option<WireHeader>,
    pub(super) written: bool,
    pub(super) next: Option<GrantedResourceInvalidation>,
}
/// One explicit native user request, retaining its original finite request/campaign cutoff.
#[derive(Clone, Copy)]
pub(super) struct Retry {
    pub(super) header: WireHeader,
    pub(super) failed: LeaseKey,
    pub(super) until: Moment,
    pub(super) successor: Option<LeaseKey>,
    pub(super) outcome: Option<sugarsubstitute_contracts::lifecycle::RequestOutcome>,
    pub(super) expired: bool,
}
/// These fixed slots store only this one Main's checked lineage, never a second resource policy.
#[derive(Clone, Copy, Default)]
pub(super) struct Recovery {
    pub(super) running: Option<RunningBackendBinding>,
    pub(super) outage: Option<Outage>,
    pub(super) handoff: Option<Handoff>,
    pub(super) retry: Option<Retry>,
}
impl Recovery {
    /// Preserve the genuine initial Grant selection without altering later Main authorization.
    pub(super) fn bind_initial(
        &mut self,
        lease: LeaseKey,
        document: MountedDocument,
    ) -> Result<()> {
        if self.running.is_some() || self.outage.is_some() || self.handoff.is_some() {
            return Err(ParentError::WrongPhase.into());
        }
        self.running = Some(RunningBackendBinding { lease, document });
        Ok(())
    }
    /// Preflight a whole transition before any local request or lineage slot changes.
    pub(super) fn check_invalidation(&self, notice: GrantedResourceInvalidation) -> Result<()> {
        if let Some(handoff) = self.handoff
            && handoff.pair.current == notice.lease
        {
            if handoff.next.is_some_and(|old| old != notice) {
                return Err(ParentError::ChangedDuplicate.into());
            }
            return Ok(());
        }
        let running = self.running.ok_or(ParentError::WrongPhase)?;
        if running.lease != notice.lease {
            return Err(ParentError::ForeignPeer.into());
        }
        if self.outage.is_some_and(|old| old.failed != notice.lease || old.status != notice.status)
        {
            return Err(ParentError::ChangedDuplicate.into());
        }
        Ok(())
    }
    /// Retain exact actual death, including a successor death racing its pending binding reply.
    pub(super) fn invalidate(
        &mut self,
        notice: GrantedResourceInvalidation,
        affected_clients: u16,
    ) -> Result<()> {
        if !(1..=64).contains(&affected_clients) {
            return Err(ParentError::WrongPhase.into());
        }
        self.check_invalidation(notice)?;
        if let Some(handoff) = &mut self.handoff
            && handoff.pair.current == notice.lease
        {
            handoff.next = Some(notice);
            return Ok(());
        }
        self.outage = Some(Outage { failed: notice.lease, status: notice.status });
        Ok(())
    }
    /// Validate the actual App pair against the original full native binding.
    pub(super) fn check_replacement(&self, pair: GrantedResourceReplacement) -> Result<()> {
        let outage = self.outage.ok_or(ParentError::WrongPhase)?;
        if outage.failed != pair.previous
            || pair.previous.run != pair.current.run
            || pair.previous.connection != pair.current.connection
            || pair.previous.resource.owner != pair.current.resource.owner
            || pair.previous.resource.id != pair.current.resource.id
            || pair.previous.resource.generation == pair.current.resource.generation
            || pair.previous.id == pair.current.id
        {
            return Err(ParentError::ForeignPeer.into());
        }
        if self.handoff.is_some_and(|old| old.pair != pair) {
            return Err(ParentError::TransitionPending.into());
        }
        Ok(())
    }
    /// Keep the genuine actual Ready event before any native reply issue or child adoption.
    pub(super) fn replace(&mut self, pair: GrantedResourceReplacement) -> Result<()> {
        self.check_replacement(pair)?;
        self.handoff.get_or_insert(Handoff { pair, header: None, written: false, next: None });
        Ok(())
    }
    /// Select one immutable pending header; another request cannot receive a second handoff.
    pub(super) fn stage(&mut self, header: WireHeader) -> Result<GrantedResourceReplacement> {
        let handoff = self.handoff.as_mut().ok_or(ParentError::WrongPhase)?;
        if handoff.header.is_some_and(|old| old != header) {
            return Err(ParentError::WrongPhase.into());
        }
        handoff.header = Some(header);
        Ok(handoff.pair)
    }
    /// Full actual Written proves only delivery, never App adoption or backend readiness.
    pub(super) fn written(&mut self, header: WireHeader) -> Result<()> {
        let handoff = self.handoff.as_mut().ok_or(ParentError::WrongPhase)?;
        if handoff.header != Some(header) {
            return Err(ParentError::ForeignPeer.into());
        }
        handoff.written = true;
        Ok(())
    }
    /// Match a genuine new OutputPull after the authenticated child consumed the exact handoff.
    pub(super) fn consumed_pair(&self, current: LeaseKey) -> Result<GrantedResourceReplacement> {
        let handoff = self.handoff.ok_or(ParentError::WrongPhase)?;
        if !handoff.written || handoff.pair.current != current {
            return Err(ParentError::ForeignPeer.into());
        }
        Ok(handoff.pair)
    }
    /// Consume only the native binding ledger after the external App/live-or-dead check.
    /// The callback runs after every local check; rejection preserves all original slots.
    pub(super) fn consume_binding(
        &mut self,
        current: LeaseKey,
        acknowledged: Option<std::num::NonZeroU64>,
        validate: impl FnOnce(
            GrantedResourceReplacement,
            Option<GrantedResourceInvalidation>,
        ) -> Result<()>,
    ) -> Result<()> {
        let pair = self.consumed_pair(current)?;
        if acknowledged.is_some() {
            return Err(ParentError::WrongPhase.into());
        }
        let handoff = self.handoff.ok_or(ParentError::WrongPhase)?;
        let running = self.running.ok_or(ParentError::WrongPhase)?;
        validate(pair, handoff.next)?;
        self.running =
            Some(RunningBackendBinding { lease: pair.current, document: running.document });
        self.outage =
            handoff.next.map(|notice| Outage { failed: notice.lease, status: notice.status });
        self.handoff = None;
        self.retry = None;
        Ok(())
    }
    /// Authenticate release of an already invalidated interest without granting live authority.
    /// The caller must also check the authoritative backend outage before acknowledging it.
    pub(super) fn invalidated_release(
        &self,
        reference: sugarsubstitute_supervision::lifecycle_protocol::LeaseReference,
    ) -> Result<Option<LeaseKey>> {
        let Some(outage) = self.outage else { return Ok(None) };
        if super::replies::reference(outage.failed) != reference {
            return Err(ParentError::ForeignPeer.into());
        }
        Ok(Some(outage.failed))
    }
    /// Preserve only a current healthy running lease as explicit Release/Reload authority.
    pub(super) fn live_lease(&self) -> Option<LeaseKey> {
        self.outage.is_none().then(|| self.running.map(|value| value.lease)).flatten()
    }
    /// Admit reload only for the exact healthy current backend interest.
    /// Completed handoffs preserve the mounted document; stale and pending interests cannot act.
    pub(super) fn reload<T>(
        &self,
        reference: sugarsubstitute_supervision::lifecycle_protocol::LeaseReference,
        effect: impl FnOnce(LeaseKey) -> Result<T>,
    ) -> Result<T> {
        let lease = self.live_lease().ok_or(ParentError::WrongPhase)?;
        if super::replies::reference(lease) != reference {
            return Err(ParentError::ForeignPeer.into());
        }
        effect(lease)
    }
    /// Validate the exact mounted current Main without recreating a browser/profile or authorization.
    pub(super) fn main_identity(owner: &ParentLifecycle, identity: MainIdentity) -> Result<()> {
        if owner.closed
            || owner.cleanup.is_some()
            || owner.attempt_cleanup.is_some()
            || owner.main.snapshot().identity != Some(identity)
            || owner.main.stage()
                != sugarsubstitute_application::supervision::MainAuthorizationStage::Authorized
            || owner.recovery.running.is_none()
        {
            return Err(ParentError::WrongPhase.into());
        }
        Ok(())
    }
}

/// Authenticate immutable producer history and original source without manufacturing current Ready.
/// A stale current key alone is insufficient: the live branch independently validates the actual pair;
/// the dead branch first authenticates B and requires absent A before its genuine historical record.
pub(in crate::production::parent_lifecycle) fn replacement_document(
    binding: sugarsubstitute_contracts::lifecycle::ClientBinding,
    pair: GrantedResourceReplacement,
    resources: &ParentResources<'_, '_>,
) -> Result<MountedDocument> {
    use sugarsubstitute_contracts::lifecycle::LifecycleRejection;
    let source = &resources.preparation.as_ref().ok_or(ParentError::WrongPhase)?.source;
    let context = match resources.backend.invalidated_lease(binding, pair.current) {
        Ok(_) => {
            match resources.backend.invalidated_lease(binding, pair.previous) {
                Err(cause)
                    if cause.downcast_ref::<LifecycleRejection>()
                        == Some(&LifecycleRejection::StaleIdentity) => {}
                Err(cause) => return Err(cause),
                Ok(_) => return Err(ParentError::WrongPhase.into()),
            }
            resources.backend.historical_replacement_context(binding, pair.current, source)?
        }
        Err(cause)
            if cause.downcast_ref::<LifecycleRejection>()
                == Some(&LifecycleRejection::StaleIdentity) =>
        {
            resources.backend.replacement_context(binding, pair.previous, pair.current)?
        }
        Err(cause) => return Err(cause),
    }
    .ok_or(ParentError::WrongPhase)?;
    if !context.matches_selection(source) {
        return Err(ParentError::ProfileMismatch.into());
    }
    let metadata = context.metadata();
    if metadata.resource() != pair.current.resource {
        return Err(ParentError::ForeignPeer.into());
    }
    MountedDocument::new(
        metadata.origin(),
        metadata.frontend_version()?,
        metadata.sugarcubes_version()?,
    )
}
