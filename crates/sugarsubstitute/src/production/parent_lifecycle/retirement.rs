// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route actual registry exit evidence without equating it with family/profile retirement.
use super::{ParentError, ParentLifecycle, ParentResources};
use sugarsubstitute_contracts::lifecycle::{ConnectionId, DesktopObservation, ProcessKey, RunKey};
use sugarsubstitute_supervision::RegistryObservation;

impl ParentLifecycle {
    /// Project only this parent's actual matching native registry exit transaction.
    pub(crate) fn failed_incident(
        &self,
        run: RunKey,
        process: sugarsubstitute_contracts::lifecycle::ProcessKey,
    ) -> std::result::Result<
        Option<sugarsubstitute_application::supervision::FailedDesktopFact>,
        ParentError,
    > {
        if run != self.run {
            return Err(ParentError::ForeignPeer);
        }
        let Some(exit) = &self.exit else {
            return Ok(None);
        };
        match exit.failed_incident() {
            Ok(fact) if fact.run() == run && fact.process() == process => Ok(Some(fact)),
            Ok(_) => Err(ParentError::ForeignPeer),
            Err(sugarsubstitute_contracts::lifecycle::LifecycleRejection::WrongPhase) => Ok(None),
            Err(_) => Err(ParentError::WrongPhase),
        }
    }
    pub(super) fn finish_disconnect(
        &mut self,
        resources: &mut ParentResources<'_, '_>,
    ) -> super::Result<()> {
        if !self.registered
            || self.disconnect_attempted
            || self.forwarded.is_some()
            || resources.backend.delivery_pending()
        {
            return Ok(());
        }
        let Some(budget) = self.cleanup else {
            return Ok(());
        };
        if !self.recovery_close_cause(resources)? {
            return Ok(());
        }
        self.disconnect_attempted = true;
        match resources.backend.disconnect(self.run, self.connection, budget, resources.execution) {
            Ok(transition) => self.backend_replies(transition, resources),
            Err(cause) => {
                self.forwarded = resources.backend.take_pending_transition();
                Err(cause)
            }
        }
    }

    /// Borrow observations already consumed from the actual coordinator registry.
    /// A root exit never asserts family, profile, pipe or capture settlement.
    pub(crate) fn registry_observation(
        &mut self,
        observation: &RegistryObservation,
        resources: &ParentResources<'_, '_>,
    ) -> std::result::Result<(), ParentError> {
        let RegistryObservation::RootExited { process, code } = observation else {
            return Ok(());
        };
        if *process != resources.process {
            return Ok(());
        }
        self.observe_desktop_exit(*process, *code)
    }
    /// Consume the exact independently watched recovery UI exit once.
    /// The composing recovery Record suppresses carrier-root exit routing and retains
    /// this original member owner through its actual terminal and family cleanup.
    /// Source expiry and watch cancellation cannot fabricate an actual UI exit.
    /// # Errors
    /// Rejects foreign run/family custody and repeated or inconsistent exit delivery.
    pub(crate) fn recovery_desktop_exit(
        &mut self,
        desktop: &sugarsubstitute_supervision::RecoveryEnrolledDesktop,
        resources: &ParentResources<'_, '_>,
    ) -> std::result::Result<(), ParentError> {
        if desktop.key().run() != self.run || desktop.process() != resources.process {
            return Err(ParentError::ForeignPeer);
        }
        let Some(actual) = desktop.actual_exit() else {
            return Ok(());
        };
        self.observe_desktop_exit(resources.process, actual.exit_code)
    }
    /// Both authenticated adapters preserve the same family correlation and exit owner.
    /// This private transition grants no process, profile, pipe or resource settlement.
    fn observe_desktop_exit(
        &mut self,
        process: ProcessKey,
        code: u32,
    ) -> std::result::Result<(), ParentError> {
        if self.exit.is_none() {
            self.exit = Some(
                sugarsubstitute_application::supervision::DesktopExitTransaction::new(
                    self.run, process,
                )
                .map_err(|_| ParentError::ForeignPeer)?,
            );
        }
        self.exit
            .as_mut()
            .ok_or(ParentError::WrongPhase)?
            .observe(DesktopObservation::Exited { process, code: i64::from(code) })
            .map_err(|_| ParentError::WrongPhase)
    }
}

/// Sealed disposition of this actual pipe actor's backend admission. Construction
/// requires checked native retirement; absent policy
/// records or process exit cannot manufacture a never-registered disposition.
pub(crate) struct RetiredRegistration {
    run: RunKey,
    registered: bool,
}
impl RetiredRegistration {
    /// Substitute only native retirement facts in owner-local classification tests.
    #[cfg(test)]
    pub(in crate::production) fn boundary_fixture(run: RunKey, registered: bool) -> Self {
        Self { run, registered }
    }
    pub(crate) fn run(&self) -> RunKey {
        self.run
    }
    pub(crate) fn registered(&self) -> bool {
        self.registered
    }
}
impl ParentLifecycle {
    /// Match the original launch before exporting its completed admission disposition.
    pub(crate) fn retired_registration(
        &self,
        run: RunKey,
        connection: ConnectionId,
    ) -> std::result::Result<Option<RetiredRegistration>, ParentError> {
        if run != self.run || connection != self.connection {
            return Err(ParentError::ForeignPeer);
        }
        if !self.native_retired() {
            return Ok(None);
        }
        Ok(Some(RetiredRegistration { run: self.run, registered: self.registered }))
    }
}

#[cfg(test)]
#[path = "retirement/recovery_tests.rs"]
mod recovery_tests;
