// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Apply fenced decisions to retained native resources on one UI thread.
use super::super::{
    NativeReportView, ReportSessionContent, ReportSurfaceError, ReportSurfaceObservation,
    ReportSurfaceOptions, ReportSurfaceUpdate,
    state::{Intent, State},
};
use super::{
    callback::{Event, Input},
    controls::Controls,
    copy, save_dialog,
};
use std::{
    num::NonZeroUsize,
    rc::Rc,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::{
    execution::MemoryLease,
    incident::{ReportAction, ReportCommand, ReportFailure},
    localization::incident::ReportMessages,
};
use tao::{
    event_loop::EventLoopProxy,
    window::{Window, WindowId},
};
use windows::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{IsWindow, IsWindowVisible},
};
pub(super) struct Session {
    pub(super) state: State,
    pub(super) controls: Option<Controls>,
    pub(super) window: Option<Rc<Window>>,
    pub(super) window_id: Option<WindowId>,
    pub(super) hwnd: HWND,
    pub(super) destroyed: bool,
    pub(super) native_retired: bool,
    pub(super) settled: bool,
    pub(super) failure: Option<ReportSurfaceError>,
    pub(super) close_deadline: Option<Instant>,
    pub(super) picker: Option<save_dialog::UnretiredDialog>,
    pub(super) prepared_save: Option<super::modal::PreparedSave>,
    retirement_attempted: bool,
    view: NativeReportView,
    pub(super) labels: &'static ReportMessages,
}
#[cfg(test)]
mod tests;
impl Session {
    /// Native loop return grants no teardown authority; only actual retained-owner facts do.
    pub(super) fn teardown_ready(&self) -> bool {
        self.state.closing() && self.native_retired && self.settled
    }
    pub(super) fn new(
        options: ReportSurfaceOptions,
    ) -> (Self, sugarsubstitute_contracts::execution::MemoryClaim) {
        let storage = options.dispatch_storage;
        (
            Self {
                state: State::new(options.view.binding(), options.view.restart()),
                controls: None,
                window: None,
                window_id: None,
                hwnd: HWND::default(),
                destroyed: false,
                native_retired: false,
                settled: false,
                failure: None,
                close_deadline: None,
                picker: None,
                prepared_save: None,
                retirement_attempted: false,
                view: options.view,
                labels: options.labels,
            },
            storage,
        )
    }
    pub(super) fn initialize(
        &mut self,
        window: Rc<Window>,
        proxy: EventLoopProxy<Event>,
        lease: MemoryLease,
        content: &mut impl ReportSessionContent,
    ) -> Result<(), ReportSurfaceError> {
        use tao::platform::windows::WindowExtWindows;
        self.hwnd = HWND(window.hwnd() as *mut std::ffi::c_void);
        self.window_id = Some(window.id());
        self.window = Some(window.clone());
        self.controls = Some(Controls::new(window, &self.view, proxy, lease)?);
        let controls = self
            .controls
            .as_mut()
            .ok_or_else(|| ReportSurfaceError::rejected("report_control_owner"))?;
        controls.initialize(&self.view, self.labels)?;
        controls.window.set_visible(true);
        // SAFETY: Inspect the exact retained parent after actual text/controls submission.
        if !unsafe { IsWindowVisible(self.hwnd) }.as_bool() {
            return Err(ReportSurfaceError::rejected("report_window_visibility"));
        }
        self.state.presented()?;
        content.observe(ReportSurfaceObservation::Presented(self.state.binding()))
    }
    pub(super) fn retain(&mut self, cause: ReportSurfaceError) {
        if self.failure.is_none() {
            self.failure = Some(cause);
        } else {
            tracing::error!(operation = cause.operation(), error = %cause, "additional report surface failure");
        }
    }
    pub(super) fn close_at(&mut self, content: &mut impl ReportSessionContent, now: Instant) {
        if self.close_deadline.is_some() {
            return;
        }
        self.state.shutdown();
        self.prepared_save = None;
        self.close_deadline = Some(match now.checked_add(Duration::from_secs(15)) {
            Some(deadline) => deadline,
            None => {
                self.retain(ReportSurfaceError::rejected("report_shutdown_deadline_overflow"));
                now
            }
        });
        if let Err(cause) = content.request_shutdown() {
            self.retain(cause);
        }
        if let Some(controls) = &self.controls
            && let Err(cause) = controls.enable(false, self.state.restart())
        {
            self.retain(cause);
        }
    }
    pub(super) fn fail(
        &mut self,
        content: &mut impl ReportSessionContent,
        cause: ReportSurfaceError,
    ) {
        self.fail_at(content, cause, Instant::now());
    }
    pub(super) fn fail_at(
        &mut self,
        content: &mut impl ReportSessionContent,
        cause: ReportSurfaceError,
        observed_at: Instant,
    ) {
        self.retain(cause);
        if let Err(cause) = content.observe(ReportSurfaceObservation::Failed {
            binding: self.state.binding(),
            failure: ReportFailure::Presentation,
        }) {
            self.retain(cause);
        }
        self.close_at(content, observed_at);
    }
    pub(super) fn poll(
        &mut self,
        content: &mut impl ReportSessionContent,
        maximum: NonZeroUsize,
    ) -> Result<(), ReportSurfaceError> {
        if self.state.closing() {
            return Ok(());
        }
        let update = content.poll(maximum)?;
        self.apply(content, update)
    }
    fn apply(
        &mut self,
        content: &mut impl ReportSessionContent,
        update: ReportSurfaceUpdate,
    ) -> Result<(), ReportSurfaceError> {
        self.apply_at(content, update, Instant::now())
    }
    pub(super) fn apply_at(
        &mut self,
        content: &mut impl ReportSessionContent,
        update: ReportSurfaceUpdate,
        observed_at: Instant,
    ) -> Result<(), ReportSurfaceError> {
        match update {
            ReportSurfaceUpdate::None => {}
            ReportSurfaceUpdate::ReplaceView { old, current, view } => {
                if view.binding() != current {
                    return Err(ReportSurfaceError::rejected("report_view_payload_binding"));
                }
                self.state.preflight_replace(old, current)?;
                self.controls
                    .as_ref()
                    .ok_or_else(|| ReportSurfaceError::rejected("report_replace_native_owner"))?
                    .replace(&view)?;
                self.state.replaced(current, view.restart());
                self.view = view;
                content.observe(ReportSurfaceObservation::ViewReplaced(current))?;
            }
            ReportSurfaceUpdate::CopyAuthorized { request, binding, counter, outcome } => {
                let pending = self.state.matching_request(request, binding)?;
                if pending.intent != Intent::Action(ReportAction::CopyText) {
                    return Err(ReportSurfaceError::rejected("report_copy_action_kind"));
                }
                let controls = self
                    .controls
                    .as_mut()
                    .ok_or_else(|| ReportSurfaceError::rejected("report_action_native_owner"))?;
                let mut native = copy::NativeCopy::new(controls, &self.view, self.labels);
                copy::execute(&mut native, counter, outcome, &mut self.failure, |outcome| {
                    content.observe(ReportSurfaceObservation::CopyFinished {
                        request,
                        binding,
                        outcome,
                    })
                })?;
                self.state.completed_request();
            }
            ReportSurfaceUpdate::ActionResult { request, binding, outcome } => {
                let pending = self.state.matching_request(request, binding)?;
                let controls = self
                    .controls
                    .as_mut()
                    .ok_or_else(|| ReportSurfaceError::rejected("report_action_native_owner"))?;
                match pending.intent {
                    Intent::Action(ReportAction::CopyText) => {
                        return Err(ReportSurfaceError::rejected("report_copy_counter_missing"));
                    }
                    Intent::Export(_) => {
                        controls.status(if outcome.is_ok() {
                            self.labels.exported
                        } else {
                            self.labels.export_failed
                        })?;
                        self.state.completed_request();
                    }
                    Intent::Action(_) => {
                        if outcome.is_err() {
                            controls.status(self.labels.ack_failed)?;
                            self.state.completed_request();
                        }
                    }
                }
            }
            ReportSurfaceUpdate::RestartUnavailable { request, binding } => {
                self.state.restart_unavailable(request, binding)?;
                self.controls
                    .as_mut()
                    .ok_or_else(|| ReportSurfaceError::rejected("report_restart_native_owner"))?
                    .status(self.labels.restart_unavailable)?;
            }
            ReportSurfaceUpdate::CloseAfterDurableAck { binding, acknowledgement } => {
                self.state.durable_close(binding, acknowledgement)?;
                self.close_at(content, observed_at);
            }
        }
        if let Some(controls) = &self.controls {
            controls.enable(self.state.actionable(), self.state.restart())?;
        }
        Ok(())
    }
    pub(super) fn input(
        &mut self,
        content: &mut impl ReportSessionContent,
        input: Input,
    ) -> Result<(), ReportSurfaceError> {
        let Ok(request) = self.state.request(input.binding, input.intent) else {
            return Ok(());
        };
        let controls = self
            .controls
            .as_ref()
            .ok_or_else(|| ReportSurfaceError::rejected("report_input_native_owner"))?;
        controls.enable(false, self.state.restart())?;
        match input.intent {
            Intent::Action(action) => {
                content.action(request.id, ReportCommand::new(request.binding, action))?
            }
            Intent::Export(selection) => {
                self.prepare_save(request, selection)?;
            }
        }
        Ok(())
    }
    pub(super) fn shutdown(&mut self, content: &mut impl ReportSessionContent) {
        if let Some(picker) = &mut self.picker
            && picker.observe_retirement()
        {
            self.picker = None;
        }
        if self.picker.is_none() && !self.retirement_attempted {
            self.retirement_attempted = true;
            match self.controls.as_mut().map_or(Ok(()), Controls::retire) {
                Ok(()) => {
                    self.controls = None;
                    self.window = None;
                }
                Err(cause) => self.retain(cause),
            }
        }
        if !self.settled {
            match content.poll_shutdown() {
                Ok(settled) => self.settled = settled,
                Err(cause) => self.retain(cause),
            }
        }
    }
    pub(super) fn observe_retirement(&mut self, content: &mut impl ReportSessionContent) {
        // SAFETY: Scalar observation follows native dispatch; no successor parent is admitted.
        let gone = self.hwnd.is_invalid()
            || (self.destroyed && !unsafe { IsWindow(Some(self.hwnd)) }.as_bool());
        if !self.native_retired
            && self.picker.is_none()
            && self.controls.is_none()
            && self.window.is_none()
            && gone
        {
            match self.state.native_retired() {
                Ok(()) => {
                    self.native_retired = true;
                    if let Err(cause) = content.observe(ReportSurfaceObservation::NativeRetired(
                        self.state.binding().session(),
                    )) {
                        self.retain(cause);
                    }
                }
                Err(cause) => self.retain(cause),
            }
        }
    }
}
