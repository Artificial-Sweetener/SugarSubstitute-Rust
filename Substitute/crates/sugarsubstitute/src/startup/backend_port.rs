// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! When a start that launches ComfyUI stops, find out whether a ComfyUI the application did not
//! start holds its port, ask the user whether to close it, and start again once it is closed.
//!
//! The check runs once for each stopped start, in the background. Nothing is shown when the
//! port is free or its listener belongs to a running copy of this application: the splash keeps
//! its ordinary retry. A verified foreign ComfyUI is offered for closing; a listener that could
//! not be verified is named so the user can close it themselves.
use super::{StartupBackend, StartupHost, backend::PreparedStartup};
use crate::{
    backend_conflict::{BackendPortSession, Finding},
    instance_conflict::Service,
};
use std::{cell::RefCell, error::Error, rc::Rc, sync::Arc};
use sugarsubstitute_application::startup::StartupPhase;
use sugarsubstitute_contracts::{
    configuration::ConnectionConfiguration,
    execution::{CompletionWake, Operation},
    instance_conflict::{ConflictAction, ConflictController, ConflictPhase, ConflictView},
};

/// One stopped start's resolution of its port.
pub(super) struct BackendPortConflict {
    service: Rc<RefCell<Service<BackendPortSession>>>,
    session: BackendPortSession,
    /// The window may take the question.
    offered: bool,
    /// The window took it.
    taken: bool,
    /// Nothing is to be asked; the resolution only retires.
    dismissed: bool,
}

/// The controller the window drives. The application keeps the service, which it started with
/// its own wake, so the window's start is already done.
struct Shared(Rc<RefCell<Service<BackendPortSession>>>);

impl ConflictController for Shared {
    fn start(&mut self, _wake: Arc<dyn CompletionWake>) {}
    fn poll(&mut self) -> ConflictView {
        self.0.borrow_mut().poll()
    }
    fn choose(&mut self, action: ConflictAction) {
        self.0.borrow_mut().choose(action);
    }
}

impl<B: StartupBackend> StartupHost<B> {
    /// The port a stopped launching start was to use.
    fn stopped_launch_port(&self) -> Option<u16> {
        let session = self.startup.as_ref()?.session();
        if session.phase() != StartupPhase::Stopped {
            return None;
        }
        match session.configuration().connection() {
            ConnectionConfiguration::Launch(launch) => Some(launch.port().get()),
            ConnectionConfiguration::Attach(_) => None,
        }
    }

    /// Advance the resolution one turn. Called every turn.
    ///
    /// # Errors
    /// Returns the failure to start again after the port was freed.
    pub(super) fn advance_backend_port(&mut self) -> Result<(), Box<dyn Error>> {
        let port = self.stopped_launch_port();
        if port.is_none() && self.backend_port.is_none() {
            self.backend_port_checked = false;
        }
        if let (Some(port), None, false, false, Some(wake)) = (
            port,
            &self.backend_port,
            self.backend_port_checked,
            self.closing,
            self.backend_port_wake.clone(),
        ) {
            self.backend_port_checked = true;
            let session = BackendPortSession::new(port);
            let mut service = Service::admitted_as(session.clone(), Operation::NativeLifecycle);
            service.start(wake);
            self.backend_port = Some(BackendPortConflict {
                service: Rc::new(RefCell::new(service)),
                session,
                offered: false,
                taken: false,
                dismissed: false,
            });
        }
        let Some(conflict) = &mut self.backend_port else {
            return Ok(());
        };
        let view = conflict.service.borrow_mut().poll();
        if !conflict.offered && !conflict.dismissed {
            match decide(view.phase, conflict.session.finding(), self.closing) {
                Decision::Wait => {}
                Decision::Ask => conflict.offered = true,
                Decision::Dismiss => {
                    conflict.dismissed = true;
                    conflict.service.borrow_mut().choose(ConflictAction::Cancel);
                }
            }
        }
        let (finished, continued) = {
            let service = conflict.service.borrow();
            (service.finished(), service.continued())
        };
        if !finished {
            return Ok(());
        }
        self.backend_port = None;
        // The port is free only if the start is still the stopped one the user answered for.
        if continued && !self.closing && self.stopped_launch_port().is_some() {
            self.retry_owned()?;
        }
        Ok(())
    }

    /// Hand the window the question once it is ready to be asked.
    pub(super) fn offered_backend_port(&mut self) -> Option<Box<dyn ConflictController>> {
        let conflict = self.backend_port.as_mut()?;
        if !conflict.offered || conflict.taken {
            return None;
        }
        conflict.taken = true;
        Some(Box::new(Shared(conflict.service.clone())))
    }
}

/// What to do with the resolution before anything has been shown.
#[derive(Debug, Eq, PartialEq)]
enum Decision {
    /// Discovery is still running.
    Wait,
    /// Show the question, or the notice that the listener could not be verified.
    Ask,
    /// Show nothing and retire.
    Dismiss,
}

/// Decide from the resolution's phase, what discovery found and whether the application is
/// closing. Only a foreign ComfyUI awaiting the user's decision, or a listener that could not be
/// verified, is ever shown.
fn decide(phase: ConflictPhase, finding: Finding, closing: bool) -> Decision {
    match (phase, finding) {
        (ConflictPhase::Checking, _) if !closing => Decision::Wait,
        (ConflictPhase::AwaitingDecision, Finding::Foreign) if !closing => Decision::Ask,
        (ConflictPhase::Failed, Finding::Unverified) if !closing => Decision::Ask,
        (ConflictPhase::Retiring { .. } | ConflictPhase::Done, _) => Decision::Wait,
        _ => Decision::Dismiss,
    }
}

#[cfg(test)]
mod tests {
    use super::{Decision, decide};
    use crate::backend_conflict::Finding;
    use sugarsubstitute_contracts::instance_conflict::ConflictPhase;

    #[test]
    fn only_a_foreign_comfyui_or_an_unverified_listener_is_shown() {
        assert_eq!(decide(ConflictPhase::Checking, Finding::Unknown, false), Decision::Wait);
        assert_eq!(decide(ConflictPhase::AwaitingDecision, Finding::Foreign, false), Decision::Ask);
        assert_eq!(decide(ConflictPhase::Failed, Finding::Unverified, false), Decision::Ask);
    }

    #[test]
    fn a_free_port_or_the_applications_own_backend_shows_nothing() {
        for finding in [Finding::NotListening, Finding::Application, Finding::Unknown] {
            assert_eq!(decide(ConflictPhase::Failed, finding, false), Decision::Dismiss);
        }
    }

    #[test]
    fn a_closing_application_asks_nothing_and_lets_retirement_finish() {
        assert_eq!(decide(ConflictPhase::Checking, Finding::Unknown, true), Decision::Dismiss);
        assert_eq!(
            decide(ConflictPhase::AwaitingDecision, Finding::Foreign, true),
            Decision::Dismiss
        );
        assert_eq!(
            decide(ConflictPhase::Retiring { continuing: false }, Finding::Foreign, true),
            Decision::Wait
        );
        assert_eq!(decide(ConflictPhase::Done, Finding::Foreign, true), Decision::Wait);
    }
}
