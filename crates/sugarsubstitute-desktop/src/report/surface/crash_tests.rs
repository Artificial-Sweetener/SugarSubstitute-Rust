// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive the crash report owner as the report window does, against a scripted session.
use super::*;
use crate::report::surface::ControlRequests;
use std::{cell::Cell, collections::VecDeque, path::PathBuf, rc::Rc};
use sugarsubstitute_contracts::{
    execution::{CounterDeadline, MemoryRelease},
    incident::{
        DurableAcknowledgement, IncidentKey, IncidentRevision, IncidentSubject, IncidentUtc,
        ReportRestartAvailability, ReportSessionId,
    },
    lifecycle::{CoordinatorEpoch, CoordinatorKey, NamespaceId},
    localization::messages,
};
use sugarsubstitute_ui::report::ReportState;

type TestResult = Result<(), Box<dyn Error>>;

struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}

/// What the session saw, in order.
#[derive(Debug, PartialEq)]
enum Seen {
    Presented,
    Replaced(u64),
    Action(SessionAction),
    CopyFinished(bool),
    Saved { units: usize, content: ExportSelection, overwrite: bool },
    SaveCancelled,
    Retired,
    Failed,
    Shutdown,
    Finished,
}

#[derive(Default)]
struct Session {
    seen: Vec<Seen>,
    updates: VecDeque<ReportSurfaceUpdate>,
    requests: Vec<ReportRequestId>,
    settles: bool,
}
impl ReportSessionContent for Rc<std::cell::RefCell<Session>> {
    fn start(&mut self, _: Arc<dyn CompletionWake>) -> Result<(), ReportSurfaceError> {
        Ok(())
    }
    fn poll(&mut self, _: NonZeroUsize) -> Result<ReportSurfaceUpdate, ReportSurfaceError> {
        Ok(self.borrow_mut().updates.pop_front().unwrap_or(ReportSurfaceUpdate::None))
    }
    fn action(
        &mut self,
        request: ReportRequestId,
        command: SessionCommand,
    ) -> Result<(), ReportSurfaceError> {
        let mut session = self.borrow_mut();
        session.requests.push(request);
        session.seen.push(Seen::Action(command.action()));
        Ok(())
    }
    fn observe(&mut self, observation: ReportSurfaceObservation) -> Result<(), ReportSurfaceError> {
        let seen = match observation {
            ReportSurfaceObservation::Presented(_) => Seen::Presented,
            ReportSurfaceObservation::ViewReplaced(binding) => {
                Seen::Replaced(binding.revision().get())
            }
            ReportSurfaceObservation::CopyFinished { outcome, .. } => {
                Seen::CopyFinished(outcome.is_ok())
            }
            ReportSurfaceObservation::SaveSelected(selection) => {
                self.borrow_mut().requests.push(selection.request());
                Seen::Saved {
                    units: selection.destination().len(),
                    content: selection.content(),
                    overwrite: selection.overwrite_consented(),
                }
            }
            ReportSurfaceObservation::SaveCancelled { .. } => Seen::SaveCancelled,
            ReportSurfaceObservation::NativeRetired(_) => Seen::Retired,
            ReportSurfaceObservation::Failed { .. } => Seen::Failed,
        };
        self.borrow_mut().seen.push(seen);
        Ok(())
    }
    fn request_shutdown(&mut self) -> Result<(), ReportSurfaceError> {
        self.borrow_mut().seen.push(Seen::Shutdown);
        Ok(())
    }
    fn poll_shutdown(&mut self) -> Result<bool, ReportSurfaceError> {
        Ok(self.borrow().settles)
    }
    fn finish_shutdown(&mut self) -> Result<(), ReportSurfaceError> {
        self.borrow_mut().seen.push(Seen::Finished);
        Ok(())
    }
}

type Shared = Rc<std::cell::RefCell<Session>>;

fn binding(revision: u64) -> Result<ReportBinding, Box<dyn Error>> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    Ok(ReportBinding::new(
        IncidentKey::new(IncidentSubject::Supervisor(owner)),
        IncidentRevision::new(revision)?,
        ReportSessionId::new(1)?,
    ))
}

fn view(
    lease: &MemoryLease,
    revision: u64,
    restart: ReportRestartAvailability,
) -> Result<NativeReportView, Box<dyn Error>> {
    Ok(NativeReportView::from_claimed(
        binding(revision)?,
        format!("report {revision}\n"),
        lease.claim(512)?,
        restart,
    )?)
}

struct Fixture {
    owner: CrashReport<Shared>,
    session: Shared,
    now: Rc<Cell<Instant>>,
    origin: Instant,
}

/// An owner whose session has presented its first view and committed revision 2, the first
/// view that admits actions.
fn fixture(restart: ReportRestartAvailability) -> Result<Fixture, Box<dyn Error>> {
    let lease = MemoryLease::new(2 * ReportSurfaceOptions::KNOWN_STORAGE_BYTES, Box::new(Release));
    let committed = view(&lease, 2, restart)?;
    let options = ReportSurfaceOptions::new(view(&lease, 1, restart)?, "en", lease)?;
    let session = Shared::default();
    let origin = Instant::now();
    let now = Rc::new(Cell::new(origin));
    let clock = now.clone();
    let mut owner = CrashReport::new(options, session.clone(), Box::new(move || clock.get()));
    let mut turn = Turn::new();
    turn.run(|report| owner.poll(report))?;
    owner.presented(0)?;
    session.borrow_mut().updates.push_back(ReportSurfaceUpdate::ReplaceView {
        old: binding(1)?,
        current: binding(2)?,
        view: committed,
    });
    turn.run(|report| owner.poll(report))?;
    owner.presented(1)?;
    assert_eq!(seen(&session), [Seen::Presented, Seen::Replaced(2)]);
    Ok(Fixture { owner, session, now, origin })
}

/// One owner turn against a report control, returning what the owner asked of the report.
struct Turn {
    message: String,
    state: ReportState,
    closed: bool,
    changed: bool,
    requests: ControlRequests,
}
impl Turn {
    fn new() -> Self {
        Self {
            message: String::new(),
            state: ReportState::new(ReportKind::Crash),
            closed: false,
            changed: false,
            requests: ControlRequests::default(),
        }
    }
    fn run<T>(
        &mut self,
        apply: impl FnOnce(&mut ReportControl<'_>) -> Result<T, Box<dyn Error>>,
    ) -> Result<T, Box<dyn Error>> {
        apply(&mut ReportControl {
            message: &mut self.message,
            state: &mut self.state,
            closed: &mut self.closed,
            changed: &mut self.changed,
            requests: &mut self.requests,
        })
    }
}

fn seen(session: &Shared) -> Vec<Seen> {
    std::mem::take(&mut session.borrow_mut().seen)
}

fn acknowledge(session: &Shared, revision: u64) -> Result<(), Box<dyn Error>> {
    session.borrow_mut().updates.push_back(ReportSurfaceUpdate::CloseAfterDurableAck {
        binding: binding(2)?,
        acknowledgement: DurableAcknowledgement::new(
            binding(2)?.incident(),
            IncidentRevision::new(revision)?,
            IncidentUtc::from_unix_seconds(1)?,
        ),
    });
    Ok(())
}

#[test]
fn close_waits_for_the_durable_acknowledgement_then_retires_the_window_before_settling()
-> TestResult {
    let Fixture { mut owner, session, .. } = fixture(ReportRestartAvailability::Available)?;
    let mut turn = Turn::new();
    turn.run(|report| owner.poll(report))?;
    assert!(turn.state.command(ReportAction::Restart).visible, "restart is offered");
    turn.run(|report| owner.command(ReportCommand::Action(ReportAction::Close), report))?;
    assert_eq!(seen(&session), [Seen::Action(SessionAction::Dismiss)]);
    assert!(!turn.state.command(ReportAction::Copy).enabled, "one request at a time");
    turn.run(|report| owner.poll(report))?;
    assert!(!turn.closed, "the window stays until the dismissal is durable");
    acknowledge(&session, 3)?;
    turn.run(|report| owner.poll(report))?;
    assert!(turn.closed);
    owner.close()?;
    assert_eq!(seen(&session), [Seen::Shutdown], "an acknowledged close is not a failure");
    assert!(owner.window_releasable()?, "the window retires before the session settles");
    assert!(!owner.settled()?, "nothing settles while the window still exists");
    owner.window_retired()?;
    assert_eq!(seen(&session), [Seen::Retired]);
    assert!(!owner.settled()?);
    session.borrow_mut().settles = true;
    assert!(owner.settled()?);
    owner.finish()?;
    assert_eq!(seen(&session), [Seen::Finished]);
    let receipt = owner.receipt(Ok(()))?;
    assert!(receipt.presented());
    assert_eq!(receipt.requests(), 1);
    assert_eq!(receipt.replacements(), 1);
    Ok(())
}

#[test]
fn restart_is_hidden_when_unavailable_and_requests_restart_when_offered() -> TestResult {
    let Fixture { mut owner, .. } = fixture(ReportRestartAvailability::Unavailable)?;
    let mut turn = Turn::new();
    turn.run(|report| owner.poll(report))?;
    assert!(!turn.state.command(ReportAction::Restart).visible);
    let Fixture { mut owner, session, .. } = fixture(ReportRestartAvailability::Available)?;
    turn.run(|report| owner.poll(report))?;
    turn.run(|report| owner.command(ReportCommand::Action(ReportAction::Restart), report))?;
    let request = *session.borrow().requests.last().ok_or("request")?;
    session
        .borrow_mut()
        .updates
        .push_back(ReportSurfaceUpdate::RestartUnavailable { request, binding: binding(2)? });
    turn.run(|report| owner.poll(report))?;
    assert!(!turn.state.command(ReportAction::Restart).visible, "a refused restart disappears");
    assert!(turn.state.command(ReportAction::Close).enabled);
    assert_eq!(
        turn.state.status.as_ref().map(|status| status.text.as_str()),
        Some(messages("en").report.restart_unavailable)
    );
    Ok(())
}

#[test]
fn copy_is_authorized_by_the_session_and_bounded_by_its_counter() -> TestResult {
    let Fixture { mut owner, session, .. } = fixture(ReportRestartAvailability::Available)?;
    let mut turn = Turn::new();
    turn.run(|report| owner.poll(report))?;
    assert!(!turn.run(|report| owner.copy(report))?, "nothing is copied before authorization");
    assert_eq!(seen(&session), [Seen::Action(SessionAction::CopyText)]);
    let request = *session.borrow().requests.last().ok_or("request")?;
    let counter = CounterDeadline::new(1_000, 10)?;
    session.borrow_mut().updates.push_back(ReportSurfaceUpdate::CopyAuthorized {
        request,
        binding: binding(2)?,
        counter,
        outcome: Ok(()),
    });
    turn.run(|report| owner.poll(report))?;
    assert_eq!(turn.requests.copy, Some(Some(counter)));
    turn.run(|report| owner.copied(Ok(()), report))?;
    assert_eq!(seen(&session), [Seen::CopyFinished(true)]);
    assert!(turn.state.command(ReportAction::Copy).enabled);
    // A copy whose counter has passed fails the report rather than copying late.
    assert!(!turn.run(|report| owner.copy(report))?);
    let request = *session.borrow().requests.last().ok_or("request")?;
    session.borrow_mut().updates.push_back(ReportSurfaceUpdate::CopyAuthorized {
        request,
        binding: binding(2)?,
        counter,
        outcome: Ok(()),
    });
    turn.run(|report| owner.poll(report))?;
    seen(&session);
    turn.run(|report| owner.copied(Err(CopyFailure::Expired), report))?;
    assert_eq!(seen(&session), [Seen::Failed, Seen::Shutdown]);
    assert!(turn.closed);
    Ok(())
}

#[test]
fn export_carries_dump_consent_and_the_selected_destination_or_its_cancellation() -> TestResult {
    let Fixture { mut owner, session, .. } = fixture(ReportRestartAvailability::Available)?;
    let mut turn = Turn::new();
    turn.run(|report| owner.poll(report))?;
    assert!(turn.run(|report| owner.export_requested(true, report))?);
    assert!(!turn.state.command(ReportAction::Export).enabled, "one export at a time");
    let destination = PathBuf::from(r"D:\reports\report.zip");
    turn.run(|report| {
        owner.command(ReportCommand::Export { destination, include_dump: true }, report)
    })?;
    assert_eq!(
        seen(&session),
        [Seen::Saved { units: 21, content: ExportSelection::TextAndNativeDump, overwrite: true }]
    );
    let request = *session.borrow().requests.last().ok_or("request")?;
    session.borrow_mut().updates.push_back(ReportSurfaceUpdate::ActionResult {
        request,
        binding: binding(2)?,
        outcome: Ok(()),
    });
    turn.run(|report| owner.poll(report))?;
    assert_eq!(
        turn.state.status.as_ref().map(|status| status.text.as_str()),
        Some(messages("en").report.exported)
    );
    assert!(turn.run(|report| owner.export_requested(false, report))?);
    turn.run(|report| owner.export_abandoned(report))?;
    assert_eq!(seen(&session), [Seen::SaveCancelled]);
    assert!(turn.state.command(ReportAction::Export).enabled);
    Ok(())
}

#[test]
fn a_replaced_view_is_reported_only_once_a_frame_showing_it_is_presented() -> TestResult {
    let Fixture { mut owner, session, .. } = fixture(ReportRestartAvailability::Available)?;
    let lease = MemoryLease::new(4096, Box::new(Release));
    let mut turn = Turn::new();
    turn.run(|report| owner.poll(report))?;
    session.borrow_mut().updates.push_back(ReportSurfaceUpdate::ReplaceView {
        old: binding(2)?,
        current: binding(3)?,
        view: view(&lease, 3, ReportRestartAvailability::Unavailable)?,
    });
    turn.run(|report| owner.poll(report))?;
    assert_eq!(turn.requests.details.as_deref(), Some("report 3\n"));
    assert!(!turn.state.command(ReportAction::Close).enabled, "refreshing views take no input");
    owner.presented(1)?;
    assert!(seen(&session).is_empty(), "a frame of the old text proves nothing");
    owner.presented(2)?;
    assert_eq!(seen(&session), [Seen::Replaced(3)]);
    turn.run(|report| owner.poll(report))?;
    assert!(turn.state.command(ReportAction::Close).enabled);
    assert!(!turn.state.command(ReportAction::Restart).visible, "the new view's restart decision");
    Ok(())
}

#[test]
fn a_window_closed_by_failure_tells_the_session_and_the_report_fails() -> TestResult {
    let Fixture { mut owner, session, .. } = fixture(ReportRestartAvailability::Available)?;
    let mut turn = Turn::new();
    turn.run(|report| owner.poll(report))?;
    owner.close()?;
    assert_eq!(seen(&session), [Seen::Failed, Seen::Shutdown]);
    owner.window_retired()?;
    session.borrow_mut().settles = true;
    assert!(owner.settled()?);
    owner.finish()?;
    let failure = owner.receipt(Ok(())).err().ok_or("a failed window has no receipt")?;
    assert_eq!(failure.operation(), "report_window_closed");
    Ok(())
}

#[test]
fn a_session_unsettled_fifteen_seconds_after_close_keeps_its_custody_and_fails() -> TestResult {
    let Fixture { mut owner, session, now, origin } =
        fixture(ReportRestartAvailability::Available)?;
    let mut turn = Turn::new();
    turn.run(|report| owner.poll(report))?;
    turn.run(|report| owner.command(ReportCommand::Action(ReportAction::Close), report))?;
    acknowledge(&session, 3)?;
    turn.run(|report| owner.poll(report))?;
    owner.close()?;
    owner.window_retired()?;
    now.set(origin + Duration::from_secs(14));
    assert!(!owner.settled()?);
    now.set(origin + Duration::from_secs(15));
    assert!(owner.settled().is_err(), "settlement ends at the bound");
    owner.finish()?;
    assert!(!seen(&session).contains(&Seen::Finished), "unsettled content is never finished");
    let failure = owner.receipt(Ok(())).err().ok_or("an unsettled report has no receipt")?;
    assert_eq!(failure.operation(), "report_native_shutdown_deadline");
    Ok(())
}
