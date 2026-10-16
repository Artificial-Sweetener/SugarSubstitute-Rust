// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire exact expired acquisition operations without late ordinary effects or replies.
use super::{ParentError, ParentLifecycle, Reply, Result, types::WritePurpose};
use std::num::NonZeroU64;
use sugarsubstitute_contracts::{execution::Moment, lifecycle::ClientBinding};
use sugarsubstitute_supervision::lifecycle_protocol::LeaseReference;

impl ParentLifecycle {
    /// Fence the unfinished original acquisition; request only never-issued write
    /// revocation and retain every native guard until its actual owner outcome.
    pub(super) fn expire_acquisition(&mut self, now: Moment) -> Result<()> {
        let Some(acquisition) = &mut self.acquisition else {
            return Ok(());
        };
        if !acquisition.expire(now) {
            return Ok(());
        }
        let original = acquisition.header.request.ok_or(ParentError::WrongPhase)?;
        if let Some(index) = self.requests.find(original) {
            self.requests.acquisition(index, original)?;
            self.requests.abandon(index)?;
        }
        if let Some(write) = &self.write {
            let current = match write.purpose {
                WritePurpose::Reply(index) => self.requests.belongs_to(index, original)?,
                WritePurpose::AcquisitionProgress(header) => header.request == Some(original),
                _ => false,
            };
            if current {
                // Already-issued I/O cannot be revoked. Its actual receipt/terminal
                // remains retained; this request is never recorded as completion.
                let _disposition = write.admission.revoke();
            }
        }
        Ok(())
    }

    /// Authenticate the exact provisional source and ACK before fencing an expired
    /// pull. No native output is consumed and the current passive loan stays owned.
    pub(super) fn retire_expired_output(
        &mut self,
        index: usize,
        binding: ClientBinding,
        reference: LeaseReference,
        acknowledged: Option<NonZeroU64>,
        supplied: Moment,
        now: Moment,
    ) -> Result<bool> {
        self.expire_acquisition(now)?;
        let Some(acquisition) = &self.acquisition else {
            return Ok(false);
        };
        if !acquisition.retiring() || now < acquisition.original {
            return Ok(false);
        }
        let lease = acquisition.output_lease(binding, reference)?;
        if self.output_lease != Some(lease) || supplied > acquisition.original {
            return Err(ParentError::ForeignPeer.into());
        }
        super::output::acknowledge_delivery(
            self.output.as_ref().map(|delivery| (delivery.lease, delivery.id)),
            self.acknowledged_output,
            lease,
            acknowledged,
        )?;
        let original = acquisition.header.request.ok_or(ParentError::WrongPhase)?;
        self.requests.acquisition(index, original)?;
        self.requests.get_mut(index)?.until = Some(supplied);
        self.requests.abandon(index)?;
        Ok(true)
    }

    /// Select a reply after disposing only expired current acquisition metadata.
    /// All unrelated expiry remains a strict original error, not a recoverable label.
    pub(super) fn next_live_reply(&mut self, now: Moment) -> Result<Option<usize>> {
        self.expire_acquisition(now)?;
        self.expire_recovery(now)?;
        for _ in 0..super::history::SLOTS {
            let Some(index) = self.requests.next_reply() else {
                return Ok(None);
            };
            let until = self.requests.reply_deadline(index, now)?;
            let record = self.requests.get(index)?;
            if now < until {
                return Ok(Some(index));
            }
            let acquisition = self.acquisition.as_ref().ok_or(ParentError::WrongPhase)?;
            let original = acquisition.header.request.ok_or(ParentError::WrongPhase)?;
            if !acquisition.retiring()
                || !self.requests.belongs_to(index, original)?
                || !matches!(
                    record.reply,
                    Some(
                        Reply::Output(_)
                            | Reply::OutputIdle(_)
                            | Reply::Granted { .. }
                            | Reply::Failed(_)
                            | Reply::Rejected(_)
                    )
                )
            {
                return Err(sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeError::DeadlineExpired.into());
            }
            self.requests.abandon(index)?;
        }
        Err(ParentError::Overloaded.into())
    }

    /// Retain the existing received loan while an original expired write still owns
    /// native custody. Metadata expiry cannot dispose a queued or issued operation.
    pub(super) fn retiring_write_pending(&self) -> Result<bool> {
        let Some(acquisition) = &self.acquisition else {
            return Ok(false);
        };
        if !acquisition.retiring() {
            return Ok(false);
        }
        let Some(write) = &self.write else {
            return Ok(false);
        };
        Ok(match write.purpose {
            WritePurpose::Reply(index) => self
                .requests
                .belongs_to(index, acquisition.header.request.ok_or(ParentError::WrongPhase)?)?,
            WritePurpose::AcquisitionProgress(header) => header == acquisition.header,
            _ => false,
        })
    }
}
