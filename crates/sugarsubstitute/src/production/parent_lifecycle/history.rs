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

pub(super) struct Record {
    pub(super) header: WireHeader,
    digest: [u8; 32],
    pub(super) identity: Option<MainIdentity>,
    pub(super) reply: Option<Reply>,
    backend: Option<RequestOutcome>,
    backend_lease: Option<LeaseKey>,
    pub(super) sent: bool,
    abandoned: bool,
    pub(super) until: Option<sugarsubstitute_contracts::execution::Moment>,
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
    pub(super) const fn new() -> Self {
        Self { records: [const { None }; SLOTS], high_water: 0 }
    }

    /// Admission precedes target conversion, providers and every native effect.
    pub(super) fn admit(
        &mut self,
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
                        .filter(|record| record.sent)
                        .map(|record| (index, record.header.request.map_or(0, RequestId::get)))
                })
                .min_by_key(|(_, id)| *id)
                .map(|(index, _)| index)
        });
        let index = index.ok_or(ParentError::Overloaded)?;
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
            backend: None,
            backend_lease: None,
            sent: false,
            abandoned: false,
            until: None,
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
            .filter_map(|record| record.until)
            .min()
    }
}
