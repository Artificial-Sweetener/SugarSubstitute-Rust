// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit an already verified helper and transfer its original probe custody after native settlement.
use super::*;
use sugarsubstitute_supervision::SettledUpdaterProbe;

/// Separate finite scopes and the original campaign cutoff for one contained helper.
pub(in crate::updates) struct LaunchScope {
    pub work: Scope,
    pub cleanup: Scope,
    pub native: ScopeSpec,
    pub key: CoordinatorKey,
    pub until: Moment,
}

/// Mandatory durable retirement retained independently of the completed native process table.
pub(in crate::updates) enum ProbeCustody {
    Unstarted(Box<WindowsLaunchInput>),
    Settled(Box<SettledUpdaterProbe>),
}

/// Actual process completion; probe publication remains the storage owner's responsibility.
pub(in crate::updates) struct Completion {
    pub probe: Option<ProbeCustody>,
    pub failure: Option<Failure>,
    pub succeeded: bool,
}

impl Parent {
    /// Adopt a prepared input only after inert construction succeeds. Errors preserve the caller's
    /// option, including an already published probe. The inherited campaign cutoff is never renewed.
    pub(in crate::updates) fn from_input(
        client: ExecutionClient,
        scope: LaunchScope,
        input: &mut Option<WindowsLaunchInput>,
    ) -> Result<Self> {
        input.as_ref().ok_or("Prepared helper input absent")?;
        let remaining = scope.until.checked_duration_since(client.now()?)?;
        if remaining.is_zero() {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "Helper campaign expired").into());
        }
        let wake_at = Instant::now().checked_add(remaining).ok_or("Helper cutoff overflow")?;
        Ok(Self::owned(
            client,
            scope,
            Retained { source: None, input: input.take(), registry: None },
            wake_at,
        ))
    }

    /// One initial state for application-derived and independently pinned helper images.
    pub(super) fn owned(
        client: ExecutionClient,
        scope: LaunchScope,
        held: Retained,
        wake_at: Instant,
    ) -> Self {
        let probe = held.input.as_ref().is_some_and(WindowsLaunchInput::is_updater_probe);
        Self {
            client,
            work: scope.work,
            cleanup: scope.cleanup,
            native: scope.native,
            key: scope.key,
            retained: Some(held),
            constructing: None,
            preparing: None,
            retiring: None,
            retiring_application: false,
            process: None,
            until: scope.until,
            wake_at,
            shutdown: None,
            continuation: None,
            grace_wake: None,
            settle_wake: None,
            stop_issued: false,
            retired: false,
            failure: None,
            probe,
            probe_custody: None,
            successful_exit: false,
            completion_taken: false,
        }
    }

    /// Transfer after all native observations and ordinary custody retirement have completed.
    /// An unstarted probe must be withdrawn; a settled probe must publish its immutable outcome.
    pub(in crate::updates) fn take_completion(&mut self) -> Option<Completion> {
        if !self.retired || self.completion_taken {
            return None;
        }
        self.completion_taken = true;
        Some(Completion {
            probe: self.probe_custody.take(),
            succeeded: self.successful_exit && self.failure.is_none(),
            failure: self.failure.take(),
        })
    }
}
