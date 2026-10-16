// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Park one authentic pull before request admission while another client owns its shared loan.
use super::{ParentError, ParentLifecycle, ParentResources, Result};
use std::num::NonZeroU64;
use sugarsubstitute_contracts::{
    execution::{CounterDeadline, Moment},
    lifecycle::ResourceKey,
};
use sugarsubstitute_supervision::lifecycle_protocol::{DecodedPayload, LeaseReference, WireHeader};

impl ParentLifecycle {
    /// Observe only capacity after common native/header authentication. Actual lease,
    /// ACK and native batch admission remain with the ordinary request/output owners.
    pub(super) fn defer_output_receive(
        &mut self,
        header: WireHeader,
        payload: &DecodedPayload<'_>,
        resources: &ParentResources<'_, '_>,
    ) -> Result<bool> {
        self.output_receive_until = None;
        let DecodedPayload::OutputPull { lease, acknowledged_delivery } = payload else {
            return Ok(false);
        };
        let supplied = resources.execution.client.decode_counter_deadline(CounterDeadline::new(
            header.deadline_qpc,
            self.original.frequency(),
        )?)?;
        let now = resources.execution.client.now()?;
        // The ordinary owner handles expired provisional pulls. Any live wait must
        // first satisfy the same finite interval policy as ordinary admission.
        if now < supplied {
            self.deadline(header, resources)?;
        }
        self.wait_output_loan(*lease, *acknowledged_delivery, supplied, now, |key| {
            resources.backend.output_loan_available(key)
        })
    }

    /// Keep the original cutoff while waiting for another client's loan. Own ACKs
    /// and recovery notices must route immediately. Expiry returns through normal
    /// admission, including the existing provisional-acquisition retirement rule.
    /// The callback borrows only the external owner's actual capacity observation.
    pub(super) fn wait_output_loan(
        &mut self,
        reference: LeaseReference,
        acknowledged: Option<NonZeroU64>,
        supplied: Moment,
        now: Moment,
        available: impl FnOnce(ResourceKey) -> Result<bool>,
    ) -> Result<bool> {
        self.output_receive_until = None;
        if self.closed
            || self.cleanup.is_some()
            || self.attempt_cleanup.is_some()
            || self.output.is_some()
            || self.recovery.outage.is_some()
        {
            return Ok(false);
        }
        let Some(lease) = self.output_lease else {
            return Ok(false);
        };
        if super::replies::reference(lease) != reference {
            return Ok(false);
        }
        super::output::acknowledge_delivery(None, self.acknowledged_output, lease, acknowledged)?;
        let until = if self.lease.is_none() {
            self.acquisition.as_ref().ok_or(ParentError::WrongPhase)?.output_until(supplied)
        } else {
            supplied
        };
        if now >= until || available(lease.resource)? {
            return Ok(false);
        }
        self.output_receive_until = Some(until);
        Ok(true)
    }
}
