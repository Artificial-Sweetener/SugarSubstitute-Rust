// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Find out whether a ComfyUI the application did not start holds the port of a start that
//! launches ComfyUI, ask the user whether to close it, and start again once it is closed.
//!
//! The port is looked at in the background as soon as the start begins preparing its
//! connection, without holding the launch back, so the question comes before the application's
//! own ComfyUI has loaded and failed to bind; and again when a start stops that was found free
//! early. Nothing is shown when the port is free or its listener belongs to a running copy of
//! this application: the splash keeps its ordinary retry. A verified foreign ComfyUI is offered
//! for closing; a listener that could not be verified is named so the user can close it
//! themselves. One start asks at most once.
use super::{StartupBackend, StartupHost, backend::PreparedStartup};
use crate::{
    backend_conflict::{BackendPortSession, Finding, RESERVATION_BYTES},
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

/// When, within one start, the port is looked at, and what an answer given early leads to.
#[derive(Debug, Default)]
pub(super) struct Trigger {
    /// The start now preparing has had its port looked at.
    early: bool,
    /// The stopped start has had its port looked at, or needs no look.
    stopped: bool,
    /// The user was shown a question or notice during this start.
    answered: bool,
    /// The user freed the port before the start stopped: start again when it stops.
    retry_on_stop: bool,
}

/// What one turn does about the port.
#[derive(Debug, Eq, PartialEq)]
enum Step {
    Nothing,
    /// Look at the port in the background.
    Check,
    /// Start again: the port was freed at the user's word.
    Retry,
}

impl Trigger {
    /// Decide this turn's step. `phase` is the phase of a start that launches ComfyUI, `None`
    /// when there is no such start; `resolving` is whether a look at the port is still running.
    fn turn(&mut self, phase: Option<StartupPhase>, resolving: bool, closing: bool) -> Step {
        match phase {
            Some(StartupPhase::PreparingConnection) => {
                self.stopped = false;
                if self.early || resolving || closing {
                    return Step::Nothing;
                }
                self.early = true;
                self.answered = false;
                Step::Check
            }
            Some(StartupPhase::Stopped) => {
                self.early = false;
                if resolving {
                    // The look that began early covers this stop as well.
                    self.stopped = true;
                    return Step::Nothing;
                }
                if self.retry_on_stop {
                    self.retry_on_stop = false;
                    self.stopped = true;
                    return if closing { Step::Nothing } else { Step::Retry };
                }
                if self.stopped || self.answered || closing {
                    self.stopped |= self.answered;
                    return Step::Nothing;
                }
                self.stopped = true;
                Step::Check
            }
            // A failed start is on its way to stopped: what was learned about it still holds.
            Some(StartupPhase::Failed | StartupPhase::Cancelling) => Step::Nothing,
            _ => {
                *self = Self::default();
                Step::Nothing
            }
        }
    }

    /// Record a finished look. `asked` is whether the user was shown anything, `freed` whether
    /// the port was freed at their word, `stopped` whether the start has already stopped.
    fn finished(&mut self, stopped: bool, asked: bool, freed: bool, closing: bool) -> Step {
        self.answered |= asked;
        if !freed || closing {
            return Step::Nothing;
        }
        if stopped {
            return Step::Retry;
        }
        self.retry_on_stop = true;
        Step::Nothing
    }
}

impl<B: StartupBackend> StartupHost<B> {
    /// The port and phase of a start that launches ComfyUI.
    fn launch(&self) -> Option<(u16, StartupPhase)> {
        let session = self.startup.as_ref()?.session();
        match session.configuration().connection() {
            ConnectionConfiguration::Launch(launch) => Some((launch.port().get(), session.phase())),
            ConnectionConfiguration::Attach(_) => None,
        }
    }

    /// Advance the resolution one turn. Called every turn.
    ///
    /// # Errors
    /// Returns the failure to start again after the port was freed.
    pub(super) fn advance_backend_port(&mut self) -> Result<(), Box<dyn Error>> {
        // Nothing is looked at before the window can be woken for the answer.
        let Some(wake) = self.backend_port_wake.clone() else {
            return Ok(());
        };
        let launch = self.launch();
        let step = self.backend_port_trigger.turn(
            launch.map(|(_, phase)| phase),
            self.backend_port.is_some(),
            self.closing,
        );
        match (step, launch) {
            (Step::Check, Some((port, _))) => {
                let session = BackendPortSession::new(port);
                let mut service = Service::admitted_as(
                    session.clone(),
                    Operation::NativeLifecycle,
                    RESERVATION_BYTES,
                );
                service.start(wake);
                self.backend_port = Some(BackendPortConflict {
                    service: Rc::new(RefCell::new(service)),
                    session,
                    offered: false,
                    taken: false,
                    dismissed: false,
                });
            }
            (Step::Retry, _) => self.retry_owned()?,
            _ => {}
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
        let (finished, continued, asked) = {
            let service = conflict.service.borrow();
            (service.finished(), service.continued(), conflict.offered)
        };
        if !finished {
            return Ok(());
        }
        self.backend_port = None;
        let stopped = self.launch().is_some_and(|(_, phase)| phase == StartupPhase::Stopped);
        if self.backend_port_trigger.finished(stopped, asked, continued, self.closing)
            == Step::Retry
        {
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
    use super::{Decision, Step, Trigger, decide};
    use crate::backend_conflict::Finding;
    use sugarsubstitute_application::startup::StartupPhase;
    use sugarsubstitute_contracts::instance_conflict::ConflictPhase;

    const PREPARING: Option<StartupPhase> = Some(StartupPhase::PreparingConnection);
    const STOPPED: Option<StartupPhase> = Some(StartupPhase::Stopped);

    #[test]
    fn a_start_is_looked_at_once_as_it_begins_and_that_look_covers_its_stop() {
        let mut trigger = Trigger::default();
        assert_eq!(trigger.turn(Some(StartupPhase::SplashFirstFrame), false, false), Step::Nothing);
        assert_eq!(trigger.turn(PREPARING, false, false), Step::Check);
        assert_eq!(trigger.turn(PREPARING, true, false), Step::Nothing);
        assert_eq!(trigger.turn(STOPPED, true, false), Step::Nothing);
        assert_eq!(trigger.finished(true, false, false, false), Step::Nothing);
        assert_eq!(trigger.turn(STOPPED, false, false), Step::Nothing);
    }

    #[test]
    fn a_start_found_free_early_is_looked_at_again_when_it_stops() {
        let mut trigger = Trigger::default();
        assert_eq!(trigger.turn(PREPARING, false, false), Step::Check);
        assert_eq!(trigger.finished(false, false, false, false), Step::Nothing);
        assert_eq!(trigger.turn(PREPARING, false, false), Step::Nothing);
        assert_eq!(trigger.turn(Some(StartupPhase::Failed), false, false), Step::Nothing);
        assert_eq!(trigger.turn(Some(StartupPhase::Cancelling), false, false), Step::Nothing);
        assert_eq!(trigger.turn(STOPPED, false, false), Step::Check);
        assert_eq!(trigger.turn(STOPPED, true, false), Step::Nothing);
        assert_eq!(trigger.finished(true, false, false, false), Step::Nothing);
        assert_eq!(trigger.turn(STOPPED, false, false), Step::Nothing);
    }

    #[test]
    fn a_port_freed_before_the_start_stops_starts_again_when_it_does() {
        let mut trigger = Trigger::default();
        assert_eq!(trigger.turn(PREPARING, false, false), Step::Check);
        assert_eq!(trigger.finished(false, true, true, false), Step::Nothing);
        assert_eq!(trigger.turn(PREPARING, false, false), Step::Nothing);
        assert_eq!(trigger.turn(Some(StartupPhase::Failed), false, false), Step::Nothing);
        assert_eq!(trigger.turn(STOPPED, false, false), Step::Retry);
        assert_eq!(trigger.turn(STOPPED, false, false), Step::Nothing);
        // The start that follows is looked at afresh.
        assert_eq!(trigger.turn(PREPARING, false, false), Step::Check);
    }

    #[test]
    fn a_port_freed_after_the_start_stopped_starts_again_at_once() {
        let mut trigger = Trigger::default();
        assert_eq!(trigger.turn(PREPARING, false, false), Step::Check);
        assert_eq!(trigger.turn(STOPPED, true, false), Step::Nothing);
        assert_eq!(trigger.finished(true, true, true, false), Step::Retry);
    }

    #[test]
    fn a_declined_question_is_not_asked_again_until_the_user_retries() {
        let mut trigger = Trigger::default();
        assert_eq!(trigger.turn(PREPARING, false, false), Step::Check);
        assert_eq!(trigger.finished(false, true, false, false), Step::Nothing);
        assert_eq!(trigger.turn(STOPPED, false, false), Step::Nothing);
        assert_eq!(trigger.turn(STOPPED, false, false), Step::Nothing);
        // Retry is a new start.
        assert_eq!(trigger.turn(PREPARING, false, false), Step::Check);
    }

    #[test]
    fn a_start_that_succeeds_or_a_closing_application_needs_nothing() {
        let mut trigger = Trigger::default();
        assert_eq!(trigger.turn(PREPARING, false, false), Step::Check);
        assert_eq!(trigger.finished(false, true, true, false), Step::Nothing);
        // The application's own ComfyUI took the freed port: nothing is owed at a later stop.
        assert_eq!(trigger.turn(Some(StartupPhase::ServerReady), false, false), Step::Nothing);
        assert_eq!(trigger.turn(Some(StartupPhase::Running), false, false), Step::Nothing);
        assert_eq!(trigger.turn(STOPPED, false, true), Step::Nothing);
        assert_eq!(trigger.turn(None, false, false), Step::Nothing);
        let mut closing = Trigger::default();
        assert_eq!(closing.turn(PREPARING, false, true), Step::Nothing);
        assert_eq!(closing.finished(true, true, true, true), Step::Nothing);
    }

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
