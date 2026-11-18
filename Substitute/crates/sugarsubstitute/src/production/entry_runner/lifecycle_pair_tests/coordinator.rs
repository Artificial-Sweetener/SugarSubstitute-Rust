// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The coordinator peer: the real entry runner on its own thread, with the activation adapter
//! and the pump that takes its restart notice's frames.
use super::{activation::WireActivation, clock::*, *};
use crate::{
    completion_wake::{Gate, ReleaseOnDrop, Wake},
    lifecycle_harness::{Delivery, LifecyclePeer, Turn, WireEnd},
    production::{coordinator_backend::BackendResources, parent_lifecycle::DesktopPeer},
};
use sugarsubstitute_contracts::{
    execution::{Clock, JobPhase, Moment, ScopeSpec},
    lifecycle::{ProcessId, ProcessKey, RunKey},
    startup::LaunchId,
};

/// What the scenarios read of the coordinator.
#[derive(Clone, Debug, Default)]
pub(super) struct CoordinatorState {
    pub(super) window_opened: bool,
    pub(super) has_desktop: bool,
    pub(super) closing: bool,
    pub(super) normal_shutdown: bool,
    pub(super) restart_decided: bool,
    pub(super) status_restarting: bool,
    pub(super) notice_on_screen: bool,
    pub(super) window_cut_off: bool,
    pub(super) relaunch_requested: bool,
    pub(super) cutoff_failure: bool,
    pub(super) failures: Vec<String>,
    pub(super) finished: bool,
}

/// The real entry runner on its own thread, with the activation adapter and the notice pump.
pub(super) struct CoordinatorPeer {
    _install_release: Option<ReleaseOnDrop>,
    install: Option<Arc<Gate>>,
    runner: Box<EntryRunner<WireActivation>>,
    native: Option<Scope>,
    clock: Arc<VirtualClock>,
    _installation: tempfile::TempDir,
    surfaced: bool,
    frame_at: Option<Duration>,
    phase: String,
    state: CoordinatorState,
    last: String,
}
impl CoordinatorPeer {
    pub(super) fn new(
        wire: WireEnd,
        wake: Arc<Wake>,
        run: RunKey,
        launch: LaunchId,
    ) -> HarnessResult<Self> {
        let installation = tempfile::tempdir()?;
        let clock = Arc::new(VirtualClock::default());
        let process = ProcessKey { owner: run.owner, id: ProcessId::new(1)? };
        let activation = WireActivation {
            wire,
            clock: clock.clone(),
            desktop: Some(DesktopPeer::new(run, launch, process)?),
            retired: None,
            backend: None,
            failure: None,
            budget: None,
        };
        let command = ProductionCommand {
            launch_selection: None,
            installation: installation.path().to_path_buf(),
            diagnostics: Default::default(),
            options: crate::production::entry_runner::tests::unopened_paths()?.into(),
        };
        let mut runner = Box::new(EntryRunner::new(
            command,
            installation.path().join("unopened.exe"),
            Box::new(activation),
            crate::diagnostics::DiagnosticHealth::fixture(),
        ));
        runner.clock = clock.clone();
        runner.native_now = native_now;
        runner.authority = Some(CoordinatorAuthority::new(
            sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget::new(
                Moment::from_nanos(0),
            )?,
            None,
        )?);
        runner.launch_update = crate::updates::launch::LaunchUpdate::new(Some(
            crate::updates::UpdatePolicy::BetaEager,
        ));
        runner.initialize_execution(wake)?;
        // The entry server elected this coordinator and admitted the desktop's launch.
        runner.original =
            Some(sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget::new(clock.now())?);
        runner.initial_completed = true;
        runner.intake_settled = true;
        let client = runner.client()?.clone();
        let native = client.create_native_scope(
            ScopeSpec { id: runner.scopes.allocate()?, deadline: None },
            None,
        )?;
        Ok(Self {
            _install_release: None,
            install: None,
            runner,
            native: Some(native),
            clock,
            _installation: installation,
            surfaced: false,
            frame_at: None,
            phase: String::new(),
            state: CoordinatorState::default(),
            last: String::new(),
        })
    }

    fn log(&self) -> &Log {
        self.runner.child.wire.log()
    }

    /// Run `with` on the backend owner's borrowed resources and the activation adapter.
    fn with_backend<R>(
        &mut self,
        with: impl FnOnce(&BackendResources<'_>, &mut WireActivation) -> R,
    ) -> HarnessResult<R> {
        let runner = &mut *self.runner;
        let resources = BackendResources {
            client: runner.client.as_ref().ok_or("Client absent")?,
            native: self.native.as_ref().ok_or("Native scope absent")?,
            cleanup: runner.cleanup.as_ref().ok_or("Cleanup scope absent")?,
            jobs: &runner.jobs,
            scopes: &runner.scopes,
            authority: runner.authority.as_mut().ok_or("Authority absent")?,
            capture: None,
        };
        Ok(with(&resources, &mut runner.child))
    }

    /// The coordinator's beta-eager check prepared version 2.0.0 before the window opened.
    pub(super) fn decide_restart(&mut self) -> HarnessResult {
        use sugarsubstitute_contracts::update::attempt::AttemptOutcome;
        self.log().note("coordinator: an update to 2.0.0 is prepared before the window opened");
        self.runner.observe_window();
        self.runner.launch_update.finish(&AttemptOutcome::Prepared { version: "2.0.0".into() });
        self.runner.announce_restart()
    }

    /// The backend's SugarCubes owner runs a job putting SugarCubes files back; with `held` it
    /// keeps running until [`Self::release_install`].
    pub(super) fn start_sugarcubes_job(&mut self, held: bool) -> HarnessResult {
        let gate = held.then(|| Arc::new(Gate::default()));
        let release = gate.clone();
        let retained = self
            .with_backend(|resources, _| {
                crate::production::coordinator_backend::tests::backend_with_pack_job(
                    resources, release,
                )
            })?
            .map_err(|cause| cause.to_string())?;
        self.runner.child.backend = Some(retained);
        if let Some(gate) = gate {
            self._install_release = Some(gate.guard());
            self.install = Some(gate);
        }
        self.log().note(format!("coordinator: a SugarCubes job starts (held: {held})"));
        Ok(())
    }

    /// The held SugarCubes job finishes.
    pub(super) fn release_install(&mut self) {
        if let Some(gate) = self.install.take() {
            self.log().note("coordinator: the SugarCubes job finishes");
            gate.release();
        }
    }

    /// The activation retains a failed backend cleanup.
    pub(super) fn fail_backend_cleanup(&mut self) {
        self.log().note("coordinator: the backend's cleanup failed");
        self.runner.child.failure = Some("The backend's cleanup failed".into());
    }

    /// The coordinator closes, as a session end or a second launch's close does.
    pub(super) fn close(&mut self) -> HarnessResult {
        self.log().note("coordinator: the coordinator closes");
        self.runner.accept_user_close();
        HeadlessLifetime::request_shutdown(self.runner.as_mut())
    }

    pub(super) fn state(&self) -> CoordinatorState {
        self.state.clone()
    }

    fn deliver(&mut self, delivery: Delivery) -> HarnessResult {
        let now = self.clock.now();
        let activation = &mut self.runner.child;
        match delivery {
            Delivery::Frame(bytes) => {
                if let Some(desktop) = &mut activation.desktop {
                    desktop.receive(&bytes, now)?;
                } else {
                    activation
                        .wire
                        .log()
                        .note("coordinator: a frame arrived after the desktop was gone");
                }
            }
            Delivery::Exited(code) => {
                if let Some(desktop) = &mut activation.desktop {
                    desktop.exited(code)?;
                }
                activation.retire("its process exited")?;
            }
            Delivery::Terminated => {
                return Err("A coordinator is never terminated by its window".into());
            }
        }
        Ok(())
    }

    /// The production activation's role cleanup at the original grace: a desktop whose close
    /// did not complete by then, or that never began the close it was asked for, is cut off.
    fn cut_off_overdue(&mut self) -> HarnessResult {
        let now = self.clock.now();
        let activation = &mut self.runner.child;
        let Some(desktop) = &activation.desktop else { return Ok(()) };
        let overdue_close =
            desktop.parent().overdue_close_request(now).is_some() && !desktop.completed();
        let asked = activation.budget.is_some_and(|budget| now >= budget.graceful_until)
            && desktop.parent().close_progress_request().is_none();
        if overdue_close || asked {
            activation
                .wire
                .log()
                .note("coordinator: the window's grace expired; its role cleanup cuts it off");
            activation.wire.signal(Delivery::Terminated)?;
            activation.retire("cut off")?;
            self.state.window_cut_off = true;
        }
        Ok(())
    }

    fn fingerprint(&self) -> String {
        let runner = &self.runner;
        format!(
            "{:?}|{}|{}|{}|{}|{:?}|{}|{}|{:?}",
            self.state,
            runner.closing,
            runner.child_closed,
            runner.restart_notice.is_some(),
            runner.restart_covered(),
            runner.update_status.snapshot().0,
            runner.child.desktop.is_some(),
            runner.failure.is_some(),
            runner.child.backend.as_ref().map(|retained| retained.backend.pack_idle()),
        )
    }
}

impl LifecyclePeer for CoordinatorPeer {
    fn turn(&mut self, now: Duration) -> HarnessResult<Turn> {
        NATIVE_ELAPSED.set(now);
        self.clock.set(now);
        let mut progressed = false;
        while let Some(delivery) = self.runner.child.wire.receive() {
            progressed = true;
            self.deliver(delivery)?;
        }
        if self.runner.child.desktop.as_ref().is_some_and(DesktopPeer::revealed)
            && !self.state.window_opened
        {
            self.state.window_opened = true;
            self.runner.authority.as_mut().ok_or("Authority absent")?.observe_window_opened();
        }
        self.cut_off_overdue()?;
        if self.runner.child.backend.is_some() {
            self.with_backend(|resources, activation| {
                activation.backend.as_mut().map(|retained| retained.backend.advance_pack(resources))
            })?
            .transpose()
            .map_err(|cause| cause.to_string())?;
        }
        let maximum = NonZeroUsize::MIN.saturating_add(31);
        if !self.runner.closing {
            if let Err(cause) = HeadlessLifetime::activity(self.runner.as_mut(), maximum) {
                self.state.failures.push(cause.to_string());
                self.log().note(format!("coordinator: its turn failed: {cause}"));
            }
            let mut admission = self.runner.normal_admission();
            admission.initial_retired = true;
            admission.intake_ready = true;
            self.runner.close_after_fence(admission, |_| Ok(true))?;
        }
        if self.runner.closing && !self.state.finished {
            match HeadlessLifetime::poll_shutdown(self.runner.as_mut()) {
                Ok(true) => {
                    let finished = HeadlessLifetime::finish_shutdown(self.runner.as_mut());
                    if let Err(cause) = &finished {
                        self.state.failures.push(cause.to_string());
                    }
                    self.state.finished = true;
                    self.log().note(format!("coordinator: the process exits ({finished:?})"));
                }
                Ok(false) => {}
                Err(cause) => {
                    self.state.failures.push(cause.to_string());
                    self.log().note(format!("coordinator: shutdown failed: {cause}"));
                }
            }
        }
        // The pump takes one notice frame per moment, as a display refreshes; once shown, the
        // notice's further frames wait for time to pass.
        let refresh = !self.state.notice_on_screen || self.frame_at != Some(now);
        if refresh && let Some(notice) = HeadlessLifetime::notice(self.runner.as_mut()) {
            self.frame_at = Some(now);
            if !self.surfaced {
                self.surfaced = true;
                self.log().note("coordinator: the restart notice's window opens");
                HeadlessLifetime::notice_surface(
                    self.runner.as_mut(),
                    sugarsubstitute_desktop::NoticeSurface {
                        shape: sugarsubstitute_contracts::presentation::FrameShape::new(
                            420, 420, 1.0,
                        )?,
                        dark: true,
                        accent: None,
                    },
                );
            }
            if notice.frame.is_some() && !self.state.notice_on_screen {
                self.state.notice_on_screen = true;
                self.log().note("coordinator: the Updating notice is on screen");
            }
        }
        let wrote = {
            let now = self.clock.now();
            let activation = &mut self.runner.child;
            match &mut activation.desktop {
                Some(desktop) => desktop.write(&activation.wire, now)?,
                None => false,
            }
        };
        progressed |= wrote;
        let runner = &self.runner;
        self.state.has_desktop = runner.child.desktop.is_some();
        self.state.closing = runner.closing;
        self.state.normal_shutdown = runner.normal_shutdown;
        self.state.restart_decided = runner.launch_update.restart().is_some();
        self.state.status_restarting = runner.update_status.snapshot().1.restarting();
        self.state.cutoff_failure |= runner.deadline_reported;
        if !matches!(runner.relaunch, crate::production::entry_runner::relaunch::Relaunch::Idle)
            && !self.state.relaunch_requested
        {
            self.state.relaunch_requested = true;
            self.log().note("coordinator: restart_to_apply_relaunch requested");
        }
        let phase = format!(
            "relaunch {}, notice busy {}, child closed {}, updater {}",
            match self.runner.relaunch {
                crate::production::entry_runner::relaunch::Relaunch::Idle => "idle",
                crate::production::entry_runner::relaunch::Relaunch::Launching(..) => "launching",
                crate::production::entry_runner::relaunch::Relaunch::Launched(_) => "launched",
                crate::production::entry_runner::relaunch::Relaunch::Done => "done",
            },
            self.runner.restart_notice.as_ref().is_some_and(|notice| notice.notice.busy()),
            self.runner.child_closed,
            self.runner.updater.is_some(),
        );
        let phase = match (self.runner.execution_stopping, &self.runner.execution) {
            (true, Some(execution)) => format!(
                "{phase}, unresolved {:?}",
                execution.poll_shutdown().map(|progress| progress.unresolved_jobs)
            ),
            _ => phase,
        };
        if self.runner.closing && phase != self.phase {
            self.log().note(format!("coordinator: closing: {phase}"));
            self.phase = phase;
        }
        let fingerprint = self.fingerprint();
        progressed |= fingerprint != self.last;
        self.last = fingerprint;
        // Once execution was asked to stop, each running job's end and the last worker's
        // return deliver a completion wake, so stopping execution is moving. A job still queued
        // when the stop is asked is taken by a worker in real time and ends the same way: it is
        // moving too, however late a loaded machine starts it. A job the test holds moves only
        // when the test releases it.
        let workers_returning = self.runner.execution_stopping
            && self.install.is_none()
            && self.runner.execution.as_ref().is_some_and(|execution| {
                execution.poll_shutdown().is_ok_and(|progress| {
                    !progress.settled
                        && progress
                            .unresolved_jobs
                            .iter()
                            .all(|job| matches!(job.phase, JobPhase::Queued | JobPhase::Running))
                })
            });
        let busy = workers_returning
            || (!self.state.notice_on_screen
                && self.runner.restart_notice.as_ref().is_some_and(|notice| notice.notice.busy()))
            || matches!(
                self.runner.relaunch,
                crate::production::entry_runner::relaunch::Relaunch::Launching(..)
            )
            || (self.install.is_none()
                && self
                    .runner
                    .child
                    .backend
                    .as_ref()
                    .is_some_and(|retained| !retained.backend.pack_idle()));
        let native = HeadlessLifetime::next_deadline(self.runner.as_ref()).and_then(|instant| {
            instant.checked_duration_since(
                sugarsubstitute_execution::qualification_time::anchor() + BASE,
            )
        });
        let timer = self.clock.next_timer().and_then(elapsed_of);
        let due = native.into_iter().chain(timer).min();
        Ok(Turn { progressed, busy, due })
    }
}
