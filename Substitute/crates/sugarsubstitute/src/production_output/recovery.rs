// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode mounted-backend notices only inside the original authenticated finite request.
use super::*;
use sugarsubstitute_contracts::startup::MainIdentity;
use sugarsubstitute_supervision::lifecycle_protocol::ReplyOutcome;

/// Copy only bounded protocol facts; the child retains the actual frame until adoption.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackendNotice {
    Unavailable { failed: LeaseReference, status: u32, affected_clients: u16, retry_ready: bool },
    Replaced { identity: MainIdentity, previous: LeaseReference, current: LeaseReference },
    RetryFailed(LifecycleFailureCode),
    RetryRejected(LifecycleRejection),
}
#[derive(Clone, Copy)]
pub(super) enum RequestKind {
    Pull(Option<MainIdentity>),
    Retry(MainIdentity),
}
impl RequestKind {
    fn identity(self) -> Option<MainIdentity> {
        match self {
            Self::Pull(identity) => identity,
            Self::Retry(identity) => Some(identity),
        }
    }
}
/// Validate the codec's data against one retained original request, without native effects.
pub(super) fn notice(
    binding: OutputBinding,
    pending: Pending,
    header: WireHeader,
    reply: ReplyOutcome<'_>,
) -> Result<Option<BackendNotice>, OutputError> {
    if header != pending.header {
        return Err(OutputError::Identity);
    }
    let result = match reply {
        ReplyOutcome::BackendUnavailable { failed, status, affected_clients, retry_ready }
            if matches!(pending.kind, RequestKind::Pull(Some(_)))
                && failed == binding.lease
                && (1..=64).contains(&affected_clients) =>
        {
            BackendNotice::Unavailable { failed, status, affected_clients, retry_ready }
        }
        ReplyOutcome::BackendReplaced { identity, previous, current }
            if Some(identity) == pending.kind.identity()
                && identity.run == binding.run
                && previous == binding.lease
                && previous != current
                && (previous.resource != current.resource
                    || previous.generation != current.generation) =>
        {
            BackendNotice::Replaced { identity, previous, current }
        }
        ReplyOutcome::Failed(code) if matches!(pending.kind, RequestKind::Retry(_)) => {
            BackendNotice::RetryFailed(code)
        }
        ReplyOutcome::Rejected(code) if matches!(pending.kind, RequestKind::Retry(_)) => {
            BackendNotice::RetryRejected(code)
        }
        ReplyOutcome::OutputIdle(_) | ReplyOutcome::OutputDelivery(_)
            if matches!(pending.kind, RequestKind::Pull(_)) =>
        {
            return Ok(None);
        }
        _ => return Err(OutputError::Identity),
    };
    Ok(Some(result))
}
impl OutputOwner {
    /// Bind the already mounted Main once; this cannot authorize or construct a browser.
    pub(crate) fn running_main(&mut self, identity: MainIdentity) -> Result<(), OutputError> {
        if self.closed
            || identity.run != self.binding.run
            || self.main_identity.is_some_and(|current| current != identity)
        {
            return Err(OutputError::Identity);
        }
        self.main_identity = Some(identity);
        Ok(())
    }
    /// Queue one user retry after the previous pull/parser/frame has actually drained.
    pub(crate) fn prepare_retry(
        &mut self,
        header: WireHeader,
        original: Moment,
        identity: MainIdentity,
        failed: LeaseReference,
    ) -> Result<WireMessage<'static>, OutputError> {
        if self.main_identity != Some(identity) || failed != self.binding.lease {
            return Err(OutputError::Identity);
        }
        self.prepare_request(header, original, RequestKind::Retry(identity))
    }
    pub(crate) fn pending_retry(&self) -> bool {
        self.pending.is_some_and(|pending| matches!(pending.kind, RequestKind::Retry(_)))
    }
    pub(crate) fn withdraw_retry(&mut self, header: WireHeader) -> Result<(), OutputError> {
        let pending = self.pending.ok_or(OutputError::Order)?;
        if pending.header != header
            || pending.admitted
            || !self.pending_retry()
            || self.job.is_some()
            || self.source.state.lock().map_err(|_| OutputError::Poisoned)?.frame.is_some()
        {
            return Err(OutputError::Order);
        }
        self.pending = None;
        Ok(())
    }
    pub(super) fn prepare_request(
        &mut self,
        header: WireHeader,
        original: Moment,
        kind: RequestKind,
    ) -> Result<WireMessage<'static>, OutputError> {
        if self.closed || self.pending.is_some() || self.job.is_some() || self.terminal.is_some() {
            return Err(OutputError::Order);
        }
        if header.run != self.binding.run
            || header.connection != Some(self.binding.connection)
            || header.request.is_none()
            || header.deadline_qpc <= 0
        {
            return Err(OutputError::Identity);
        }
        self.pending = Some(Pending { header, original, admitted: false, kind });
        self.pending_message().map(|value| value.0).ok_or(OutputError::Order)
    }
    /// Reborrow only the exact never-admitted request; transport busy does not change its cutoff.
    pub(crate) fn pending_message(&self) -> Option<(WireMessage<'static>, Moment)> {
        self.pending.filter(|pending| !pending.admitted).map(|pending| {
            let payload = match pending.kind {
                RequestKind::Pull(_) => WirePayload::OutputPull {
                    lease: self.binding.lease,
                    acknowledged_delivery: self.acknowledged,
                },
                RequestKind::Retry(identity) => {
                    WirePayload::RetryBackend { identity, failed: self.binding.lease }
                }
            };
            (WireMessage { header: pending.header, payload }, pending.original)
        })
    }
}
