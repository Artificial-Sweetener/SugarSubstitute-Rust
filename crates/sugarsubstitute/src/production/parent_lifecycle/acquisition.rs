// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the current acquisition through its attempt lifetime, independently of replay history.
use super::{ParentError, Reply};
use sugarsubstitute_contracts::{
    execution::Moment,
    lifecycle::{ClientBinding, LeaseKey, RequestId, RequestOutcome},
    startup::MainIdentity,
};
use sugarsubstitute_supervision::lifecycle_protocol::WireHeader;

#[path = "acquisition/write_state.rs"]
mod write_state;
pub(super) use write_state::WriteObservation;

/// One original acquisition's copied correlation. Native lease, readiness, write and
/// settlement authority remain with their actual owners; these facts manufacture none.
pub(super) struct Acquisition {
    pub(super) header: WireHeader,
    pub(super) original: Moment,
    kind: sugarsubstitute_application::startup::ConnectionBudgetKind,
    progress_until: Option<Moment>,
    original_identity: MainIdentity,
    identity: MainIdentity,
    lease: Option<LeaseKey>,
    outcome: Option<RequestOutcome>,
    disposition: Option<Disposition>,
    sent: bool,
    progress: Progress,
    final_write: write_state::OriginalWrite,
    retiring: bool,
    retirement: Option<(WireHeader, bool)>,
    write_cutoff: Option<sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeError>,
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum Progress {
    Unpublished,
    Queued,
    Written,
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum Disposition {
    Granted,
    Failed(sugarsubstitute_contracts::lifecycle::LifecycleFailureCode),
    Rejected(sugarsubstitute_contracts::lifecycle::LifecycleRejection),
    Cancelled,
}
impl Acquisition {
    /// Clamp only attached provisional pulls to their aggregate connection deadline.
    pub(super) fn output_until(&self, supplied: Moment) -> Moment {
        if self.kind.waits_for_owned_process() { supplied } else { supplied.min(self.original) }
    }
    /// Freeze one finite progress transfer when genuine data first becomes available.
    pub(super) fn progress_deadline(
        &mut self,
        now: Moment,
    ) -> Result<Moment, sugarsubstitute_contracts::execution::TimeError> {
        if let Some(until) = self.progress_until {
            return Ok(until);
        }
        let until = self.output_until(now.checked_add(std::time::Duration::from_secs(30))?);
        self.progress_until = Some(until);
        Ok(until)
    }
    /// Freeze the accepted Acquire before backend effects; no IO or allocation occurs.
    pub(super) fn new(
        binding: ClientBinding,
        header: WireHeader,
        identity: MainIdentity,
        original: Moment,
        kind: sugarsubstitute_application::startup::ConnectionBudgetKind,
    ) -> Result<Self, ParentError> {
        if header.run != binding.run
            || header.connection != Some(binding.connection)
            || identity.run != binding.run
            || header.request.is_none()
        {
            return Err(ParentError::ForeignPeer);
        }
        Ok(Self {
            header,
            original,
            kind,
            progress_until: None,
            original_identity: identity,
            identity,
            lease: None,
            outcome: None,
            disposition: None,
            sent: false,
            progress: Progress::Unpublished,
            final_write: write_state::OriginalWrite::new(),
            retiring: false,
            retirement: None,
            write_cutoff: None,
        })
    }
    /// Retain only this original backend producer's outcome before fallible local routing.
    /// Other request continuations remain with the replay owner.
    pub(super) fn observe_backend(
        &mut self,
        header: WireHeader,
        outcome: RequestOutcome,
    ) -> Result<(), ParentError> {
        if header.request != self.header.request {
            return Ok(());
        }
        if header != self.header {
            return Err(ParentError::ForeignPeer);
        }
        if self.outcome.is_some_and(|old| old != outcome)
            && !matches!(outcome, RequestOutcome::Pending(_))
        {
            return Err(ParentError::ChangedDuplicate);
        }
        if let RequestOutcome::Pending(lease) | RequestOutcome::Granted(lease) = outcome {
            if lease.run != self.header.run
                || Some(lease.connection) != self.header.connection
                || lease.resource.owner != lease.run.owner
                || self.lease.is_some_and(|old| old != lease)
                || (self.disposition.is_some() && self.lease != Some(lease))
            {
                return Err(ParentError::ForeignPeer);
            }
            self.lease = Some(lease);
        }
        if matches!(outcome, RequestOutcome::Pending(_)) {
            return Ok(());
        }
        self.outcome = Some(outcome);
        Ok(())
    }
    /// Retain incoming cancellation until the genuine final Grant write is checked.
    pub(super) fn awaiting_grant_write(&self) -> bool {
        self.disposition == Some(Disposition::Granted) && self.final_write.pending(self.retiring)
    }
    /// Borrow the authentic provisional source once; this is never a final disposition.
    pub(super) fn progress(&self) -> Option<LeaseKey> {
        (!self.retiring && self.progress == Progress::Unpublished && self.disposition.is_none())
            .then_some(self.lease)
            .flatten()
    }
    /// Record actual native queue acceptance under the immutable original header.
    pub(super) fn progress_queued(&mut self, header: WireHeader) -> Result<(), ParentError> {
        if header != self.header || self.progress != Progress::Unpublished || self.lease.is_none() {
            return Err(ParentError::WrongPhase);
        }
        self.progress = Progress::Queued;
        Ok(())
    }
    /// Called only after the actual write receipt validates completion or revocation.
    /// Neither outcome consumes the Acquire replay slot or publishes final readiness.
    pub(super) fn progress_finished(
        &mut self,
        header: WireHeader,
        written: bool,
    ) -> Result<(), ParentError> {
        if header != self.header || self.progress != Progress::Queued {
            return Err(ParentError::WrongPhase);
        }
        self.progress = if written { Progress::Written } else { Progress::Unpublished };
        Ok(())
    }
    /// Match data-only output access to the observed producer and current registered binding.
    pub(super) fn output_lease(
        &self,
        binding: ClientBinding,
        reference: sugarsubstitute_supervision::lifecycle_protocol::LeaseReference,
    ) -> Result<LeaseKey, ParentError> {
        let lease = self.lease.ok_or(ParentError::WrongPhase)?;
        if self.header.run != binding.run
            || self.header.connection != Some(binding.connection)
            || lease.run != binding.run
            || lease.connection != binding.connection
            || super::replies::reference(lease) != reference
        {
            return Err(ParentError::ForeignPeer);
        }
        if self.progress != Progress::Written
            && !(self.sent && self.disposition == Some(Disposition::Granted))
        {
            return Err(ParentError::WrongPhase);
        }
        Ok(lease)
    }
    /// Retain a final reply only after its existing semantic owner accepted the producer.
    pub(super) fn reply(&mut self, header: WireHeader, reply: Reply) -> Result<(), ParentError> {
        if header.request != self.header.request {
            return Ok(());
        }
        if header != self.header {
            return Err(ParentError::ForeignPeer);
        }
        let disposition = match reply {
            Reply::Granted { lease, identity }
                if Some(lease) == self.lease
                    && identity == self.original_identity
                    && self.outcome == Some(RequestOutcome::Granted(lease)) =>
            {
                Disposition::Granted
            }
            Reply::Failed(code) => Disposition::Failed(code),
            Reply::Rejected(code) => Disposition::Rejected(code),
            Reply::Cancelled => Disposition::Cancelled,
            _ => return Err(ParentError::WrongPhase),
        };
        if self.disposition.is_some_and(|old| old != disposition) {
            return Err(ParentError::ChangedDuplicate);
        }
        self.disposition = Some(disposition);
        Ok(())
    }
    /// Latch only the same final request after the actual write receipt completed.
    pub(super) fn observe_write(
        &mut self,
        header: WireHeader,
        observation: WriteObservation,
    ) -> Result<(), ParentError> {
        if header.request == self.header.request {
            if header != self.header {
                return Err(ParentError::ForeignPeer);
            }
            if self.disposition.is_none() {
                return Err(ParentError::WrongPhase);
            }
            if !self.final_write.written() {
                self.final_write.observe(observation)?;
            }
            self.sent = self.final_write.written();
        }
        if observation == WriteObservation::Written
            && let Some((original, written)) = &mut self.retirement
            && *original == header
        {
            *written = true;
        }
        Ok(())
    }

    /// Fence only an unfinished original acquisition at its unchanged cutoff.
    /// A timely written Grant owns the running lease beyond acquisition expiry.
    pub(super) fn expire(&mut self, now: Moment) -> bool {
        if self.kind.waits_for_owned_process()
            || now < self.original
            || (!self.retiring && self.disposition == Some(Disposition::Granted) && self.sent)
        {
            return false;
        }
        self.retiring = true;
        self.final_write.withdraw_unqueued();
        true
    }

    /// Closed semantic retirement does not dispose any native write or source loan.
    pub(super) const fn retiring(&self) -> bool {
        self.retiring
    }

    /// Retained queue/issue custody prevents classifying another operation as no-issue.
    pub(super) fn write_active(&self) -> bool {
        self.progress == Progress::Queued || !self.final_write.closed()
    }

    /// Retain the exact first direct rejection of an original never-issued write.
    /// Actual returned allocation/correlation is validated by the containing sender.
    pub(super) fn retain_write_cutoff(
        &mut self,
        cause: sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeError,
    ) -> Result<(), ParentError> {
        if !self.retiring || cause != sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeError::DeadlineExpired {
            return Err(ParentError::WrongPhase);
        }
        self.write_cutoff.get_or_insert(cause);
        Ok(())
    }

    /// Borrow the original direct cutoff cause; no global failure is inferred.
    #[cfg(test)]
    pub(super) const fn write_cutoff(
        &self,
    ) -> Option<sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeError> {
        self.write_cutoff
    }

    /// Retain a genuine native negative/retired observation under this exact attempt.
    /// The caller authenticates its creating-thread producer and channel before this call.
    pub(super) fn observe_retirement(
        &mut self,
        binding: ClientBinding,
        identity: MainIdentity,
        header: WireHeader,
    ) -> Result<(), ParentError> {
        if binding.run != self.header.run
            || Some(binding.connection) != self.header.connection
            || header.run != binding.run
            || header.connection != Some(binding.connection)
            || identity != self.identity
            || header.request.is_none()
            || header.request == self.header.request
        {
            return Err(ParentError::ForeignPeer);
        }
        if self.retirement.is_some_and(|(old, _)| old != header) {
            return Err(ParentError::ChangedDuplicate);
        }
        self.retirement.get_or_insert((header, false));
        Ok(())
    }

    /// Borrow the exact accepted native retirement correlation for closed diagnostics.
    /// Matching metadata neither admits a reply nor establishes native retirement.
    pub(super) fn retirement_header(&self) -> Option<WireHeader> {
        self.retirement.map(|(header, _)| header)
    }

    /// Only a checked original native-retirement ACK permits expired semantic cleanup.
    pub(super) fn retirement_written(&self) -> bool {
        self.retirement.is_some_and(|(_, written)| written)
    }
    /// Match the exact original request and current attempt without replaying its effects.
    /// Completed acquisition expiry never renews the original or cleanup budget.
    pub(super) fn settlement_lease(
        &self,
        binding: ClientBinding,
        identity: MainIdentity,
        acquisition: RequestId,
        current_lease: Option<LeaseKey>,
        main_identity: Option<MainIdentity>,
    ) -> Result<Option<LeaseKey>, ParentError> {
        if self.header.request != Some(acquisition) {
            return Err(ParentError::StaleRequest);
        }
        if self.header.run != binding.run
            || self.header.connection != Some(binding.connection)
            || self.identity != identity
            || identity.run != binding.run
            || self.original_identity.run != binding.run
            || current_lease.is_some_and(|lease| self.lease != Some(lease))
        {
            return Err(ParentError::ForeignPeer);
        }
        let old_main_retired = main_identity
            .is_none_or(|old| old.run == identity.run && old.launch.get() < identity.launch.get());
        let provisional = self.disposition != Some(Disposition::Granted)
            && self.progress == Progress::Written
            && self.lease.is_some()
            && current_lease.is_none()
            && old_main_retired;
        let retired_original = self.retiring && self.retirement_written();
        if ((!self.sent || self.disposition.is_none()) && !provisional && !retired_original)
            || (self.retiring && !self.retirement_written())
            || !self.final_write.closed()
            || (self.disposition == Some(Disposition::Granted)
                && (current_lease != self.lease || main_identity != Some(self.identity)))
            || (self.lease.is_none()
                && !matches!(
                    self.disposition,
                    Some(
                        Disposition::Failed(_) | Disposition::Rejected(_) | Disposition::Cancelled
                    )
                ))
        {
            return Err(ParentError::WrongPhase);
        }
        Ok(self.lease)
    }
    /// Advance only after checked old Main retirement and current backend reload admission.
    /// `previous` is the original acquisition association; the caller authenticates `current`.
    pub(super) fn check_reload(
        &self,
        identity: MainIdentity,
        previous: LeaseKey,
        lease: LeaseKey,
    ) -> Result<(), ParentError> {
        if self.lease != Some(previous)
            || lease.run != previous.run
            || lease.connection != previous.connection
            || lease.resource.owner != previous.resource.owner
            || lease.resource.id != previous.resource.id
            || self.disposition != Some(Disposition::Granted)
            || !self.sent
            || identity.run != self.identity.run
            || identity.launch.get() <= self.identity.launch.get()
            || identity.navigation.get() <= self.identity.navigation.get()
        {
            return Err(ParentError::ForeignPeer);
        }
        Ok(())
    }
    /// Commit the already checked semantic association after actual resource reload admission.
    pub(super) fn reload(
        &mut self,
        identity: MainIdentity,
        previous: LeaseKey,
        lease: LeaseKey,
    ) -> Result<(), ParentError> {
        self.check_reload(identity, previous, lease)?;
        self.identity = identity;
        self.lease = Some(lease);
        self.retirement = None;
        Ok(())
    }
}
