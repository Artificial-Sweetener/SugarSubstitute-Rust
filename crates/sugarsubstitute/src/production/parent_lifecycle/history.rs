// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain eight exact request digests without replaying an evicted operation.
use super::{ParentError, Reply};
use sha2::{Digest, Sha256};
use std::{collections::TryReserveError, fmt};
use sugarsubstitute_contracts::{
    lifecycle::{LeaseKey, RequestId, RequestOutcome, ResourceKey},
    startup::MainIdentity,
};
use sugarsubstitute_supervision::lifecycle_protocol::WireHeader;

pub(super) const SLOTS: usize = 8;

/// One typed command retained inside its original replay slot before any backend effect.
#[derive(Clone, Copy)]
pub(super) enum HeldBackendCommand {
    Acquire(sugarsubstitute_contracts::startup::MainIdentity),
    Release(LeaseKey),
    Reload {
        identity: sugarsubstitute_contracts::startup::MainIdentity,
        lease: LeaseKey,
    },
    Retry {
        identity: sugarsubstitute_contracts::startup::MainIdentity,
        failed: sugarsubstitute_supervision::lifecycle_protocol::LeaseReference,
    },
}
/// Finite logical admission outcome, never a fabricated native job terminal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HeldBackendTerminal {
    TimedOut,
    Cancelled,
}

pub(super) struct Record {
    pub(super) header: WireHeader,
    digest: [u8; 32],
    pub(super) identity: Option<MainIdentity>,
    pub(super) reply: Option<Reply>,
    pub(super) held_backend: Option<HeldBackendCommand>,
    pub(super) held_terminal: Option<HeldBackendTerminal>,
    backend: Option<RequestOutcome>,
    backend_lease: Option<LeaseKey>,
    pub(super) sent: bool,
    abandoned: bool,
    acquisition: Option<RequestId>,
    retired: bool,
    pub(super) until: Option<sugarsubstitute_contracts::execution::Moment>,
    readiness_waiting: bool,
    transfer_until: Option<sugarsubstitute_contracts::execution::Moment>,
    pub(super) output_bytes: Box<[u8]>,
}

#[derive(Debug)]
pub(super) enum RequestRejection {
    Protocol(ParentError),
    Allocation(TryReserveError),
}
impl RequestRejection {
    pub(super) const fn code(&self) -> ParentError {
        match self {
            Self::Protocol(code) => *code,
            Self::Allocation(_) => ParentError::Storage,
        }
    }
}
impl From<ParentError> for RequestRejection {
    fn from(code: ParentError) -> Self {
        Self::Protocol(code)
    }
}
impl fmt::Display for RequestRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Parent request admission rejected: {:?}", self.code())
    }
}
impl std::error::Error for RequestRejection {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Protocol(code) => Some(code),
            Self::Allocation(cause) => Some(cause),
        }
    }
}

pub(super) struct Requests {
    records: [Option<Record>; SLOTS],
    high_water: u64,
}

impl Requests {
    /// Retain authenticated configured readiness intent; this grants no process or Ready authority.
    /// A held command keeps its finite admission cutoff and must publish an actual terminal.
    pub(super) fn wait_for_readiness(&mut self, index: usize) -> Result<(), ParentError> {
        let record = self.get_mut(index)?;
        if record.held_backend.is_some()
            || record.reply.is_some()
            || record.sent
            || record.abandoned
        {
            return Err(ParentError::WrongPhase);
        }
        record.readiness_waiting = true;
        Ok(())
    }
    /// Freeze one finite native final transfer without changing the original wire header.
    pub(super) fn reply_deadline(
        &mut self,
        index: usize,
        now: sugarsubstitute_contracts::execution::Moment,
    ) -> Result<
        sugarsubstitute_contracts::execution::Moment,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        let record = self.get_mut(index)?;
        if !record.readiness_waiting {
            return Ok(record.until.ok_or(ParentError::WrongPhase)?);
        }
        if record.reply.is_none() {
            return Err(ParentError::WrongPhase.into());
        }
        if let Some(until) = record.transfer_until {
            return Ok(until);
        }
        let until = now.checked_add(std::time::Duration::from_secs(30))?;
        record.transfer_until = Some(until);
        Ok(until)
    }
    /// Borrow configured readiness intent; a held command is still subject to finite admission.
    pub(super) fn readiness_waiting(&self, index: usize) -> Result<bool, ParentError> {
        Ok(self.get(index)?.readiness_waiting)
    }
    pub(super) const fn new() -> Self {
        Self { records: [const { None }; SLOTS], high_water: 0 }
    }

    /// Inspect the existing replay/identity/capacity predicates without allocating or
    /// changing request high-water. Waiting for external capacity cannot hide a
    /// changed duplicate, stale request or exhausted replay window.
    pub(super) fn inspect(
        &self,
        header: WireHeader,
        bytes: &[u8],
    ) -> Result<(usize, bool), RequestRejection> {
        let request = header.request.ok_or(ParentError::WrongPhase)?;
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        if let Some(index) = self.find(request) {
            let record = self.records[index].as_ref().ok_or(ParentError::WrongPhase)?;
            if record.header != header || record.digest != digest {
                return Err(ParentError::ChangedDuplicate.into());
            }
            return Ok((index, false));
        }
        if request.get() <= self.high_water {
            return Err(ParentError::StaleRequest.into());
        }
        let index = self.records.iter().position(Option::is_none).or_else(|| {
            self.records
                .iter()
                .enumerate()
                .filter_map(|(index, record)| {
                    record
                        .as_ref()
                        .filter(|record| record.sent || record.retired)
                        .map(|record| (index, record.header.request.map_or(0, RequestId::get)))
                })
                .min_by_key(|(_, id)| *id)
                .map(|(index, _)| index)
        });
        let index = index.ok_or(ParentError::Overloaded)?;
        Ok((index, true))
    }

    /// Admission precedes target conversion, providers and every native effect.
    pub(super) fn admit(
        &mut self,
        header: WireHeader,
        bytes: &[u8],
    ) -> Result<(usize, bool), RequestRejection> {
        let (index, fresh) = self.inspect(header, bytes)?;
        if !fresh {
            return Ok((index, false));
        }
        let request = header.request.ok_or(ParentError::WrongPhase)?;
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        let mut output_bytes = Vec::new();
        output_bytes.try_reserve_exact(32768).map_err(RequestRejection::Allocation)?;
        if output_bytes.capacity() != 32768 {
            return Err(ParentError::Storage.into());
        }
        output_bytes.resize(32768, 0);
        self.records[index] = Some(Record {
            header,
            digest,
            identity: None,
            reply: None,
            held_backend: None,
            held_terminal: None,
            backend: None,
            backend_lease: None,
            sent: false,
            abandoned: false,
            acquisition: None,
            retired: false,
            until: None,
            readiness_waiting: false,
            transfer_until: None,
            output_bytes: output_bytes.into_boxed_slice(),
        });
        self.high_water = request.get();
        Ok((index, true))
    }

    pub(super) fn find(&self, request: RequestId) -> Option<usize> {
        self.records.iter().position(|record| {
            record.as_ref().is_some_and(|record| record.header.request == Some(request))
        })
    }

    pub(super) fn get(&self, index: usize) -> Result<&Record, ParentError> {
        self.records.get(index).and_then(Option::as_ref).ok_or(ParentError::WrongPhase)
    }

    pub(super) fn get_mut(&mut self, index: usize) -> Result<&mut Record, ParentError> {
        self.records.get_mut(index).and_then(Option::as_mut).ok_or(ParentError::WrongPhase)
    }

    /// Latch final producer data before fallible local routing; replay performs no effects.
    pub(super) fn observe_backend(
        &mut self,
        index: usize,
        outcome: RequestOutcome,
    ) -> Result<bool, ParentError> {
        let record = self.get_mut(index)?;
        if record.backend.is_some_and(|original| original != outcome)
            && !matches!(outcome, RequestOutcome::Pending(_))
        {
            return Err(ParentError::ChangedDuplicate);
        }
        if let RequestOutcome::Pending(lease)
        | RequestOutcome::Granted(lease)
        | RequestOutcome::Reloaded(lease) = outcome
        {
            if record.header.run != lease.run
                || record.header.connection != Some(lease.connection)
                || lease.resource.owner != lease.run.owner
                || record.backend_lease.is_some_and(|original| original != lease)
            {
                return Err(ParentError::ForeignPeer);
            }
            record.backend_lease = Some(lease);
        }
        if matches!(outcome, RequestOutcome::Pending(_)) {
            return Ok(false);
        }
        if let Some(original) = record.backend {
            if original != outcome {
                return Err(ParentError::ChangedDuplicate);
            }
            return Ok(false);
        }
        record.backend = Some(outcome);
        Ok(true)
    }

    /// Read only a lease observed from the actual backend under this exact request.
    pub(super) fn backend_resource(
        &self,
        request: RequestId,
    ) -> Result<Option<ResourceKey>, ParentError> {
        let index = self.find(request).ok_or(ParentError::StaleRequest)?;
        Ok(self.get(index)?.backend_lease.map(|lease| lease.resource))
    }

    /// Borrow only the lease retained from the actual original backend producer.
    #[cfg(test)]
    pub(super) fn backend_lease(
        &self,
        request: RequestId,
    ) -> Result<Option<LeaseKey>, ParentError> {
        let index = self.find(request).ok_or(ParentError::StaleRequest)?;
        Ok(self.get(index)?.backend_lease)
    }

    /// Fence this expired operation while retaining its exact header/digest/cutoff.
    /// This is no acknowledgement, semantic consumption or native settlement.
    pub(super) fn abandon(&mut self, index: usize) -> Result<(), ParentError> {
        self.get_mut(index)?.abandoned = true;
        Ok(())
    }

    /// Correlate an ordinary acquire/output operation before any expiry fencing.
    pub(super) fn acquisition(
        &mut self,
        index: usize,
        original: RequestId,
    ) -> Result<(), ParentError> {
        let record = self.get_mut(index)?;
        if record.acquisition.is_some_and(|old| old != original) {
            return Err(ParentError::ChangedDuplicate);
        }
        record.acquisition = Some(original);
        Ok(())
    }

    /// Borrow only the immutable acquisition association; this is no settlement fact.
    pub(super) fn belongs_to(
        &self,
        index: usize,
        original: RequestId,
    ) -> Result<bool, ParentError> {
        Ok(self.get(index)?.acquisition == Some(original))
    }

    /// Called only after the genuine settlement ACK write. Expired old requests become
    /// replay-evictable without falsely marking their ordinary replies as written.
    pub(super) fn settle_abandoned(&mut self, original: RequestId) {
        for record in self.records.iter_mut().flatten() {
            if record.acquisition == Some(original) && record.abandoned {
                record.retired = true;
            }
        }
    }

    /// Borrow the one original command still awaiting global backend admission.
    pub(super) fn held_backend(&self) -> Option<usize> {
        self.records
            .iter()
            .position(|record| record.as_ref().is_some_and(|record| record.held_backend.is_some()))
    }
    /// Return the original held command exactly once; closed admission never blocks other record operations.
    pub(super) fn poll_held_backend(
        &mut self,
        now: sugarsubstitute_contracts::execution::Moment,
        open: bool,
    ) -> Result<Option<(usize, HeldBackendCommand)>, ParentError> {
        let Some(index) = self.held_backend() else {
            return Ok(None);
        };
        if now >= self.get(index)?.until.ok_or(ParentError::WrongPhase)? {
            self.timeout_held(index)?;
            return Ok(None);
        }
        if !open {
            return Ok(None);
        }
        let command = self.get_mut(index)?.held_backend.take().ok_or(ParentError::WrongPhase)?;
        Ok(Some((index, command)))
    }
    /// Expire only never-admitted logical work at its retained cutoff. No cancellation or native settlement is inferred.
    pub(super) fn timeout_held(&mut self, index: usize) -> Result<(), ParentError> {
        let record = self.get_mut(index)?;
        if record.held_backend.is_none() {
            return Err(ParentError::WrongPhase);
        }
        record.held_terminal = Some(HeldBackendTerminal::TimedOut);
        record.held_backend = None;
        if record.readiness_waiting {
            record.reply = Some(Reply::Failed(
                sugarsubstitute_contracts::lifecycle::LifecycleFailureCode::Deadline,
            ));
        } else {
            record.abandoned = true;
            record.retired = true;
        }
        Ok(())
    }

    /// A deliberate close cancels only the still-unadmitted logical command.
    pub(super) fn cancel_held(&mut self, index: usize) -> Result<(), ParentError> {
        let record = self.get_mut(index)?;
        if record.held_backend.is_none() {
            return Err(ParentError::WrongPhase);
        }
        record.held_backend = None;
        record.held_terminal = Some(HeldBackendTerminal::Cancelled);
        if record.readiness_waiting {
            record.reply = Some(Reply::Cancelled);
        } else {
            record.abandoned = true;
            record.retired = true;
        }
        Ok(())
    }
    /// Preserve expected admitted Retry timeout independently of its actual producer/write settlement.
    pub(super) fn timeout_backend(&mut self, index: usize) -> Result<(), ParentError> {
        let record = self.get_mut(index)?;
        record.held_terminal = Some(HeldBackendTerminal::TimedOut);
        record.abandoned = true;
        Ok(())
    }
    /// The caller independently proves final backend observation and no outstanding native write.
    pub(super) fn retire_timed_out(&mut self, index: usize) -> Result<(), ParentError> {
        let record = self.get_mut(index)?;
        if record.held_terminal != Some(HeldBackendTerminal::TimedOut) || !record.abandoned {
            return Err(ParentError::WrongPhase);
        }
        record.retired = true;
        Ok(())
    }
    pub(super) fn next_reply(&self) -> Option<usize> {
        self.records.iter().position(|record| {
            record
                .as_ref()
                .is_some_and(|record| record.reply.is_some() && !record.sent && !record.abandoned)
        })
    }

    pub(super) fn next_deadline(&self) -> Option<sugarsubstitute_contracts::execution::Moment> {
        self.records
            .iter()
            .flatten()
            .filter(|record| !record.sent && !record.abandoned)
            .filter_map(|record| {
                if record.readiness_waiting && record.held_backend.is_none() {
                    record.transfer_until
                } else {
                    record.until
                }
            })
            .min()
    }
}
