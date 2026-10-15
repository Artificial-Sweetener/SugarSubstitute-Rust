// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route actual registry exit evidence without equating it with family/profile retirement.
use super::{ParentError, ParentLifecycle, ParentResources};
use sugarsubstitute_contracts::lifecycle::{ConnectionId, DesktopObservation, RunKey};
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
        if self.exit.is_none() {
            self.exit = Some(
                sugarsubstitute_application::supervision::DesktopExitTransaction::new(
                    self.run,
                    resources.process,
                )
                .map_err(|_| ParentError::ForeignPeer)?,
            );
        }
        self.exit
            .as_mut()
            .ok_or(ParentError::WrongPhase)?
            .observe(DesktopObservation::Exited { process: *process, code: i64::from(*code) })
            .map_err(|_| ParentError::WrongPhase)?;
        Ok(())
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
