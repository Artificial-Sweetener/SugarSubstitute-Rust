// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Apply fenced decisions to retained native resources on one UI thread.
use super::super::{
    NativeReportView, ReportSessionContent, ReportSurfaceError, ReportSurfaceFailure,
    ReportSurfaceObservation, ReportSurfaceOptions, ReportSurfaceUpdate,
    state::{Intent, State},
};
use super::{
    buffer::Text,
    callback::{Event, Input},
    controls::Controls,
    save_dialog,
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
    retirement_attempted: bool,
    view: NativeReportView,
    labels: &'static ReportMessages,
}
#[cfg(test)]
mod tests;
impl Session {
    /// Native loop return grants no teardown authority; only actual retained-owner facts do.
    pub(super) fn teardown_ready(&self) -> bool {
        self.state.closing() && self.native_retired && self.settled
    }
    pub(super) fn new(options: ReportSurfaceOptions) -> Self {
        Self {
            state: State::new(options.view.binding()),
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
            retirement_attempted: false,
            view: options.view,
            labels: options.labels,
        }
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
    pub(super) fn close(&mut self, content: &mut impl ReportSessionContent) {
        if self.close_deadline.is_some() {
            return;
        }
        self.state.shutdown();
        let now = Instant::now();
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
            && let Err(cause) = controls.enable(false)
        {
            self.retain(cause);
        }
    }
    pub(super) fn fail(
        &mut self,
        content: &mut impl ReportSessionContent,
        cause: ReportSurfaceError,
    ) {
        self.retain(cause);
        if let Err(cause) = content.observe(ReportSurfaceObservation::Failed {
            binding: self.state.binding(),
            failure: ReportFailure::Presentation,
        }) {
            self.retain(cause);
        }
        self.close(content);
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
                self.view = view;
                self.state.replaced(current);
                content.observe(ReportSurfaceObservation::ViewReplaced(current))?;
            }
            ReportSurfaceUpdate::ActionResult { request, binding, outcome } => {
                let pending = self.state.matching_request(request, binding)?;
                let controls = self
                    .controls
                    .as_mut()
                    .ok_or_else(|| ReportSurfaceError::rejected("report_action_native_owner"))?;
                match pending.intent {
                    Intent::Action(ReportAction::CopyText) => {
                        let result = if outcome.is_ok() {
                            Text::new(self.view.text(), &controls.lease).and_then(|text| {
                                controls.clipboard.copy(&text.units, &controls.lease)
                            })
                        } else {
                            Err(ReportSurfaceError::rejected("report_copy_host_rejected"))
                        };
                        let outcome = result
                            .as_ref()
                            .map(|_| ())
                            .map_err(|_| ReportSurfaceFailure::Clipboard);
                        controls.status(if outcome.is_ok() {
                            self.labels.copied
                        } else {
                            self.labels.copy_failed
                        })?;
                        content.observe(ReportSurfaceObservation::CopyFinished {
                            request,
                            binding,
                            outcome,
                        })?;
                        if let Err(cause) = result {
                            self.retain(cause);
                        }
                        self.state.completed_request();
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
            ReportSurfaceUpdate::CloseAfterDurableAck { binding, acknowledgement } => {
                self.state.durable_close(binding, acknowledgement)?;
                self.close(content);
            }
        }
        if let Some(controls) = &self.controls {
            controls.enable(self.state.actionable())?;
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
        controls.enable(false)?;
        match input.intent {
            Intent::Action(action) => {
                content.action(request.id, ReportCommand::new(request.binding, action))?
            }
            Intent::Export(selection) => {
                let outcome = save_dialog::select(
                    self.hwnd,
                    request,
                    selection,
                    self.labels,
                    &controls.lease,
                    content,
                )?;
                self.picker = outcome.retirement;
                if let Some(cause) = outcome.failure {
                    return Err(cause);
                }
                if let Some(selection) = outcome.selection {
                    content.observe(ReportSurfaceObservation::SaveSelected(selection))?;
                } else {
                    self.state.completed_request();
                    content.observe(ReportSurfaceObservation::SaveCancelled {
                        request: request.id,
                        binding: request.binding,
                    })?;
                }
                if let Some(update) = outcome.update {
                    self.apply(content, update)?;
                } else if let Some(controls) = &self.controls {
                    controls.enable(self.state.actionable())?;
                }
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
