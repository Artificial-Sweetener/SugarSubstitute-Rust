// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain actual raw output until exact client consumption, including bounded replay bytes.
use super::{ParentError, ParentLifecycle, ParentResources, Reply, Result};
use std::num::NonZeroU64;
use sugarsubstitute_contracts::{
    lifecycle::LeaseKey,
    transcript::{OutputGap, OutputStream, StreamSequence},
};
use sugarsubstitute_execution::windows_output::AdmittedOutputBatch;
use sugarsubstitute_supervision::lifecycle_protocol::LeaseReference;

pub(super) struct Delivery {
    pub(super) id: NonZeroU64,
    pub(super) guard: AdmittedOutputBatch,
}
#[derive(Clone, Copy)]
pub(super) struct OutputFacts {
    pub(super) lease: LeaseKey,
    pub(super) id: NonZeroU64,
    pub(super) stream: OutputStream,
    pub(super) sequence: StreamSequence,
    pub(super) gap: Option<OutputGap>,
    pub(super) end: bool,
    pub(super) count: usize,
}
impl ParentLifecycle {
    pub(super) fn output_pull(
        &mut self,
        index: usize,
        reference: LeaseReference,
        acknowledged: Option<NonZeroU64>,
        resources: &ParentResources<'_, '_>,
    ) -> Result<()> {
        let lease = self.lease(reference)?;
        match (&self.output, acknowledged) {
            (Some(delivery), Some(id)) if delivery.id == id => self.output = None,
            (None, None) => {}
            _ => return Err(ParentError::WrongPhase.into()),
        }
        let id = NonZeroU64::new(
            self.next_delivery.checked_add(1).ok_or(ParentError::IdentityExhausted)?,
        )
        .ok_or(ParentError::IdentityExhausted)?;
        let Some(guard) = resources.backend.take_output(lease.resource)? else {
            self.requests.get_mut(index)?.reply = Some(Reply::OutputIdle(lease));
            return Ok(());
        };
        // Adopt the actual loan before any subsequent bounds/identity failure.
        self.output = Some(Delivery { id, guard });
        self.next_delivery = id.get();
        let batch = self
            .output
            .as_ref()
            .and_then(|delivery| delivery.guard.batch())
            .ok_or(ParentError::WrongPhase)?;
        if batch.resource() != lease.resource || batch.bytes().len() > 32768 {
            return Err(ParentError::ForeignPeer.into());
        }
        let record = self.requests.get_mut(index)?;
        record.output_bytes[..batch.bytes().len()].copy_from_slice(batch.bytes());
        record.reply = Some(Reply::Output(OutputFacts {
            lease,
            id,
            stream: batch.stream(),
            sequence: batch.sequence(),
            gap: batch.gap(),
            end: batch.end(),
            count: batch.bytes().len(),
        }));
        Ok(())
    }
}
