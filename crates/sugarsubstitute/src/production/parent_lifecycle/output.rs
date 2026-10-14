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
    pub(super) lease: LeaseKey,
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
        let current = self.output.as_ref().map(|delivery| (delivery.lease, delivery.id));
        if acknowledge_delivery(current, self.acknowledged_output, lease, acknowledged)? {
            self.acknowledged_output = acknowledged.map(|id| (lease, id));
            self.output = None;
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
        self.output = Some(Delivery { id, lease, guard });
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

/// Validate acknowledgement against the exact connection-bound lease, retaining consumed
/// correlation across idle replies. Only the first matching ACK disposes a current native loan.
fn acknowledge_delivery(
    current: Option<(LeaseKey, NonZeroU64)>,
    consumed: Option<(LeaseKey, NonZeroU64)>,
    lease: LeaseKey,
    acknowledged: Option<NonZeroU64>,
) -> std::result::Result<bool, ParentError> {
    match (current, acknowledged) {
        (Some((actual_lease, delivery)), Some(id)) if actual_lease == lease && delivery == id => {
            Ok(true)
        }
        (None, Some(id)) if consumed == Some((lease, id)) => Ok(false),
        (None, None) => Ok(false),
        _ => Err(ParentError::WrongPhase),
    }
}
impl ParentLifecycle {
    /// Begin a newly granted transcript only after the previous delivery loan was consumed
    /// or the prior lease was authoritatively retired. No live native loan is discarded.
    pub(super) fn begin_output_sequence(&mut self) -> std::result::Result<(), ParentError> {
        if self.output.is_some() {
            return Err(ParentError::WrongPhase);
        }
        self.reset_output();
        Ok(())
    }
    /// Reset only at checked lease release or settled acquisition cleanup. A fresh
    /// transcript starts at delivery one; retained reloads keep the same lease sequence.
    pub(super) fn reset_output(&mut self) {
        self.output = None;
        self.next_delivery = 0;
        self.acknowledged_output = None;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
    fn lease(id: u64) -> std::result::Result<LeaseKey, Box<dyn std::error::Error>> {
        use sugarsubstitute_contracts::lifecycle::{
            ConnectionId, LeaseId, ResourceGeneration, ResourceId, ResourceKey,
        };
        let identity = super::super::tests::identity()?;
        Ok(LeaseKey {
            run: identity.run,
            connection: ConnectionId::new(1)?,
            id: LeaseId::new(id)?,
            resource: ResourceKey {
                owner: identity.run.owner,
                id: ResourceId::new(1)?,
                generation: ResourceGeneration::new(1)?,
            },
        })
    }
    #[test]
    fn delivery_ack_idle_then_fresh_pull_keeps_highwater_without_disposal_or_phase_failure()
    -> TestResult {
        let lease = lease(1)?;
        let first = NonZeroU64::new(1).ok_or("First delivery identity missing")?;
        assert!(acknowledge_delivery(Some((lease, first)), None, lease, Some(first))?);
        let consumed = Some((lease, first));
        for _ in 0..3 {
            assert!(!acknowledge_delivery(None, consumed, lease, Some(first))?);
        }
        let second = NonZeroU64::new(2).ok_or("Second delivery identity missing")?;
        assert_eq!(
            acknowledge_delivery(Some((lease, second)), consumed, lease, Some(first)),
            Err(ParentError::WrongPhase)
        );
        assert!(acknowledge_delivery(Some((lease, second)), consumed, lease, Some(second))?);
        assert!(!acknowledge_delivery(None, Some((lease, second)), lease, Some(second))?);
        assert_eq!(
            acknowledge_delivery(None, Some((lease, second)), lease, Some(first)),
            Err(ParentError::WrongPhase)
        );
        Ok(())
    }
    #[test]
    fn invented_missing_and_future_acknowledgements_cannot_dispose_live_delivery() -> TestResult {
        let lease = lease(1)?;
        let first = NonZeroU64::new(1).ok_or("First delivery identity missing")?;
        let future = NonZeroU64::new(2).ok_or("Future delivery identity missing")?;
        assert!(!acknowledge_delivery(None, None, lease, None)?);
        assert_eq!(
            acknowledge_delivery(None, None, lease, Some(first)),
            Err(ParentError::WrongPhase)
        );
        assert_eq!(
            acknowledge_delivery(Some((lease, first)), None, lease, None),
            Err(ParentError::WrongPhase)
        );
        assert_eq!(
            acknowledge_delivery(Some((lease, first)), None, lease, Some(future)),
            Err(ParentError::WrongPhase)
        );
        assert_eq!(
            acknowledge_delivery(None, Some((lease, first)), lease, Some(future)),
            Err(ParentError::WrongPhase)
        );
        Ok(())
    }
    #[test]
    fn consumed_acknowledgement_cannot_cross_release_to_a_new_lease_or_dispose_its_live_loan()
    -> TestResult {
        let old = lease(1)?;
        let new = lease(2)?;
        let id = NonZeroU64::new(1).ok_or("Delivery identity missing")?;
        assert_eq!(
            acknowledge_delivery(None, Some((old, id)), new, Some(id)),
            Err(ParentError::WrongPhase)
        );
        assert_eq!(
            acknowledge_delivery(Some((old, id)), Some((old, id)), new, Some(id)),
            Err(ParentError::WrongPhase)
        );
        let identity = super::super::tests::identity()?;
        let storage = crate::production::coordinator_backend::tests::storage_tests::claim(
            ParentLifecycle::storage_bytes(),
        )
        .map_err(|cause| cause as Box<dyn std::error::Error>)?;
        let mut parent = ParentLifecycle::new(
            identity.run,
            identity.launch,
            sugarsubstitute_contracts::configuration::ProfileId::new("Output_1".into())?,
            sugarsubstitute_contracts::lifecycle::ConnectionId::new(1)?,
            sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeDeadline::new(
                100,
                1_000_000_000,
            )?,
            storage,
        )?;
        parent.next_delivery = 1;
        parent.acknowledged_output = Some((old, id));
        parent.begin_output_sequence()?;
        assert_eq!(parent.next_delivery, 0);
        assert_eq!(parent.acknowledged_output, None);
        assert!(parent.output.is_none());
        assert_eq!(
            acknowledge_delivery(None, parent.acknowledged_output, new, Some(id)),
            Err(ParentError::WrongPhase)
        );
        assert!(!acknowledge_delivery(None, parent.acknowledged_output, new, None)?);
        Ok(())
    }
}
