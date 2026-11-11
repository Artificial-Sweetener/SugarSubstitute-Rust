// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish exact engine attempts and restore only a continuously owned, never-started attempt.
use super::*;

impl TrustStore {
    /// Reserve only existing preparation; losing its journal cannot authorize reinitialization.
    pub(in crate::setup_storage) fn reserve_updater_trial(
        &self,
        generation: &Generation,
        context: &JobContext,
    ) -> Result<InstallationRecord, TrustFailure> {
        self.protected(context, |protection| {
            let _writer = self.writer(protection)?;
            let state = self.updater_locked(protection, SelectionRead::Existing, context)?;
            if !matches!(state.candidate(), Some(Candidate::Prepared { generation: selected }) if selected == generation) {
                return Err(invalid("Exact updater preparation is required for a trial").into());
            }
            self.updater_receipt(generation, protection)?;
            Ok(state)
        })
    }

    /// Called inside authenticated publication with the trust writer and preparation lock held.
    /// Repeated publication of the same attempt is safe after an ambiguous storage completion.
    pub(in crate::setup_storage) fn begin_updater_trial(
        &self,
        prepared: &InstallationRecord,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        protection::with_domain(context, |protection| -> Result<_, TrustFailure> {
            let trial = prepared.begin_trial().map_err(io::Error::other)?;
            let state = self.updater_locked(protection, SelectionRead::Existing, context)?;
            let generation = prepared
                .candidate()
                .ok_or_else(|| invalid("Updater preparation is absent"))?
                .generation();
            self.updater_receipt(generation, protection)?;
            if state == trial {
                return Ok(());
            }
            if &state != prepared {
                return Err(invalid("Updater preparation changed before trial publication").into());
            }
            self.write_updater(&trial, protection, context)
        })
        .map_err(TrustFailure::from)
    }

    /// The creator retains the original preparation lock and has never granted native creation.
    /// Trust expiry or subscription changes cannot prevent withdrawing that unstarted attempt.
    pub(in crate::setup_storage) fn restore_unstarted_updater(
        &self,
        prepared: &InstallationRecord,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        let trial = prepared.begin_trial().map_err(io::Error::other)?;
        let Some(Candidate::Trial { attempt }) = trial.candidate() else {
            return Err(invalid("Updater trial is absent").into());
        };
        let restored = trial.cancel_unstarted_trial(attempt).map_err(io::Error::other)?;
        self.protected(context, |protection| {
            let _writer = self.writer(protection)?;
            let state = self.updater_locked(protection, SelectionRead::Existing, context)?;
            if &state == prepared || state == restored {
                return Ok(());
            }
            if state != trial {
                return Err(invalid("Updater attempt changed before withdrawal").into());
            }
            self.write_updater(&restored, protection, context)
        })
    }
}

impl TrustStore {
    /// Commit a settled result inside authenticated publication while its trust writer is held.
    pub(in crate::setup_storage) fn publish_updater_outcome(
        &self,
        trial: &InstallationRecord,
        next: &InstallationRecord,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        protection::with_domain(context, |protection| -> Result<_, TrustFailure> {
            self.commit_updater_outcome(trial, next, protection, context)
        })
        .map_err(TrustFailure::from)
    }
    /// Rejection and unstarted withdrawal do not require still-current download authorization.
    pub(in crate::setup_storage) fn withdraw_updater_trial(
        &self,
        trial: &InstallationRecord,
        next: &InstallationRecord,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        self.protected(context, |protection| {
            let _writer = self.writer(protection)?;
            self.commit_updater_outcome(trial, next, protection, context)
        })
    }
    /// Preserve all unrelated journal changes and permit retry after an ambiguous durable write.
    fn commit_updater_outcome(
        &self,
        trial: &InstallationRecord,
        next: &InstallationRecord,
        protection: &Protection,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        let actual = self.updater_locked(protection, SelectionRead::Existing, context)?;
        if &actual == next {
            return Ok(());
        }
        if &actual != trial {
            return Err(invalid("Updater attempt changed before outcome publication").into());
        }
        self.write_updater(next, protection, context)
    }
}
