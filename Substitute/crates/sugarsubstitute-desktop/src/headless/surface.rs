// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The one surface a headless role may show beside its pump: a hosted report form on the
//! pump's own loop, with no owner window. The role keeps pumping while it is open; the form's
//! controller decides when it ends, and the pump closes it before the role finishes.
use crate::{
    HeadlessLifetime, HeadlessPresentation,
    file_dialog::Picker,
    installer::native::HostedForm,
    native_action::NativeAction,
    report::surface::{ConflictReport, ReportPresentation, lifetime::ReportLifetime},
};
use tao::{
    event::Event,
    event_loop::{EventLoopProxy, EventLoopWindowTarget},
};

/// The hosted form, once the role has asked for one.
pub(super) struct Surface {
    picker: Option<Picker>,
    form: Option<HostedForm<ReportLifetime<'static>>>,
    /// The role has shown its surface; a role shows at most one per pump.
    shown: bool,
}

impl Surface {
    /// Hold the loop's shell picker until a form binds it.
    pub(super) fn new(picker: Picker) -> Self {
        Self { picker: Some(picker), form: None, shown: false }
    }

    /// Whether a form is open or still settling, so the loop must keep running.
    pub(super) fn open(&self) -> bool {
        self.form.is_some()
    }

    /// Route one event addressed to the form. Returns whether it was the form's.
    pub(super) fn route(&mut self, event: &Event<'_, NativeAction>) -> bool {
        let Some(form) = &mut self.form else { return false };
        match event {
            Event::WindowEvent { window_id, event, .. } if form.owns_window(*window_id) => {
                form.window_event(event);
            }
            Event::UserEvent(NativeAction::Form(scope, action)) => form.action(*scope, *action),
            Event::UserEvent(NativeAction::SystemAccent) => {
                form.accent(crate::windows_system_accent::current());
            }
            Event::RedrawRequested(id) if form.owns_window(*id) => form.redraw(),
            _ => return false,
        }
        true
    }

    /// Open the role's surface once it asks for one, then advance the open form. `closing`
    /// closes it at once, as when the role is shutting down.
    pub(super) fn turn(
        &mut self,
        lifetime: &mut impl HeadlessLifetime,
        target: &EventLoopWindowTarget<NativeAction>,
        proxy: &EventLoopProxy<NativeAction>,
        closing: bool,
    ) {
        if !closing
            && !self.shown
            && let Some(presentation) = lifetime.take_presentation()
        {
            self.shown = true;
            self.show(presentation, target, proxy);
        }
        if let Some(form) = &mut self.form {
            let (turn, picker) = form.turn(closing);
            if picker.is_some() {
                self.picker = picker;
            }
            if turn.finished {
                self.form = None;
            }
        }
    }

    fn show(
        &mut self,
        presentation: HeadlessPresentation,
        target: &EventLoopWindowTarget<NativeAction>,
        proxy: &EventLoopProxy<NativeAction>,
    ) {
        let Some(picker) = self.picker.take() else {
            tracing::error!(operation = "open_headless_surface", "The shell picker is held");
            return;
        };
        let HeadlessPresentation { copy, title, controller } = presentation;
        let window = crate::report::surface::report_window(title.clone());
        let opened = ReportLifetime::new(
            ReportPresentation::waiting_for_previous_copy(copy, &title),
            ConflictReport::forwarding(controller, copy.conflict),
        )
        .and_then(|lifetime| {
            HostedForm::open(target, proxy.clone(), picker.clone(), None, window, lifetime)
        });
        match opened {
            Ok(form) => self.form = Some(form),
            Err(cause) => {
                self.picker = Some(picker);
                tracing::error!(operation = "open_headless_surface", cause = %cause,
                    "The waiting surface could not open");
            }
        }
    }
}
