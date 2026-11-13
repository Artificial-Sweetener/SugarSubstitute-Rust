// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A form window hosted on an event loop that another surface owns: the running application's
//! loop, with the form owned by and centred on the main window. The host routes the form's
//! events here from its own callbacks; nothing here pumps events or waits.
use super::*;
use crate::{
    completion_wake::{ProxyWake, WakeLifetime},
    file_dialog::Picker as NativePicker,
    native_action::{FormAction, FormScope, NativeAction},
};
use tao::{
    event::WindowEvent,
    event_loop::{EventLoopProxy, EventLoopWindowTarget},
    window::WindowId,
};

/// Where the hosted form is in its life.
enum Stage {
    /// The window is open.
    Open(Box<FormWindow>),
    /// The window was dropped; tao reports its destruction on a later turn.
    Destroying(WindowId),
    /// The window is gone; the lifetime's own work settles.
    Settling,
    /// Everything has settled and the lifetime is finished.
    Finished,
}

/// What a turn leaves for the host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HostedTurn {
    /// The form's window still exists, so its owner must not be destroyed yet: Windows destroys
    /// owned windows together with their owner.
    pub(crate) holds_owner: bool,
    /// The form has settled and finished; the host drops it.
    pub(crate) finished: bool,
}

/// One form window on a shared loop, from opening until its lifetime has settled. Failures are
/// logged and close the form; they never reach the host, whose own surfaces keep running.
pub(crate) struct HostedForm<L: Lifetime> {
    stage: Stage,
    lifetime: L,
    wake: WakeLifetime,
    started: std::time::Instant,
}

impl<L: Lifetime> HostedForm<L> {
    /// Open `lifetime` in a window owned by `owner` on the host's loop. `picker` is the loop's
    /// shell picker, unbound; the form binds it until its window retires.
    /// # Errors
    /// Window creation failed; nothing was opened.
    pub(crate) fn open(
        target: &EventLoopWindowTarget<NativeAction>,
        proxy: EventLoopProxy<NativeAction>,
        picker: NativePicker,
        owner: FormOwner,
        options: Window,
        mut lifetime: L,
    ) -> Result<Self, Box<dyn Error>> {
        let wake = WakeLifetime::new();
        let completion =
            Arc::new(ProxyWake::new(proxy.clone(), wake.clone(), NativeAction::Completions));
        let form = FormWindow::open(
            target,
            proxy,
            FormScope(next_scope()),
            picker,
            Some(owner),
            options,
            crate::windows_system_accent::current(),
            &mut lifetime,
            completion,
        )?;
        Ok(Self {
            stage: Stage::Open(Box::new(form)),
            lifetime,
            wake,
            started: std::time::Instant::now(),
        })
    }

    /// Whether `window` is this form's window, open or awaiting destruction.
    pub(crate) fn owns_window(&self, window: WindowId) -> bool {
        match &self.stage {
            Stage::Open(form) => form.window_id() == window,
            Stage::Destroying(id) => *id == window,
            Stage::Settling | Stage::Finished => false,
        }
    }

    /// Bring the open form forward.
    pub(crate) fn focus(&self) {
        if let Stage::Open(form) = &self.stage {
            form.focus();
        }
    }

    /// Deliver a window event addressed to this form's window.
    pub(crate) fn window_event(&mut self, event: &WindowEvent<'_>) {
        match (&mut self.stage, event) {
            (Stage::Open(form), event) => form.input(FormInput::Window(event), &mut self.lifetime),
            (Stage::Destroying(_), WindowEvent::Destroyed) => {
                self.stage = Stage::Settling;
                if let Err(cause) = self.lifetime.window_retired() {
                    failed(&*cause);
                }
            }
            _ => {}
        }
    }

    /// Deliver an action from this form's peers; another scope's actions are ignored.
    pub(crate) fn action(&mut self, scope: FormScope, action: FormAction) {
        if let Stage::Open(form) = &mut self.stage
            && form.scope() == scope
        {
            form.input(FormInput::Action(action), &mut self.lifetime);
        }
    }

    /// Repaint this form's window.
    pub(crate) fn redraw(&mut self) {
        if let Stage::Open(form) = &mut self.stage {
            form.input(FormInput::Redraw, &mut self.lifetime);
        }
    }

    /// Deliver a changed system accent.
    pub(crate) fn accent(
        &mut self,
        accent: Option<sugarsubstitute_contracts::appearance::AccentColor>,
    ) {
        if let Stage::Open(form) = &mut self.stage {
            form.input(FormInput::Accent(accent), &mut self.lifetime);
        }
    }

    /// Advance one turn. `close` closes the form at once, as when its owner is closing.
    /// Returns the picker once the window has released it.
    pub(crate) fn turn(&mut self, close: bool) -> (HostedTurn, Option<NativePicker>) {
        let mut picker = None;
        match &mut self.stage {
            Stage::Open(form) => {
                if close {
                    form.close_now();
                }
                if form.turn(&mut self.lifetime) == FormTurn::Released
                    && let Stage::Open(form) = std::mem::replace(&mut self.stage, Stage::Settling)
                {
                    let (id, released, failure) = (*form).retire();
                    if let Some(cause) = failure {
                        failed(&*cause);
                    }
                    picker = Some(released);
                    self.stage = Stage::Destroying(id);
                }
            }
            Stage::Destroying(_) | Stage::Finished => {}
            Stage::Settling => {
                let settled = self
                    .lifetime
                    .advance(self.started.elapsed())
                    .and_then(|()| self.lifetime.settled());
                if !matches!(settled, Ok(false)) {
                    if let Err(cause) = settled {
                        failed(&*cause);
                    }
                    self.wake.disconnect();
                    if let Err(cause) = self.lifetime.finish() {
                        failed(&*cause);
                    }
                    self.stage = Stage::Finished;
                }
            }
        }
        let turn = HostedTurn {
            holds_owner: matches!(self.stage, Stage::Open(_) | Stage::Destroying(_)),
            finished: matches!(self.stage, Stage::Finished),
        };
        (turn, picker)
    }
}

/// A scope no earlier form on this process used, so a late action of a closed form is inert.
fn next_scope() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// A hosted form is optional: its failures are logged and close it, never its host.
fn failed(cause: &dyn Error) {
    tracing::error!(operation = "hosted_form_window", cause = %cause,
        "A window opened over the main window failed and closed");
}
