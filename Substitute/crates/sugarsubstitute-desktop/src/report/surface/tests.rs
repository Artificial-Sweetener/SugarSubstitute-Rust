// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive the report lifetime exactly as the form window does, with fake owners and clocks.
use super::lifetime::{MEASURE_LINES, PAINT_STEPS, ReportLifetime};
use super::*;
use crate::installer::native::{Event, Lifetime};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
use sugarsubstitute_application::instance_conflict::Phase;
use sugarsubstitute_contracts::{
    execution::{ExecutionError, Moment},
    instance_conflict::{ConflictAction, ConflictController, ConflictPhase, ConflictView},
    localization::messages,
    presentation::FrameShape,
};
use sugarsubstitute_ui::{form::Input, report::ReportKind};

type TestResult = std::result::Result<(), Box<dyn Error>>;
type Log = Rc<RefCell<Vec<ReportCommand>>>;

#[derive(Default)]
struct Wake(AtomicUsize);
impl CompletionWake for Wake {
    fn wake(&self) -> std::result::Result<(), ExecutionError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// Records every command the surface forwards; Close ends the report.
struct Recorder(Log);
impl ReportOwner for Recorder {
    fn command(&mut self, command: ReportCommand, report: &mut ReportControl<'_>) -> TestResult {
        if command == ReportCommand::Action(ReportAction::Close) {
            report.close();
        }
        self.0.borrow_mut().push(command);
        Ok(())
    }
}

fn report_text(lines: usize) -> String {
    (0..lines).map(|line| format!("cause {line}: the backend did not answer\n")).collect()
}

fn startup(lines: usize) -> ReportPresentation {
    ReportPresentation::startup_failure(
        messages("en"),
        "SugarSubstitute",
        "Startup failed",
        report_text(lines),
    )
}

fn started(lifetime: &mut ReportLifetime<'_>) -> std::result::Result<Arc<Wake>, Box<dyn Error>> {
    let wake = Arc::new(Wake::default());
    lifetime.input(Event::SystemTheme(sugarsubstitute_contracts::configuration::Theme::Dark))?;
    lifetime.work_area(1040.0);
    lifetime.start(wake.clone())?;
    lifetime.resize(FrameShape::new(720, 320, 1.0)?)?;
    Ok(wake)
}

/// One window turn: advance, then apply the preferred size as the window does. `grow` adds
/// rows the way a window with a hidden frame can end up larger than requested.
fn turn(lifetime: &mut ReportLifetime<'_>, at: Duration, grow: u32) -> TestResult {
    lifetime.advance(at)?;
    if let Some([width, height]) = lifetime.preferred_size() {
        lifetime.resize(FrameShape::new(width, height + grow, 1.0)?)?;
    }
    Ok(())
}

/// Drive turns until a new frame exists, checking that every slice stayed bounded.
fn next_frame(
    lifetime: &mut ReportLifetime<'_>,
    at: Duration,
) -> std::result::Result<usize, Box<dyn Error>> {
    next_frame_in(lifetime, at, 0)
}

fn next_frame_in(
    lifetime: &mut ReportLifetime<'_>,
    at: Duration,
    grow: u32,
) -> std::result::Result<usize, Box<dyn Error>> {
    let before = lifetime.page().map(|page| page.frame.identity());
    for turn_index in 1..4000 {
        turn(lifetime, at, grow)?;
        let work = lifetime.last_slice;
        assert!(work.paint_steps <= PAINT_STEPS, "{work:?}");
        assert!(work.measured_lines <= MEASURE_LINES, "{work:?}");
        assert!(!(work.measured_layout && (work.paint_steps > 0 || work.measured_lines > 0)));
        if lifetime.page().map(|page| page.frame.identity()) != before {
            return Ok(turn_index);
        }
    }
    Err("no new frame".into())
}

fn visible(lifetime: &ReportLifetime<'_>) -> Vec<ReportAction> {
    lifetime.page().map_or_else(Vec::new, |page| {
        page.shell
            .iter()
            .filter(|control| control.availability.visible)
            .filter_map(|control| ReportAction::from_id(control.id))
            .collect()
    })
}

fn invoke(lifetime: &mut ReportLifetime<'_>, action: ReportAction) -> TestResult {
    lifetime.input(Event::Control(action.id(), Input::Invoke))
}

#[test]
fn startup_report_prepares_in_bounded_slices_and_offers_copy_and_close() -> TestResult {
    let log = Log::default();
    let mut owner = Recorder(log.clone());
    let mut lifetime = ReportLifetime::new(startup(40), &mut owner)?;
    let wake = started(&mut lifetime)?;
    let turns = next_frame(&mut lifetime, Duration::ZERO)?;
    assert!(turns > 3, "the first frame spans several slices, not one long turn");
    assert!(wake.0.load(Ordering::SeqCst) >= turns, "each unfinished slice wakes the next");
    let page = lifetime.page().ok_or("page")?;
    let size = [page.frame.shape().width(), page.frame.shape().height()];
    assert_eq!(lifetime.preferred_size(), Some(size), "the window adopts the finished frame");
    let shown = visible(&lifetime);
    assert!(shown.contains(&ReportAction::Copy) && shown.contains(&ReportAction::Close));
    assert!(!shown.contains(&ReportAction::Export) && !shown.contains(&ReportAction::Restart));

    invoke(&mut lifetime, ReportAction::Copy)?;
    assert_eq!(
        lifetime.clipboard().map(|copy| copy.text),
        Some(report_text(40)),
        "Copy takes the whole report"
    );
    assert!(lifetime.clipboard().is_none());
    lifetime.copied(Ok(()))?;
    next_frame(&mut lifetime, Duration::ZERO)?;

    assert!(!lifetime.close_requested());
    invoke(&mut lifetime, ReportAction::Dismiss)?;
    assert_eq!(*log.borrow(), vec![ReportCommand::Action(ReportAction::Close)]);
    assert!(lifetime.close_requested());
    Ok(())
}

#[test]
fn details_reveal_grows_the_window_frame_by_frame_on_the_supplied_clock() -> TestResult {
    let mut owner = Recorder(Log::default());
    let mut lifetime = ReportLifetime::new(startup(400), &mut owner)?;
    started(&mut lifetime)?;
    let start = Duration::from_secs(5);
    next_frame(&mut lifetime, start)?;
    let collapsed = lifetime.preferred_size().ok_or("size")?[1];
    invoke(&mut lifetime, ReportAction::ToggleDetails)?;
    let mut heights = vec![collapsed];
    let mut at = start;
    while heights.last() != Some(&(collapsed + 272)) {
        assert!(at < start + sugarsubstitute_ui::report::EXPAND_DURATION * 2, "{heights:?}");
        at += Duration::from_millis(40);
        next_frame(&mut lifetime, at)?;
        heights.push(lifetime.preferred_size().ok_or("size")?[1]);
    }
    assert!(heights.windows(2).all(|pair| pair[0] <= pair[1]), "{heights:?}");
    assert!(heights.iter().any(|h| *h > collapsed && *h < collapsed + 272), "{heights:?}");
    assert_eq!(*heights.last().ok_or("height")?, collapsed + 272);
    Ok(())
}

#[test]
fn large_reports_measure_behind_the_first_frame_without_long_turns() -> TestResult {
    let mut owner = Recorder(Log::default());
    let text = "x".repeat(80) + "\n";
    let presentation = ReportPresentation::startup_failure(
        messages("en"),
        "SugarSubstitute",
        "Startup failed",
        text.repeat(3000),
    );
    let mut lifetime = ReportLifetime::new(presentation, &mut owner)?;
    started(&mut lifetime)?;
    next_frame(&mut lifetime, Duration::ZERO)?;
    let mut turns = 0;
    while lifetime.last_slice.measured_lines > 0 || turns == 0 {
        lifetime.advance(Duration::ZERO)?;
        assert!(lifetime.last_slice.measured_lines <= MEASURE_LINES);
        turns += 1;
    }
    assert!(turns >= 3000 / MEASURE_LINES, "3000 lines take many short turns: {turns}");
    Ok(())
}

/// The real conflict phase on an injected clock, as the application controller drives it.
struct Conflict {
    phase: Phase,
    now: Rc<RefCell<Moment>>,
    chosen: Vec<ConflictAction>,
}
impl ConflictController for Conflict {
    fn start(&mut self, _: Arc<dyn CompletionWake>) {
        self.phase = Phase::Ready;
    }
    fn poll(&mut self) -> ConflictView {
        self.phase.view(*self.now.borrow())
    }
    fn choose(&mut self, action: ConflictAction) {
        self.chosen.push(action);
        let _ = self.phase.choose(action, *self.now.borrow());
        if action == ConflictAction::Cancel {
            self.phase = Phase::Done(false);
        }
    }
}

fn unresponsive(
    controller: &mut Conflict,
    script: impl FnOnce(&mut ReportLifetime<'_>, &Rc<RefCell<Moment>>) -> TestResult,
) -> TestResult {
    let now = controller.now.clone();
    let copy = messages("en");
    let mut owner = ConflictReport::new(controller, copy.conflict);
    let mut lifetime = ReportLifetime::new(
        ReportPresentation::unresponsive_copy(copy, "SugarSubstitute"),
        &mut owner,
    )?;
    started(&mut lifetime)?;
    next_frame(&mut lifetime, Duration::ZERO)?;
    script(&mut lifetime, &now)
}

#[test]
fn unresponsive_window_question_offers_close_it_and_start_and_cancel_in_plain_words() -> TestResult
{
    let origin = Moment::from_nanos(0);
    let mut controller =
        Conflict { phase: Phase::Checking, now: Rc::new(RefCell::new(origin)), chosen: Vec::new() };
    unresponsive(&mut controller, |lifetime, now| {
        let shown = visible(lifetime);
        assert!(shown.contains(&ReportAction::Close) && shown.contains(&ReportAction::Cancel));
        assert_eq!(shown.iter().filter(|a| **a != ReportAction::Dismiss).count(), 2);
        let words = messages("en").conflict;
        let close = lifetime
            .page()
            .and_then(|page| page.shell.iter().find(|c| c.id == ReportAction::Close.id()).cloned())
            .ok_or("close")?;
        assert_eq!(close.label, words.close, "Close it and start");
        invoke(lifetime, ReportAction::Close)?;
        *now.borrow_mut() = origin.checked_add(Duration::from_secs(1))?;
        next_frame(lifetime, Duration::ZERO)?;
        assert!(
            lifetime
                .page()
                .and_then(|p| p.shell.iter().find(|c| c.id == ReportAction::Close.id()).cloned())
                .is_some_and(|c| !c.availability.enabled),
            "while the previous window closes there is nothing to press but Cancel"
        );
        *now.borrow_mut() = origin.checked_add(Duration::from_secs(16))?;
        next_frame(lifetime, Duration::ZERO)?;
        invoke(lifetime, ReportAction::Close)?;
        Ok(())
    })?;
    assert_eq!(
        controller.chosen,
        vec![ConflictAction::Close, ConflictAction::Force],
        "the same button ends a window that still won't close"
    );
    Ok(())
}

#[test]
fn unresponsive_window_caption_close_and_escape_mean_cancel() -> TestResult {
    let mut controller = Conflict {
        phase: Phase::Checking,
        now: Rc::new(RefCell::new(Moment::from_nanos(0))),
        chosen: Vec::new(),
    };
    unresponsive(&mut controller, |lifetime, _| {
        invoke(lifetime, ReportAction::Dismiss)?;
        turn(lifetime, Duration::ZERO, 0)?;
        assert!(lifetime.close_requested(), "the controller finished, so the report ends");
        assert!(lifetime.settled()?);
        Ok(())
    })?;
    assert_eq!(controller.chosen, vec![ConflictAction::Cancel]);
    let mut controller = Conflict {
        phase: Phase::Checking,
        now: Rc::new(RefCell::new(Moment::from_nanos(0))),
        chosen: Vec::new(),
    };
    unresponsive(&mut controller, |lifetime, _| {
        lifetime.close()?;
        assert!(lifetime.settled()?);
        Ok(())
    })?;
    assert_eq!(controller.chosen, vec![ConflictAction::Cancel], "Escape closes the window");
    Ok(())
}

/// A controller that reports one fixed phase.
struct Fixed(ConflictPhase);
impl ConflictController for Fixed {
    fn start(&mut self, _: Arc<dyn CompletionWake>) {}
    fn poll(&mut self) -> ConflictView {
        ConflictView { phase: self.0 }
    }
    fn choose(&mut self, _: ConflictAction) {}
}

/// Regression: the words come from the controller's typed phase in the surface's own locale,
/// so a failure is shown as a failure in every language, not only when English text matches.
#[test]
fn every_conflict_phase_shows_its_own_words_in_the_surface_locale() -> TestResult {
    for copy in sugarsubstitute_contracts::localization::LOCALES {
        let (locale, words) = (copy.id, copy.conflict);
        let cases = [
            (ConflictPhase::Checking, words.prompt, Some(words.checking), true),
            (ConflictPhase::AwaitingDecision, words.prompt, None, true),
            (ConflictPhase::Closing, words.prompt, Some(words.waiting), true),
            (ConflictPhase::ForceOffered, words.force_prompt, None, true),
            (ConflictPhase::Forcing, words.prompt, Some(words.waiting), true),
            (ConflictPhase::Failed, words.failed, None, true),
            (ConflictPhase::Retiring { continuing: true }, words.prompt, Some(words.cleanup), true),
            (ConflictPhase::Retiring { continuing: false }, words.prompt, None, true),
            (ConflictPhase::Done, words.prompt, None, false),
            (ConflictPhase::AwaitingExit, words.cleanup, None, true),
        ];
        for (phase, message, progress, cancellable) in cases {
            let mut controller = Fixed(phase);
            let mut owner = ConflictReport::new(&mut controller, words);
            let (mut text, mut state, mut closed, mut changed) =
                (String::new(), ReportState::new(ReportKind::UnresponsiveCopy), false, false);
            let mut control = ReportControl {
                message: &mut text,
                state: &mut state,
                closed: &mut closed,
                changed: &mut changed,
                requests: &mut ControlRequests::default(),
            };
            owner.poll(&mut control)?;
            let view = ConflictView { phase };
            assert_eq!(control.message.as_str(), message, "{locale} {phase:?}");
            assert_eq!(
                control.state.status,
                progress.map(|text| ReportStatus { text: text.into(), error: false }),
                "{locale} {phase:?}"
            );
            let close = control.state.command(ReportAction::Close);
            assert_eq!(close, shown(view.close_enabled() || view.force_enabled()), "{phase:?}");
            assert_eq!(control.state.command(ReportAction::Cancel), shown(cancellable));
            assert_eq!(*control.closed, view.done(), "{phase:?}");
        }
    }
    Ok(())
}

/// Regression: a window whose client ends up larger than requested (tao adds the hidden frame
/// of an undecorated window) must still receive a frame of its own size, or nothing is shown.
#[test]
fn a_window_larger_than_requested_still_receives_a_matching_frame() -> TestResult {
    let mut owner = Recorder(Log::default());
    let mut lifetime = ReportLifetime::new(startup(10), &mut owner)?;
    started(&mut lifetime)?;
    next_frame_in(&mut lifetime, Duration::ZERO, 30)?;
    for _ in 0..200 {
        if !lifetime.pending() {
            break;
        }
        turn(&mut lifetime, Duration::ZERO, 30)?;
    }
    assert!(!lifetime.pending(), "the report settles");
    let [width, height] = lifetime.preferred_size().ok_or("size")?;
    let page = lifetime.page().ok_or("page")?;
    assert_eq!((page.frame.shape().width(), page.frame.shape().height()), (width, height + 30));
    Ok(())
}

/// A 1366x768 screen at 150% is 512 logical pixels tall; with a taskbar its work area is
/// about 480. A report that fits there takes the compact details panel.
#[test]
fn a_small_work_area_takes_the_compact_details_panel_when_it_fits() -> TestResult {
    let mut owner = Recorder(Log::default());
    let mut lifetime = ReportLifetime::new(startup(400), &mut owner)?;
    lifetime.work_area(480.0);
    lifetime.start(Arc::new(Wake::default()))?;
    lifetime.resize(FrameShape::new(720, 320, 1.0)?)?;
    next_frame(&mut lifetime, Duration::ZERO)?;
    let collapsed = lifetime.preferred_size().ok_or("size")?[1];
    invoke(&mut lifetime, ReportAction::ToggleDetails)?;
    next_frame(&mut lifetime, Duration::from_secs(1))?;
    let page = lifetime.page().ok_or("page")?;
    assert_eq!(page.frame.shape().height(), collapsed + 172, "160 px of report text");
    assert!(page.frame.shape().height() <= 432);
    Ok(())
}

/// A message too long for that work area still presents: the window stops at the work area,
/// the body scrolls under the title bar and every command stays in the footer.
#[test]
fn a_long_message_on_a_small_work_area_scrolls_the_body_above_a_fixed_footer() -> TestResult {
    let mut owner = Recorder(Log::default());
    let copy = messages("en");
    let presentation = ReportPresentation::new(
        sugarsubstitute_ui::report::ReportContent {
            kind: sugarsubstitute_ui::report::ReportKind::StartupFailure,
            severity: sugarsubstitute_ui::report::ReportSeverity::Error,
            window_title: "SugarSubstitute".into(),
            title: "Startup failed".into(),
            message: "The backend stopped while SugarSubstitute was starting, and every retry \
                failed with the same answer from the connection. "
                .repeat(30),
            summary: Vec::new(),
            has_details: true,
        },
        Some(report_text(400)),
        copy,
    );
    let mut lifetime = ReportLifetime::new(presentation, &mut owner)?;
    lifetime.work_area(480.0);
    lifetime.start(Arc::new(Wake::default()))?;
    lifetime.resize(FrameShape::new(720, 320, 1.0)?)?;
    next_frame(&mut lifetime, Duration::ZERO)?;
    let page = lifetime.page().ok_or("page")?;
    assert_eq!(page.frame.shape().height(), 432, "limited to the work area less its margin");
    assert_eq!(page.viewport.y, 32.0, "the whole body scrolls under the title bar");
    assert!(page.content_height > page.viewport.height);
    let footer: Vec<_> = page
        .shell
        .iter()
        .filter(|c| c.availability.visible && c.bounds.y >= 432.0 - 68.0)
        .filter_map(|c| ReportAction::from_id(c.id))
        .collect();
    assert!(footer.contains(&ReportAction::Copy) && footer.contains(&ReportAction::Close));
    lifetime.input(Event::Scroll(1.0e6))?;
    next_frame(&mut lifetime, Duration::ZERO)?;
    let page = lifetime.page().ok_or("page")?;
    let toggle =
        page.controls.iter().find(|c| c.id == ReportAction::ToggleDetails.id()).ok_or("toggle")?;
    assert!(toggle.availability.visible, "scrolling the body reaches Show report");
    Ok(())
}

#[path = "update_tests.rs"]
mod update;

/// A controller that reports a scripted sequence of phases, holding the last one.
struct Script(Vec<ConflictPhase>);
impl ConflictController for Script {
    fn start(&mut self, _: Arc<dyn CompletionWake>) {}
    fn poll(&mut self) -> ConflictView {
        let phase = if self.0.len() > 1 { self.0.remove(0) } else { self.0[0] };
        ConflictView { phase }
    }
    fn choose(&mut self, _: ConflictAction) {}
}

/// While a closing previous window is waited for, continuing describes finishing the close,
/// never the unresponsive question; once the question was asked, continuing keeps its words.
#[test]
fn waiting_for_a_closing_copy_never_shows_the_question_while_continuing() -> TestResult {
    let words = sugarsubstitute_contracts::localization::LOCALES[0].conflict;
    for (script, message) in [
        (
            vec![ConflictPhase::AwaitingExit, ConflictPhase::Retiring { continuing: true }],
            words.cleanup,
        ),
        (
            vec![
                ConflictPhase::AwaitingExit,
                ConflictPhase::AwaitingDecision,
                ConflictPhase::Retiring { continuing: true },
            ],
            words.prompt,
        ),
    ] {
        let steps = script.len();
        let mut controller = Script(script);
        let mut owner = ConflictReport::new(&mut controller, words);
        let (mut text, mut state, mut closed, mut changed) =
            (String::new(), ReportState::new(ReportKind::UnresponsiveCopy), false, false);
        let mut control = ReportControl {
            message: &mut text,
            state: &mut state,
            closed: &mut closed,
            changed: &mut changed,
            requests: &mut ControlRequests::default(),
        };
        for _ in 0..steps {
            owner.poll(&mut control)?;
        }
        assert_eq!(control.message.as_str(), message);
    }
    Ok(())
}
