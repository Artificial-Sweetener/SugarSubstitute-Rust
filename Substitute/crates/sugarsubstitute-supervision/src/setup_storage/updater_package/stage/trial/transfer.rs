// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the published updater trial and one explicit inherited exclusion handle for native creation.
use super::*;
use crate::setup_storage::disk_inheritance::duplicate_inheritable;
use std::{
    error::Error,
    ffi::OsString,
    fmt,
    os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle},
    path::PathBuf,
};
use sugarsubstitute_contracts::application_launch::updater::probe::{PROBE_ARGUMENT, ProbeRequest};

/// Creator-owned trial, authenticated payload and inheritable duplicate of the same exclusion file.
/// Retain this owner through creation and actual family settlement. The original trial cannot be
/// restored as unstarted only before launch admission. This grants no process-health authority.
pub struct UpdaterProbeTransfer {
    trial: UnstartedUpdaterTrial,
    exclusion: File,
    published: bool,
    withdrawal_requested: bool,
}

/// Failed transfer preparation retaining the original unstarted owner and complete failure cause.
/// No native process was admitted. Retry or withdraw on an admitted storage worker before retiring it.
pub struct RejectedUpdaterProbe {
    trial: Box<UnstartedUpdaterTrial>,
    cause: TrustFailure,
}
impl RejectedUpdaterProbe {
    /// Recover original withdrawal/retry authority without creating fresh exclusion or changing state.
    pub fn into_parts(self) -> (UnstartedUpdaterTrial, TrustFailure) {
        (*self.trial, self.cause)
    }
}
impl fmt::Debug for RejectedUpdaterProbe {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RejectedUpdaterProbe")
            .field("cause", &self.cause)
            .finish_non_exhaustive()
    }
}
impl fmt::Display for RejectedUpdaterProbe {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Updater probe transfer failed while retaining its unstarted owner")
    }
}
impl Error for RejectedUpdaterProbe {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}

impl UnstartedUpdaterTrial {
    /// Reauthenticate and publish the exact attempt before preparing its native handle allowlist.
    /// Execute on an admitted storage worker. No process is created here. Failure preserves the
    /// original owner for retry or withdrawal; success transfers withdrawal authority to the input.
    /// # Errors
    /// Rejects expired authority, changed selection, cancellation or native duplication failure.
    pub fn into_probe(
        mut self,
        now: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<UpdaterProbeTransfer, RejectedUpdaterProbe> {
        let prepared = (|| -> Result<File, TrustFailure> {
            self.publish(now, context)?;
            let exclusion = {
                // The child adopts while this creator still holds its slot and handoff byte.
                self.prepared._preparation.hold_handoff(context, context.now())?;
                duplicate_inheritable(self.prepared._preparation.witness())?
            };
            check(context)?;
            Ok(exclusion)
        })();
        let exclusion = match prepared {
            Ok(exclusion) => exclusion,
            Err(cause) => return Err(RejectedUpdaterProbe { trial: Box::new(self), cause }),
        };
        self.closed = true;
        Ok(UpdaterProbeTransfer {
            trial: self,
            exclusion,
            published: false,
            withdrawal_requested: false,
        })
    }
}

impl UpdaterProbeTransfer {
    /// Permanently close launch admission and restore this never-submitted attempt to prepared.
    /// Call on an admitted storage worker while this transfer remains owned by the creator.
    /// Failure retains the original files and exclusion for retry; even a failed withdrawal
    /// prevents this transfer from being used to compose another launch input.
    /// # Errors
    /// Preserves cancellation, changed journal state and storage failures without releasing custody.
    pub fn withdraw_before_launch(&mut self, context: &JobContext) -> Result<(), TrustFailure> {
        self.withdrawal_requested = true;
        self.publish_settled(
            crate::setup_storage::UpdaterProbeDisposition::Unstarted,
            || 0,
            context,
        )
    }

    /// Only an unpublished, never-withdrawn transfer can grant creation authority.
    pub(crate) fn can_launch(&self) -> bool {
        !self.withdrawal_requested && !self.published
    }

    /// Reverify this exact accepted generation using the original retained trust workspace.
    /// Only the registry's accepted settled result may call this after publication. The returned
    /// image retains payload files independently, allowing original preparation custody to retire.
    pub(crate) fn pin_accepted(
        &self,
        context: &JobContext,
    ) -> Result<crate::setup_storage::PreparationImage, TrustFailure> {
        if !self.published || self.withdrawal_requested {
            return Err(invalid("Updater acceptance has not been published").into());
        }
        let source = &self.trial.prepared.source;
        let image = source
            .trust
            .pin_updater(
                source.offer.authorization(&source.trust).request.channel,
                source.offer.offer().offer().sequence(),
                context,
            )?
            .ok_or_else(|| invalid("Accepted updater image is absent"))?;
        if image.generation() != &self.trial.attempt.generation {
            return Err(invalid("Accepted updater differs from the settled probe").into());
        }
        Ok(image)
    }

    /// Commit only the registry's immutable terminal decision while retaining original custody.
    pub(crate) fn publish_settled(
        &mut self,
        disposition: crate::setup_storage::UpdaterProbeDisposition,
        now: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        use crate::setup_storage::UpdaterProbeDisposition;
        check(context)?;
        if self.withdrawal_requested && !matches!(disposition, UpdaterProbeDisposition::Unstarted) {
            return Err(invalid("Updater probe launch was withdrawn").into());
        }
        if self.published {
            return Ok(());
        }
        let trial = self.trial.snapshot.begin_trial().map_err(io::Error::other)?;
        let next = match disposition {
            UpdaterProbeDisposition::Accepted => trial.accept(&self.trial.attempt),
            UpdaterProbeDisposition::Rejected => trial.reject(
                &self.trial.attempt,
                self.trial.prepared.source.trust.strike_clock().now().map_err(io::Error::other)?,
            ),
            UpdaterProbeDisposition::Unstarted => trial.cancel_unstarted_trial(&self.trial.attempt),
        }
        .map_err(io::Error::other)?;
        let source = &self.trial.prepared.source;
        if matches!(disposition, UpdaterProbeDisposition::Accepted) {
            let bytes = source.download.identity();
            source.offer.authorization(&source.trust).publish_updater(
                bytes,
                now,
                context,
                |_, recheck| {
                    recheck()?;
                    source
                        .trust
                        .publish_updater_outcome(&trial, &next, context)
                        .map_err(io::Error::other)
                },
            )?;
        } else {
            source.trust.withdraw_updater_trial(&trial, &next, context)?;
        }
        self.published = true;
        Ok(())
    }

    /// Exact published attempt retained by this creator, never a health acknowledgement.
    pub fn attempt(&self) -> &Trial {
        &self.trial.attempt
    }

    /// Canonical installation path pinned by the original authenticated trust owner.
    pub fn installation(&self) -> &Path {
        self.trial.prepared.source.trust.updater_installation_path()
    }

    /// Verified generation root used as the native working directory.
    pub fn directory(&self) -> PathBuf {
        self.trial.prepared.source.trust.updater_path(&self.trial.attempt.generation)
    }

    /// Exact protected updater executable; its source and full dependency payload remain pinned.
    pub fn executable(&self) -> PathBuf {
        self.directory().join("app/sugarsubstitute-updater.exe")
    }

    /// Include only this borrowed handle in the native process inheritance allowlist.
    /// The creator must keep this owner alive until native creation and attribute retirement finish.
    pub fn exclusion_handle(&self) -> BorrowedHandle<'_> {
        self.exclusion.as_handle()
    }

    /// Versioned entry with bounded correlation and the actual retained inherited handle.
    /// Feed authority remains compiled into the receiving image.
    /// # Errors
    /// Preserves correlation encoding failure without releasing trial or payload custody.
    pub fn arguments(&self) -> io::Result<[OsString; 4]> {
        if !self.can_launch() {
            return Err(invalid("Updater probe launch is closed"));
        }
        Ok([
            PROBE_ARGUMENT.into(),
            self.installation().as_os_str().to_owned(),
            ProbeRequest::new(self.trial.attempt.clone())
                .and_then(|request| request.encode())
                .map_err(io::Error::other)?
                .into(),
            (self.exclusion.as_raw_handle() as usize).to_string().into(),
        ])
    }
}

#[cfg(test)]
#[allow(unsafe_code)]
mod tests;
