// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fence exact Main creation before reporting native absence on the creating thread.
use crate::{DesktopError, MainIdentity};

/// Actual Session custody, sampled on its creating thread. All native construction,
/// failed partial results, COM receivers/callbacks and handles reside in MainActor.
/// MainActor::new performs metadata allocation only; native effects start after adoption.
pub(super) enum MainCustody {
    Absent,
    CheckedRetired(MainIdentity),
    Outstanding,
}

/// One inline creation fence; it does not hold or manufacture native retirement proof.
pub(super) struct MainFence {
    closed: Option<MainIdentity>,
}
impl MainFence {
    pub(super) const fn new() -> Self {
        Self { closed: None }
    }
    /// Validate the authoritative current application identity before native allocation.
    /// A command queued before or after absence cannot reopen that fenced attempt.
    pub(super) fn prepare(
        &self,
        requested: MainIdentity,
        current: Option<MainIdentity>,
    ) -> Result<(), DesktopError> {
        if current != Some(requested) || self.closed == Some(requested) {
            return Err(DesktopError::message(
                "prepare_main_identity",
                "main attempt is foreign or closed",
            ));
        }
        Ok(())
    }
    /// Join actual application construction custody with native actor custody, then
    /// close admission before the caller publishes MainAbsent. This starts no work.
    /// Any current actor, even a checked-retired one, proves creation rather than absence.
    pub(super) fn absent(
        &mut self,
        requested: MainIdentity,
        current: Option<MainIdentity>,
        application_construction_pending: bool,
        custody: MainCustody,
    ) -> Result<(), DesktopError> {
        self.prepare(requested, current)?;
        if application_construction_pending
            || !match custody {
                MainCustody::Absent => true,
                MainCustody::CheckedRetired(prior) => {
                    prior.run == requested.run
                        && prior.launch.get() < requested.launch.get()
                        && prior.navigation < requested.navigation
                }
                MainCustody::Outstanding => false,
            }
        {
            return Err(DesktopError::message(
                "observe_main_absence",
                "original main construction or native custody remains",
            ));
        }
        self.closed = Some(requested);
        Ok(())
    }
}

#[cfg(test)]
#[path = "startup_main_fence_tests.rs"]
mod tests;
