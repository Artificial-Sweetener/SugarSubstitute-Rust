// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fan out one real batch through independently authenticated, precharged Parent reply slots.
#[path = "output/fanout.rs"]
mod fanout;
#[path = "output/subscribers.rs"]
mod subscribers;
use super::*;
pub(crate) use fanout::Receipt as OutputReceipt;
pub(super) use fanout::ResourceOutput;
pub(super) use subscribers::Subscribers;
use sugarsubstitute_execution::windows_output::WindowsOutputError;

impl CoordinatorBackend {
    /// A retained shared batch is available to another subscriber despite the native loan.
    /// An already-consumed shared batch also permits immediate Idle, without waiting on another ACK.
    /// This is capacity only; actual lease and ACK validation stay with the Parent.
    pub(crate) fn output_loan_available(&self, lease: LeaseKey) -> Result<bool> {
        let record = self.record(lease.resource)?;
        if lease.run.owner != self.owner {
            return Err(LifecycleRejection::ForeignOwner.into());
        }
        let client = self.output_clients.existing(lease)?;
        if record.output_fanout.available(client)
            || record.output_fanout.current.is_some()
                && client.is_some_and(|client| client.pending.is_none())
        {
            return Ok(true);
        }
        Ok(record.output_fanout.current.is_none()
            && record.output.as_ref().is_none_or(|output| output.control.loan_available()))
    }
    /// Copy one original batch into the authenticated Parent's existing charged reply slot.
    /// No transcript sequence is renumbered; loss reflects actual bytes missed by this subscriber.
    /// The sole original guard remains here through every registered subscriber's real ACK.
    pub(crate) fn take_output(
        &mut self,
        lease: LeaseKey,
        destination: &mut [u8],
    ) -> Result<Option<fanout::Delivery>> {
        let at = self.index(lease.resource)?;
        let record = self.records[at].as_mut().ok_or(LifecycleRejection::StaleIdentity)?;
        if record.shutdown.is_some() || lease.run.owner != self.owner {
            return Err(LifecycleRejection::WrongPhase.into());
        }
        // Reserve bounded cursor metadata before taking any native output loan.
        self.output_clients.subscribe(lease)?;
        record.output_fanout.release_consumed(record.key, &self.output_clients);
        if record.output_fanout.current.is_none()
            && let Some(output) = &record.output
        {
            match output.control.take_batch() {
                Ok(Some(guard)) => record.output_fanout.adopt(record.key, guard)?,
                Ok(None) | Err(WindowsOutputError::NotReady) => {}
                Err(cause) => return Err(cause.into()),
            }
        }
        let client = self.output_clients.subscribe(lease)?;
        record.output_fanout.copy(client, destination)
    }
    /// Only a matching live Parent receipt advances this exact connection-bound subscriber.
    pub(crate) fn acknowledge_output(
        &mut self,
        lease: LeaseKey,
        receipt: &OutputReceipt,
    ) -> Result<()> {
        self.output_clients.acknowledge(lease, receipt)?;
        self.release_consumed_output();
        Ok(())
    }
    /// Release only fully consumed original loans; their Drop wakes existing capacity waiters.
    pub(super) fn release_consumed_output(&mut self) {
        for record in self.records.iter_mut().flatten() {
            record.output_fanout.release_consumed(record.key, &self.output_clients);
        }
    }
}
#[cfg(test)]
#[path = "output_tests.rs"]
mod tests;
