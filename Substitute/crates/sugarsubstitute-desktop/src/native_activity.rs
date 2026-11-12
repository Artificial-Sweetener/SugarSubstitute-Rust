// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own ordered native eligibility, actual UI challenge dispatch and final native retirement.

use crate::{DesktopError, DesktopStartupObservation};
use std::num::NonZeroU64;
use sugarsubstitute_contracts::lifecycle::{RunKey, UiChallengeKey, UiWatchdogActivity};

/// These inputs come from retained native owners; none is inferred from loop return or shutdown intent.
#[derive(Clone, Copy)]
pub(super) struct RetirementFacts {
    pub(super) main: bool,
    pub(super) splash: bool,
    pub(super) observers: bool,
}

/// One desktop run owns its revision space and challenge lifetime on the UI thread.
pub(super) struct NativeActivity {
    run: RunKey,
    revision: u64,
    published: Option<UiWatchdogActivity>,
    last_challenge: u64,
    closing: bool,
    retired: bool,
}
impl NativeActivity {
    pub(super) const fn new(run: RunKey) -> Self {
        Self {
            run,
            revision: 0,
            published: None,
            last_challenge: 0,
            closing: false,
            retired: false,
        }
    }
    pub(super) fn bind(&self, run: RunKey) -> Result<(), DesktopError> {
        if run != self.run {
            return Err(DesktopError::message("native_activity_binding", "desktop run changed"));
        }
        Ok(())
    }
    /// Consume actual native eligibility in order; an exhausted revision fails before publication.
    pub(super) fn observe(
        &mut self,
        activity: UiWatchdogActivity,
    ) -> Result<Option<DesktopStartupObservation>, DesktopError> {
        if self.closing || self.published == Some(activity) {
            return Ok(None);
        }
        let revision = self.revision.checked_add(1).and_then(NonZeroU64::new).ok_or_else(|| {
            DesktopError::message("native_activity_revision", "native revision exhausted")
        })?;
        self.revision = revision.get();
        self.published = Some(activity);
        Ok(Some(DesktopStartupObservation::NativeActivity { run: self.run, revision, activity }))
    }
    /// Called exclusively while the outer UI event loop dispatches an authenticated content command.
    /// Foreign/stale keys fail before advancing history; closing retires queued challenges silently.
    pub(super) fn process_challenge(
        &mut self,
        key: UiChallengeKey,
        native_live: bool,
    ) -> Result<Option<DesktopStartupObservation>, DesktopError> {
        if self.closing {
            return Ok(None);
        }
        if key.run != self.run || key.id.get() <= self.last_challenge || !native_live {
            return Err(DesktopError::message(
                "process_ui_challenge",
                "challenge is foreign, stale or native lifetime unavailable",
            ));
        }
        self.last_challenge = key.id.get();
        Ok(Some(DesktopStartupObservation::UiChallengeProcessed(key)))
    }
    pub(super) fn close(&mut self) {
        self.closing = true;
    }
    /// Native completion precedes execution settlement so content can send its final transport fact.
    pub(super) fn retirement(
        &mut self,
        facts: RetirementFacts,
    ) -> Option<DesktopStartupObservation> {
        if self.closing && !self.retired && facts.main && facts.splash && facts.observers {
            self.retired = true;
            Some(DesktopStartupObservation::NativeRetired(self.run))
        } else {
            None
        }
    }
    pub(super) fn retired(&self) -> bool {
        self.retired
    }
}

#[cfg(test)]
#[path = "native_activity_tests.rs"]
mod tests;
