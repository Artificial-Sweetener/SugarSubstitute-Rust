// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The window half of the two-peer lifecycle harness: the real startup host with its close
//! save, restart hold and coordinator close, speaking lifecycle frames to the coordinator.
//!
//! The host runs on the inert platform adapter its own close tests use, with its real execution
//! owner and workspace work. This peer is that adapter's pipe client: it turns what the host asks
//! of its adapter into the frames the production child sends, in the production child's order,
//! one request at a time awaiting its reply, and turns the coordinator's frames into what the
//! adapter reports to the host. It follows `production/child/retirement.rs`: the exit intent is
//! queued when the close begins, "Restart to apply" once the close save has its outcome, and
//! the final completion only after the intent's reply and the native window's retirement. The
//! exit intent names a normal close for a window that reached running, and a cancelled startup
//! for one that did not.
use super::closing_tests::Fixture;
use super::*;
use crate::lifecycle_harness::{Delivery, HarnessResult, LifecyclePeer, Turn, WireEnd};
use crate::startup::platform_tests::Prepared;
use std::{collections::VecDeque, num::NonZeroUsize, time::Duration};
use sugarsubstitute_application::startup::{StartupPhase, StartupSession};
use sugarsubstitute_contracts::{
    configuration::*,
    lifecycle::{CompletionIntent, ConnectionId, RequestId, ShutdownId},
    startup::{StartupEvent, StartupObservation},
    update::status::UpdateStatus,
};
use sugarsubstitute_desktop::{StartupWindowContent, WindowContent};
use sugarsubstitute_supervision::lifecycle_protocol::{
    DecodedPayload, FrameCodec, ReplyOutcome, WireHeader, WireMessage, WirePayload,
};

/// Which window the pair starts with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WindowStart {
    /// The application window is revealed and running.
    Revealed,
    /// The splash is still loading; the window was never revealed.
    Loading,
}

/// One control the production child sends and awaits a reply for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Control {
    MainRevealed,
    ExitIntent,
    RestartToApply,
    CheckForUpdates,
    ExitCompleted,
}

/// What the scenarios read of the window.
#[derive(Clone, Debug, Default)]
pub(crate) struct WindowState {
    pub(crate) closing: bool,
    pub(crate) revealed: bool,
    pub(crate) exit_intent: Option<CompletionIntent>,
    pub(crate) intent_acknowledged: bool,
    pub(crate) restart_sent: bool,
    pub(crate) completed: bool,
    pub(crate) exited: bool,
    pub(crate) terminated: bool,
    pub(crate) restart_held: bool,
    pub(crate) close_saved: Option<String>,
    pub(crate) restarting_status: bool,
    pub(crate) restart_covered_status: bool,
    pub(crate) update_line: Option<String>,
    pub(crate) close_reason: String,
}

/// The real startup host, its close-test fixture and its pipe client.
pub(crate) struct WindowPeer {
    fixture: Fixture,
    wire: WireEnd,
    origin: Moment,
    applied: Duration,
    running: Option<Prepared>,
    status_revision: u64,
    next_request: u64,
    queued: VecDeque<Control>,
    pending: Option<(Control, u64)>,
    intent: Option<CompletionIntent>,
    restart_queued: bool,
    checks_sent: usize,
    intent_done: bool,
    completed_queued: bool,
    completed_done: bool,
    native_close: bool,
    native_retired: bool,
    exited: bool,
    terminated: bool,
    revealed: bool,
    last: String,
}

impl WindowPeer {
    /// Start the real host as `start` says, attached to `wire`.
    /// # Errors
    /// Returns the host's construction failure.
    pub(crate) fn new(wire: WireEnd, start: WindowStart) -> HarnessResult<Self> {
        let mut fixture = Fixture::new(true)?;
        // The page is not loaded: the close save keeps the input canvas and an empty workspace.
        fixture.host.workspace.as_mut().ok_or("Workspace")?.driver = None;
        let origin = fixture.clock.now();
        let running = match start {
            WindowStart::Revealed => None,
            WindowStart::Loading => {
                let loading = loading_session(&fixture.host)?;
                fixture.host.startup.replace(Prepared { session: loading, released: false })
            }
        };
        let mut peer = Self {
            fixture,
            wire,
            origin,
            applied: Duration::ZERO,
            running,
            status_revision: 0,
            next_request: 1,
            queued: VecDeque::new(),
            pending: None,
            intent: None,
            restart_queued: false,
            checks_sent: 0,
            intent_done: false,
            completed_queued: false,
            completed_done: false,
            native_close: false,
            native_retired: false,
            exited: false,
            terminated: false,
            revealed: false,
            last: String::new(),
        };
        if start == WindowStart::Revealed {
            peer.reveal()?;
        }
        Ok(peer)
    }

    /// The completion wake of the host's execution owner, shared with the coordinator's.
    pub(crate) fn wake(&self) -> Arc<crate::completion_wake::Wake> {
        self.fixture.wake.clone()
    }

    /// The run and launch this window belongs to.
    pub(crate) fn run_and_launch(
        &self,
    ) -> (sugarsubstitute_contracts::lifecycle::RunKey, sugarsubstitute_contracts::startup::LaunchId)
    {
        (self.fixture.host.run_key(), self.fixture.host.launch_id())
    }

    /// Reveal the application window, as the native reveal does, and tell the coordinator.
    /// # Errors
    /// Returns the host's command admission failure.
    pub(crate) fn reveal(&mut self) -> HarnessResult {
        if let Some(running) = self.running.take() {
            self.fixture.host.startup = Some(running);
        }
        let identity = self.fixture.host.main_identity().ok_or("Main identity")?;
        self.fixture.host.commands.reveal_main(identity)?;
        self.revealed = true;
        self.wire.log().note("window: the application window is revealed");
        self.queued.push_back(Control::MainRevealed);
        Ok(())
    }

    /// The user presses "Restart to apply".
    /// # Errors
    /// Returns the host's close failure.
    pub(crate) fn click_restart_to_apply(&mut self) -> HarnessResult {
        self.wire.log().note("window: the user presses Restart to apply");
        self.fixture.host.request_restart_to_apply()
    }

    /// The user cancels loading by closing the splash.
    /// # Errors
    /// Returns the host's close failure.
    pub(crate) fn cancel_loading(&mut self) -> HarnessResult {
        self.wire.log().note("window: the user cancels loading");
        self.fixture.host.observe(DesktopStartupObservation::CloseRequested)
    }

    /// What the scenarios read of this window now.
    pub(crate) fn state(&self) -> WindowState {
        let host = &self.fixture.host;
        let status = host.child.update_status.as_ref();
        WindowState {
            closing: host.closing,
            revealed: host.commands.revealed(),
            exit_intent: self.intent,
            intent_acknowledged: self.intent_done,
            restart_sent: self.restart_queued,
            completed: self.completed_done,
            exited: self.exited,
            terminated: self.terminated,
            restart_held: host.restart_close_held(),
            close_saved: host.close_save_outcome().map(|outcome| format!("{outcome:?}")),
            restarting_status: status.is_some_and(UpdateStatus::restarting),
            restart_covered_status: status.is_some_and(UpdateStatus::restart_covered),
            update_line: crate::startup::window_content::updating(
                &host.child,
                sugarsubstitute_splash::StartupStatus::Loading,
                false,
            )
            .map(|line| match line {
                sugarsubstitute_splash::StartupStatus::Updating(version) => {
                    format!("Updating to version {}", version.as_str())
                }
                other => format!("{other:?}"),
            }),
            close_reason: format!("{:?}", host.close_reason),
        }
    }

    fn header(&mut self, request: Option<u64>) -> HarnessResult<WireHeader> {
        Ok(WireHeader {
            run: self.fixture.host.run_key(),
            connection: Some(ConnectionId::new(1)?),
            request: request.map(RequestId::new).transpose()?,
            deadline_qpc: 1_000_000,
        })
    }

    fn deliver(&mut self, delivery: Delivery) -> HarnessResult {
        let bytes = match delivery {
            Delivery::Frame(bytes) => bytes,
            Delivery::Terminated => {
                self.terminated = true;
                self.wire.log().note("window: the process was terminated by the coordinator");
                return Ok(());
            }
            Delivery::Exited(_) => return Err("A window never receives a process exit".into()),
        };
        let frame = FrameCodec::decode(&bytes)?;
        match frame.payload {
            DecodedPayload::UpdateStatus { revision, status, .. } => {
                if revision.get() > self.status_revision {
                    self.status_revision = revision.get();
                    self.fixture.host.child.update_status = Some(UpdateStatus::decode(status)?);
                }
            }
            DecodedPayload::CloseRequest => {
                self.fixture.host.child.close_requested_by_coordinator = true;
            }
            DecodedPayload::Reply(outcome) => {
                let request = frame.header.request.map(|request| request.get());
                let Some((control, pending)) = self.pending else {
                    return Err("A reply arrived with no request outstanding".into());
                };
                if Some(pending) != request || !matches!(outcome, ReplyOutcome::Acknowledged) {
                    return Err(format!("{control:?} was not acknowledged: {outcome:?}").into());
                }
                match control {
                    Control::ExitIntent => self.intent_done = true,
                    Control::ExitCompleted => self.completed_done = true,
                    _ => {}
                }
                self.pending = None;
            }
            other => return Err(format!("The window cannot take {other:?}").into()),
        }
        Ok(())
    }

    /// Queue what the host asked of its adapter, as the production child queues it.
    fn queue_controls(&mut self) {
        let child = &self.fixture.host.child;
        if child.shutdown_starts > 0 && self.intent.is_none() {
            let running = self
                .fixture
                .host
                .startup
                .as_ref()
                .is_some_and(|startup| startup.session().phase() == StartupPhase::Running);
            self.intent = Some(if running {
                CompletionIntent::Normal
            } else {
                CompletionIntent::StartupCancelled
            });
            self.queued.push_back(Control::ExitIntent);
        }
        if child.restart_requests > 0 && !self.restart_queued {
            self.restart_queued = true;
            self.queued.push_back(Control::RestartToApply);
        }
        while self.checks_sent < child.check_requests {
            self.checks_sent += 1;
            self.queued.push_back(Control::CheckForUpdates);
        }
        if self.native_retired
            && self.intent_done
            && !child.shutdown_requests.is_empty()
            && !self.completed_queued
        {
            self.completed_queued = true;
            self.queued.push_back(Control::ExitCompleted);
        }
    }

    /// Send the next queued control once the previous one was acknowledged.
    fn send_controls(&mut self) -> HarnessResult<bool> {
        if self.pending.is_some() {
            return Ok(false);
        }
        let Some(control) = self.queued.pop_front() else { return Ok(false) };
        let request = self.next_request;
        self.next_request += 1;
        let shutdown = ShutdownId::new(1)?;
        let payload = match control {
            Control::MainRevealed => {
                WirePayload::MainRevealed(self.fixture.host.main_identity().ok_or("Main identity")?)
            }
            Control::ExitIntent => WirePayload::ExitIntent {
                shutdown,
                intent: self.intent.ok_or("Exit intent absent")?,
                expected_code: 0,
            },
            Control::RestartToApply => WirePayload::RestartToApply,
            Control::CheckForUpdates => WirePayload::CheckForUpdates,
            Control::ExitCompleted => WirePayload::ExitCompleted(shutdown),
        };
        let header = self.header(Some(request))?;
        self.wire.send(&WireMessage { header, payload })?;
        self.pending = Some((control, request));
        Ok(true)
    }

    fn fingerprint(&self) -> String {
        let host = &self.fixture.host;
        format!(
            "{:?}|{:?}|{}|{}|{}|{}|{:?}|{}|{}|{}|{}|{}|{:?}",
            host.closing,
            host.close_save_progress(),
            host.child.shutdown_starts > 0,
            host.child.shutdown_requests.len(),
            host.child.restart_requests > 0,
            host.child.check_requests,
            host.child.update_status,
            host.child.close_requested_by_coordinator,
            self.native_close,
            self.native_retired,
            self.exited,
            self.revealed,
            self.pending,
        )
    }

    /// Virtual time from this peer's origin to a native wake instant.
    fn since_origin(&self, instant: std::time::Instant) -> Option<Duration> {
        let since = instant
            .checked_duration_since(sugarsubstitute_execution::qualification_time::anchor())?;
        let moment = Moment::from_nanos(u64::try_from(since.as_nanos()).ok()?);
        moment.checked_duration_since(self.origin).ok()
    }
}

impl LifecyclePeer for WindowPeer {
    fn turn(&mut self, now: Duration) -> HarnessResult<Turn> {
        if now > self.applied {
            self.fixture.clock.advance(now - self.applied);
            self.applied = now;
        }
        let mut progressed = false;
        while let Some(delivery) = self.wire.receive() {
            progressed = true;
            self.deliver(delivery)?;
        }
        if self.exited || self.terminated {
            return Ok(Turn { progressed, busy: false, due: None });
        }
        let maximum = NonZeroUsize::MIN.saturating_add(31);
        let _frame = self.fixture.host.poll_frame(maximum)?;
        while let Some(command) = self.fixture.host.take_command()? {
            if matches!(command, DesktopStartupCommand::Close) {
                self.wire.log().note("window: the native window closes");
                self.native_close = true;
            }
        }
        if self.native_close && !self.native_retired {
            self.fixture.host.request_shutdown()?;
            let run = self.fixture.host.run_key();
            self.fixture.host.observe(DesktopStartupObservation::NativeRetired(run))?;
            self.native_retired = true;
        }
        self.queue_controls();
        progressed |= self.send_controls()?;
        if self.completed_done && !self.exited && self.fixture.host.poll_shutdown()? {
            self.fixture.host.finish_shutdown()?;
            self.exited = true;
            self.wire.signal(Delivery::Exited(0))?;
        }
        let fingerprint = self.fingerprint();
        progressed |= fingerprint != self.last;
        self.last = fingerprint;
        let host = &self.fixture.host;
        let busy = host.closing && !host.close_save_settled();
        let due = <StartupHost<crate::startup::platform_tests::Platform> as StartupWindowContent>::next_deadline(host).and_then(|instant| self.since_origin(instant));
        Ok(Turn { progressed, busy, due })
    }
}

/// The same start, observed only until the page began loading: the window was never revealed.
fn loading_session(
    host: &StartupHost<crate::startup::platform_tests::Platform>,
) -> HarnessResult<StartupSession> {
    let launch = host.launch_id();
    let origin = LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?;
    let mut session = StartupSession::new(
        launch,
        Generation::new(1)?,
        StartupConfiguration::new(
            ConnectionConfiguration::Attach(origin),
            ProfileId::new("closing-test".into())?,
            Presentation::new("en", Theme::Dark, true)?,
        ),
    );
    for observation in [
        StartupObservation::SplashPresented,
        StartupObservation::ConnectionReady(origin),
        StartupObservation::FrontendNavigationStarted(Generation::new(1)?),
    ] {
        session.observe(StartupEvent::new(launch, observation))?;
    }
    if session.phase() == StartupPhase::Running {
        return Err("A loading window is not running yet".into());
    }
    Ok(session)
}
