// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Show a crash report from an authenticated reporter session in the Fluent report window.
//!
//! The session content owns the incident, its durable acknowledgement and every export; this
//! owner fences each user intent to the exact visible view, relays only actual native facts,
//! and keeps the window's retirement separate from the content's settlement. The window
//! closes only after the content has durably acknowledged a dismissal, or after a failure.
//! Once the window has actually retired the content is told so and settles behind it.
use super::{ReportCommand, ReportControl, ReportOwner, ReportPresentation, shown};
use crate::installer::native::CopyFailure;
use crate::report::{
    NativeReportView, ReportRequestId, ReportSessionContent, ReportSurfaceError,
    ReportSurfaceFailure, ReportSurfaceObservation, ReportSurfaceOptions, ReportSurfaceReceipt,
    ReportSurfaceUpdate, SaveSelection,
    state::{Intent, State},
};
use std::{
    error::Error,
    fmt,
    num::NonZeroUsize,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::{
    execution::{CompletionWake, MemoryClaim, MemoryLease},
    incident::{
        ExportSelection, ReportAction as SessionAction, ReportBinding,
        ReportCommand as SessionCommand, ReportFailure,
    },
    localization::{Messages, incident::ReportMessages},
};
use sugarsubstitute_ui::report::{
    ReportAction, ReportContent, ReportKind, ReportSeverity, ReportStatus,
};

/// Content still unsettled this long after close keeps its custody and the report fails.
const SHUTDOWN_BOUND: Duration = Duration::from_secs(15);
/// Session decisions drained per window turn.
const POLL_BOUND: usize = 32;

/// Show `content`'s report until its dismissal is durably acknowledged, the window has retired
/// and the content has settled. Runs on the thread that will own the report window, never from
/// inside another window's event callback.
/// # Errors
/// Preserves content, presentation and retirement causes. Content that has not settled within
/// fifteen seconds of close keeps every resource it holds and the report fails.
pub fn run_report_surface<T: ReportSessionContent + 'static>(
    options: ReportSurfaceOptions,
    content: T,
) -> Result<ReportSurfaceReceipt, ReportSurfaceError> {
    validate_content_capacity::<T>()?;
    let presentation = ReportPresentation::crash(options.copy, options.view.text().to_owned());
    let mut owner = CrashReport::new(options, content, Box::new(Instant::now));
    let shown = super::show_report(presentation, &mut owner);
    owner.receipt(shown)
}

/// Refuse a content box larger than the window's fixed dispatch reservation before any
/// start, bootstrap or native effect.
/// # Errors
/// Rejects oversized authored session metadata.
pub(in crate::report) fn validate_content_capacity<T>() -> Result<(), ReportSurfaceError> {
    if size_of::<CrashReport<T>>() as u64 > ReportSurfaceOptions::DISPATCH_STORAGE_BYTES {
        return Err(ReportSurfaceError::rejected("report_dispatch_capacity"));
    }
    Ok(())
}

/// Report owner for one reporter session.
pub(super) struct CrashReport<C> {
    /// Taken by inish: finished and dropped once settled, otherwise kept for the process.
    content: Option<C>,
    state: State,
    view: NativeReportView,
    words: &'static ReportMessages,
    lease: MemoryLease,
    _dispatch: MemoryClaim,
    now: Box<dyn Fn() -> Instant>,
    /// The pending Copy or Export request the window is completing.
    copying: Option<(ReportRequestId, ReportBinding)>,
    exporting: Option<(ReportRequestId, ReportBinding, ExportSelection)>,
    /// A committed replacement view and the details revision that shows it.
    replacing: Option<(ReportBinding, u64)>,
    details_revision: u64,
    /// This owner chose to close: durable acknowledgement or its own failure.
    closing_chosen: bool,
    close_deadline: Option<Instant>,
    window_retired: bool,
    settled: bool,
    finished: bool,
    failure: Option<ReportSurfaceError>,
}

impl<C: ReportSessionContent> CrashReport<C> {
    pub(super) fn new(
        options: ReportSurfaceOptions,
        content: C,
        now: Box<dyn Fn() -> Instant>,
    ) -> Self {
        let ReportSurfaceOptions { view, copy, lease, dispatch_storage } = options;
        Self {
            content: Some(content),
            state: State::new(view.binding(), view.restart()),
            view,
            words: copy.report,
            lease,
            _dispatch: dispatch_storage,
            now,
            copying: None,
            exporting: None,
            replacing: None,
            details_revision: 0,
            closing_chosen: false,
            close_deadline: None,
            window_retired: false,
            settled: false,
            finished: false,
            failure: None,
        }
    }

    fn session(&mut self) -> Result<&mut C, ReportSurfaceError> {
        self.content.as_mut().ok_or_else(|| ReportSurfaceError::rejected("report_session_finished"))
    }

    fn retain(&mut self, cause: ReportSurfaceError) {
        if self.failure.is_none() {
            self.failure = Some(cause);
        } else {
            tracing::error!(operation = cause.operation(), error = %cause,
                "Additional crash report failure");
        }
    }

    /// Seal admission and ask the content to shut down, once.
    fn begin_close(&mut self) {
        if self.close_deadline.is_some() {
            return;
        }
        self.state.shutdown();
        let now = (self.now)();
        self.close_deadline = Some(now.checked_add(SHUTDOWN_BOUND).unwrap_or(now));
        if let Err(cause) = self.session().and_then(C::request_shutdown) {
            self.retain(cause);
        }
    }

    /// Retain `cause`, tell the content presentation failed and close the window.
    fn fail(&mut self, cause: ReportSurfaceError, report: Option<&mut ReportControl<'_>>) {
        self.retain(cause);
        let failed = ReportSurfaceObservation::Failed {
            binding: self.state.binding(),
            failure: ReportFailure::Presentation,
        };
        if let Err(cause) = self.session().and_then(|session| session.observe(failed)) {
            self.retain(cause);
        }
        self.closing_chosen = true;
        self.begin_close();
        if let Some(report) = report {
            report.close();
        }
    }

    /// Mirror the fence: every command waits while a request, refresh or close is outstanding.
    fn project(&self, report: &mut ReportControl<'_>) {
        let actionable = self.state.actionable();
        for action in [ReportAction::Copy, ReportAction::Export, ReportAction::Close] {
            report.set_command(action, shown(actionable));
        }
        report.set_command(ReportAction::IncludeDump, shown(actionable));
        let restart = self.state.restart().allows_restart();
        report.set_command(
            ReportAction::Restart,
            sugarsubstitute_ui::report::CommandState { visible: restart, enabled: actionable },
        );
    }

    fn status(report: &mut ReportControl<'_>, text: &str, error: bool) {
        report.set_status(Some(ReportStatus { text: text.into(), error }));
    }

    fn request(&mut self, intent: Intent) -> Option<(ReportRequestId, ReportBinding)> {
        let request = self.state.request(self.state.binding(), intent).ok()?;
        Some((request.id, request.binding))
    }

    fn act(&mut self, action: SessionAction) -> Result<(), ReportSurfaceError> {
        if let Some((id, binding)) = self.request(Intent::Action(action)) {
            self.session()?.action(id, SessionCommand::new(binding, action))?;
        }
        Ok(())
    }

    fn apply(
        &mut self,
        update: ReportSurfaceUpdate,
        report: &mut ReportControl<'_>,
    ) -> Result<(), ReportSurfaceError> {
        let words = self.words;
        match update {
            ReportSurfaceUpdate::None => {}
            ReportSurfaceUpdate::ReplaceView { old, current, view } => {
                if view.binding() != current {
                    return Err(ReportSurfaceError::rejected("report_view_payload_binding"));
                }
                self.state.preflight_replace(old, current)?;
                self.details_revision += 1;
                report.replace_details(view.text().to_owned());
                self.replacing = Some((current, self.details_revision));
                self.view = view;
            }
            ReportSurfaceUpdate::CopyAuthorized { request, binding, counter, outcome } => {
                let pending = self.state.matching_request(request, binding)?;
                if pending.intent != Intent::Action(SessionAction::CopyText) {
                    return Err(ReportSurfaceError::rejected("report_copy_action_kind"));
                }
                if outcome.is_ok() {
                    self.copying = Some((request, binding));
                    report.copy_details(Some(counter));
                } else {
                    self.retain(ReportSurfaceError::rejected("report_copy_host_rejected"));
                    Self::status(report, words.copy_failed, true);
                    self.state.completed_request();
                    self.session()?.observe(ReportSurfaceObservation::CopyFinished {
                        request,
                        binding,
                        outcome: Err(ReportSurfaceFailure::Clipboard),
                    })?;
                }
            }
            ReportSurfaceUpdate::ActionResult { request, binding, outcome } => {
                let pending = self.state.matching_request(request, binding)?;
                match pending.intent {
                    Intent::Action(SessionAction::CopyText) => {
                        return Err(ReportSurfaceError::rejected("report_copy_counter_missing"));
                    }
                    Intent::Export(_) => {
                        let ok = outcome.is_ok();
                        Self::status(
                            report,
                            if ok { words.exported } else { words.export_failed },
                            !ok,
                        );
                        self.state.completed_request();
                    }
                    Intent::Action(_) => {
                        if outcome.is_err() {
                            Self::status(report, words.ack_failed, true);
                            self.state.completed_request();
                        }
                    }
                }
            }
            ReportSurfaceUpdate::RestartUnavailable { request, binding } => {
                self.state.restart_unavailable(request, binding)?;
                Self::status(report, words.restart_unavailable, true);
            }
            ReportSurfaceUpdate::CloseAfterDurableAck { binding, acknowledgement } => {
                self.state.durable_close(binding, acknowledgement)?;
                self.closing_chosen = true;
                self.begin_close();
                report.close();
            }
        }
        Ok(())
    }

    fn turn(&mut self, report: &mut ReportControl<'_>) -> Result<(), ReportSurfaceError> {
        if !self.state.closing() {
            let maximum = NonZeroUsize::new(POLL_BOUND)
                .ok_or_else(|| ReportSurfaceError::rejected("report_poll_bound"))?;
            let update = self.session()?.poll(maximum)?;
            self.apply(update, report)?;
        }
        self.project(report);
        Ok(())
    }

    fn export(&mut self, destination: &Path) -> Result<(), ReportSurfaceError> {
        let Some((request, binding, content)) = self.exporting.take() else {
            return Ok(());
        };
        let units = destination_units(destination);
        if units.is_empty() || units.len() > SaveSelection::MAX_UNITS {
            return self
                .session()?
                .observe(ReportSurfaceObservation::SaveCancelled { request, binding });
        }
        let claim = self
            .lease
            .claim((units.len() * 2) as u64)
            .map_err(|cause| ReportSurfaceError::new("report_save_selection_credit", cause))?;
        self.session()?.observe(ReportSurfaceObservation::SaveSelected(SaveSelection {
            request,
            binding,
            destination: units,
            content,
            // The shell asks before a selection replaces an existing file.
            overwrite_consented: true,
            _claim: claim,
        }))
    }

    /// The receipt of an actually retired, settled session; otherwise the retained cause.
    pub(super) fn receipt(
        mut self,
        shown: Result<(), Box<dyn Error>>,
    ) -> Result<ReportSurfaceReceipt, ReportSurfaceError> {
        if let Err(cause) = shown {
            self.retain(ReportSurfaceError::new("report_window", WindowFailure(cause.to_string())));
        }
        if let Some(cause) = self.failure.take() {
            return Err(cause);
        }
        self.state.receipt()
    }
}

/// Selected destination units without a terminator.
fn destination_units(destination: &Path) -> Vec<u16> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        destination.as_os_str().encode_wide().collect()
    }
    #[cfg(not(windows))]
    {
        destination.to_string_lossy().encode_utf16().collect()
    }
}

/// A report window failure, carried as its retained description.
#[derive(Debug)]
struct WindowFailure(String);
impl fmt::Display for WindowFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for WindowFailure {}

impl<C: ReportSessionContent> ReportOwner for CrashReport<C> {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        if let Err(cause) = self.session().and_then(|session| session.start(wake)) {
            self.fail(cause, None);
        }
        Ok(())
    }
    fn command(
        &mut self,
        command: ReportCommand,
        report: &mut ReportControl<'_>,
    ) -> Result<(), Box<dyn Error>> {
        let result = match command {
            ReportCommand::Action(ReportAction::Close) => self.act(SessionAction::Dismiss),
            ReportCommand::Action(ReportAction::Restart) => {
                self.act(SessionAction::DismissAndRequestRestart)
            }
            ReportCommand::Export { destination, .. } => self.export(&destination),
            ReportCommand::Action(_) => Ok(()),
        };
        match result {
            Ok(()) => self.project(report),
            Err(cause) => self.fail(cause, Some(report)),
        }
        Ok(())
    }
    fn poll(&mut self, report: &mut ReportControl<'_>) -> Result<(), Box<dyn Error>> {
        if let Err(cause) = self.turn(report) {
            self.fail(cause, Some(report));
        }
        Ok(())
    }
    fn copy(&mut self, report: &mut ReportControl<'_>) -> Result<bool, Box<dyn Error>> {
        if let Err(cause) = self.act(SessionAction::CopyText) {
            self.fail(cause, Some(report));
        }
        self.project(report);
        Ok(false)
    }
    fn copied(
        &mut self,
        outcome: Result<(), CopyFailure>,
        report: &mut ReportControl<'_>,
    ) -> Result<(), Box<dyn Error>> {
        let Some((request, binding)) = self.copying.take() else {
            return Ok(());
        };
        let outcome = match outcome {
            Ok(()) => Ok(()),
            Err(CopyFailure::Expired) => {
                self.fail(
                    ReportSurfaceError::rejected("report_copy_original_counter_expired"),
                    Some(report),
                );
                return Ok(());
            }
            Err(CopyFailure::Transfer(_)) => {
                self.retain(ReportSurfaceError::rejected("report_clipboard_transfer"));
                Err(ReportSurfaceFailure::Clipboard)
            }
        };
        self.state.completed_request();
        let finished = ReportSurfaceObservation::CopyFinished { request, binding, outcome };
        if let Err(cause) = self.session().and_then(|session| session.observe(finished)) {
            self.fail(cause, Some(report));
        }
        self.project(report);
        Ok(())
    }
    fn export_requested(
        &mut self,
        include_dump: bool,
        report: &mut ReportControl<'_>,
    ) -> Result<bool, Box<dyn Error>> {
        let content = if include_dump {
            ExportSelection::TextAndNativeDump
        } else {
            ExportSelection::TextOnly
        };
        let admitted = self.request(Intent::Export(content));
        self.exporting = admitted.map(|(request, binding)| (request, binding, content));
        self.project(report);
        Ok(admitted.is_some())
    }
    fn export_abandoned(&mut self, report: &mut ReportControl<'_>) -> Result<(), Box<dyn Error>> {
        if let Some((request, binding, _)) = self.exporting.take() {
            self.state.completed_request();
            if let Err(cause) = self.session().and_then(|session| {
                session.observe(ReportSurfaceObservation::SaveCancelled { request, binding })
            }) {
                self.fail(cause, Some(report));
            }
        }
        self.project(report);
        Ok(())
    }
    fn presented(&mut self, details_revision: u64) -> Result<(), Box<dyn Error>> {
        let observation = if !self.state.is_presented() {
            match self.state.presented() {
                Ok(()) => Some(ReportSurfaceObservation::Presented(self.state.binding())),
                Err(cause) => {
                    self.fail(cause, None);
                    return Ok(());
                }
            }
        } else if let Some((current, revision)) = self.replacing
            && details_revision >= revision
        {
            self.replacing = None;
            self.state.replaced(current, self.view.restart());
            Some(ReportSurfaceObservation::ViewReplaced(current))
        } else {
            None
        };
        if let Some(observation) = observation
            && let Err(cause) = self.session().and_then(|session| session.observe(observation))
        {
            self.fail(cause, None);
        }
        Ok(())
    }
    fn close(&mut self) -> Result<(), Box<dyn Error>> {
        // The window closes on its own only after a native or rendering failure.
        if !self.closing_chosen {
            self.fail(ReportSurfaceError::rejected("report_window_closed"), None);
        }
        self.begin_close();
        Ok(())
    }
    fn window_releasable(&mut self) -> Result<bool, Box<dyn Error>> {
        Ok(true)
    }
    fn window_retired(&mut self) -> Result<(), Box<dyn Error>> {
        self.window_retired = true;
        match self.state.native_retired() {
            Ok(()) => {
                let session = self.state.binding().session();
                if let Err(cause) = self.session().and_then(|content| {
                    content.observe(ReportSurfaceObservation::NativeRetired(session))
                }) {
                    self.retain(cause);
                }
            }
            Err(cause) => self.retain(cause),
        }
        Ok(())
    }
    fn settled(&mut self) -> Result<bool, Box<dyn Error>> {
        if !self.window_retired {
            return Ok(false);
        }
        if !self.settled {
            match self.session().and_then(C::poll_shutdown) {
                Ok(settled) => self.settled = settled,
                Err(cause) => self.retain(cause),
            }
        }
        if !self.settled && self.close_deadline.is_some_and(|deadline| (self.now)() >= deadline) {
            return Err(Box::new(ReportSurfaceError::rejected("report_native_shutdown_deadline")));
        }
        Ok(self.settled)
    }
    fn finish(&mut self) -> Result<(), Box<dyn Error>> {
        if self.finished {
            return Ok(());
        }
        self.finished = true;
        let Some(mut content) = self.content.take() else {
            return Ok(());
        };
        if !self.settled {
            self.retain(ReportSurfaceError::rejected("report_native_shutdown_deadline"));
            // Unsettled content keeps its workers, pipes and credit through process exit.
            std::mem::forget(content);
            return Ok(());
        }
        if let Err(cause) = content.finish_shutdown() {
            self.retain(cause);
        }
        Ok(())
    }
}

impl<C> Drop for CrashReport<C> {
    fn drop(&mut self) {
        // Content that never reached a settled finish keeps its custody through process exit.
        if let Some(content) = self.content.take() {
            std::mem::forget(content);
        }
    }
}

impl ReportPresentation {
    /// A previous run ended abnormally: heading, explanation and the full redacted report.
    pub(in crate::report) fn crash(copy: &'static Messages, details: String) -> Self {
        Self::new(
            ReportContent {
                kind: ReportKind::Crash,
                severity: ReportSeverity::Error,
                window_title: copy.report.title.to_owned(),
                title: copy.report.title.to_owned(),
                message: copy.report.explanation.to_owned(),
                summary: Vec::new(),
                has_details: true,
            },
            Some(details),
            copy,
        )
    }
}

#[cfg(test)]
#[path = "crash_tests.rs"]
mod tests;
