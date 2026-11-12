// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! One prepared-pixel form window and its shared form peers, driven by whichever event loop
//! owns it: its own loop for the installer and standalone reports, or the running application's
//! loop for a report opened over the main window. The window never pumps events itself.
use super::super::*;
use super::projection::Projection;
use crate::{
    file_dialog::Picker as NativePicker,
    native_action::{FormAction, FormScope, NativeAction},
    windows::Presenter,
    windows_controls::Controls,
    windows_scroll::ScrollViewport,
};
use std::{rc::Rc, time::Instant};
use sugarsubstitute_contracts::{execution::MemoryClaim, presentation::FrameIdentity};
use tao::{
    dpi::LogicalSize,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{EventLoopProxy, EventLoopWindowTarget},
    platform::windows::{WindowBuilderExtWindows, WindowExtWindows},
    window::{Theme, WindowBuilder, WindowId},
};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

#[path = "form_window_input.rs"]
mod input;
#[path = "form_window_present.rs"]
mod present;

/// Translate the same theme resolved by the native window into shared preparation input.
pub(super) fn paint_theme(theme: Theme) -> sugarsubstitute_contracts::configuration::Theme {
    if theme == Theme::Dark {
        sugarsubstitute_contracts::configuration::Theme::Dark
    } else {
        sugarsubstitute_contracts::configuration::Theme::Light
    }
}

/// What the owning loop delivers to the form window.
pub(crate) enum FormInput<'a> {
    /// A window event addressed to this form's window.
    Window(&'a WindowEvent<'a>),
    /// An action from this form's peers.
    Action(FormAction),
    /// The operating-system accent changed.
    Accent(Option<sugarsubstitute_contracts::appearance::AccentColor>),
    /// The system asked this form's window to repaint.
    Redraw,
}

/// What a turn left for the owning loop to do.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FormTurn {
    /// The window stays.
    Open,
    /// The window's peers and buffers are released; the owner drops the window next.
    Released,
}

/// The window a form opens over: it owns the form window and the form centres on it.
#[derive(Clone, Copy)]
pub(crate) struct FormOwner(pub(crate) ::windows::Win32::Foundation::HWND);

/// One open form window. Errors inside a turn close the window and are kept for the owner.
pub(crate) struct FormWindow {
    window: Rc<tao::window::Window>,
    proxy: EventLoopProxy<NativeAction>,
    scope: FormScope,
    owner: Option<FormOwner>,
    picker: NativePicker,
    presenter: Option<Presenter>,
    scroll: ScrollViewport,
    controls: Option<Controls>,
    storage: Option<MemoryClaim>,
    projection: Option<Projection>,
    identity: Option<FrameIdentity>,
    shape: Option<FrameShape>,
    hovered: Option<ControlId>,
    pressed: Option<ControlId>,
    point: (f64, f64),
    picker_request: Option<Picker>,
    applied_size: Option<[u32; 2]>,
    closing: bool,
    close_started: bool,
    revealed: bool,
    released: bool,
    started: Instant,
    options_visible: bool,
    close: ControlId,
    failure: Option<Box<dyn Error>>,
}

impl FormWindow {
    /// Create the hidden window on `target` and start `lifetime` with `wake`. `picker` must be
    /// installed on the same loop and unbound; it binds to this window until
    /// [`FormWindow::retire`] releases it.
    /// A construction failure is kept and closes the window on its first turn.
    /// # Errors
    /// Only window creation itself fails here.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn open(
        target: &EventLoopWindowTarget<NativeAction>,
        proxy: EventLoopProxy<NativeAction>,
        scope: FormScope,
        picker: NativePicker,
        owner: Option<FormOwner>,
        options: Window,
        accent: Option<sugarsubstitute_contracts::appearance::AccentColor>,
        lifetime: &mut impl Lifetime,
        wake: Arc<dyn CompletionWake>,
    ) -> Result<Self> {
        let mut builder = WindowBuilder::new()
            .with_title(options.title)
            .with_inner_size(LogicalSize::new(options.size[0], options.size[1]))
            .with_resizable(options.resizable)
            .with_max_inner_size(tao::dpi::PhysicalSize::new(4096, 4096))
            .with_decorations(false)
            .with_transparent(true)
            .with_theme(None)
            .with_visible(false);
        if let Some([width, height]) = options.min_size {
            builder = builder.with_min_inner_size(LogicalSize::new(width, height));
        }
        if let Some(FormOwner(hwnd)) = owner {
            builder = builder.with_owner_window(hwnd.0 as isize).with_skip_taskbar(true);
        }
        let window = Rc::new(builder.build(target)?);
        let mut failure: Option<Box<dyn Error>> = None;
        let mut retain = |cause: Box<dyn Error>| {
            if failure.is_none() {
                failure = Some(cause);
            }
        };
        let presenter = match Presenter::new_main(window.clone(), window.theme() == Theme::Dark) {
            Ok(owner) => Some(owner),
            Err(cause) => {
                retain(Box::new(cause));
                None
            }
        };
        let picker_proxy = proxy.clone();
        let hwnd = ::windows::Win32::Foundation::HWND(window.hwnd() as *mut _);
        if let Err(cause) = picker.bind(hwnd, move || {
            picker_proxy
                .send_event(NativeAction::Completions)
                .map_err(|cause| crate::DesktopError::new("form_picker_wake", cause))
        }) {
            retain(Box::new(cause));
        }
        match presenter.as_ref().map(Presenter::work_area_height).transpose() {
            Ok(Some(height)) => lifetime.work_area(height),
            Ok(None) => {}
            Err(cause) => retain(Box::new(cause)),
        }
        let scroll = ScrollViewport::new(window.clone());
        if let Err(cause) = scroll.initialize() {
            retain(Box::new(cause));
        }
        // Resolve before start so the first prepared page and native peers share this mode.
        if let Err(cause) = lifetime.input(Event::SystemTheme(paint_theme(window.theme()))) {
            retain(cause);
        }
        if let Err(cause) = lifetime.input(Event::SystemAccent(accent)) {
            retain(cause);
        }
        if let Err(cause) = lifetime.start(wake) {
            tracing::error!(operation="native_form_start", cause=%cause, "Form window start failed");
            retain(cause);
        }
        Ok(Self {
            closing: failure.is_some(),
            window,
            proxy,
            scope,
            owner,
            picker,
            presenter,
            scroll,
            controls: None,
            storage: None,
            projection: None,
            identity: None,
            shape: None,
            hovered: None,
            pressed: None,
            point: (0.0, 0.0),
            picker_request: None,
            applied_size: None,
            close_started: false,
            revealed: false,
            released: false,
            started: Instant::now(),
            options_visible: options.visible,
            close: options.close,
            failure,
        })
    }

    /// This form's window, for routing window events.
    pub(crate) fn window_id(&self) -> WindowId {
        self.window.id()
    }

    /// This form's action scope, for routing peer actions.
    pub(crate) fn scope(&self) -> FormScope {
        self.scope
    }

    /// The native window, for clipboard ownership.
    pub(super) fn hwnd(&self) -> ::windows::Win32::Foundation::HWND {
        ::windows::Win32::Foundation::HWND(self.window.hwnd() as *mut _)
    }

    /// Bring the open form forward, as when the user asks for it again.
    pub(crate) fn focus(&self) {
        if self.revealed && !self.closing {
            self.window.set_focus();
        }
    }

    /// Close at once without asking the lifetime, as when the owning application closes.
    pub(crate) fn close_now(&mut self) {
        self.closing = true;
    }

    /// Deliver one input.
    pub(crate) fn input(&mut self, input: FormInput<'_>, lifetime: &mut impl Lifetime) {
        if self.closing {
            return;
        }
        self.guard(|form| form.dispatch(input, lifetime));
    }

    /// Run one turn after the loop's events were dispatched.
    pub(crate) fn turn(&mut self, lifetime: &mut impl Lifetime) -> FormTurn {
        if self.released {
            return FormTurn::Released;
        }
        let mut released = false;
        self.guard(|form| {
            released = form.advance(lifetime)?;
            Ok(())
        });
        if released {
            self.released = true;
            FormTurn::Released
        } else {
            FormTurn::Open
        }
    }

    /// Release the picker and drop the window; tao destroys it on the next loop turn. Returns
    /// the window id whose destruction the owner waits for, the picker and the first failure.
    pub(crate) fn retire(self) -> (WindowId, NativePicker, Option<Box<dyn Error>>) {
        let Self { window, picker, mut failure, scroll, controls, presenter, .. } = self;
        if let Err(cause) = picker.release()
            && failure.is_none()
        {
            failure = Some(Box::new(cause));
        }
        drop(scroll);
        drop(controls);
        drop(presenter);
        let id = window.id();
        drop(window);
        (id, picker, failure)
    }

    fn guard(&mut self, body: impl FnOnce(&mut Self) -> Result) {
        if let Err(cause) = body(self) {
            tracing::error!(operation="native_form_window", cause=%cause, "Form window entered cleanup");
            if self.failure.is_none() {
                self.failure = Some(cause);
            }
            self.closing = true;
        }
    }

    /// One turn: close handling, sizing, picker, clipboard and presentation of a new page.
    /// Returns true once the closing window's peers and buffers are released.
    fn advance(&mut self, lifetime: &mut impl Lifetime) -> Result<bool> {
        if let Some(owner) = &self.controls {
            owner.check()?;
        }
        self.picker.check()?;
        self.closing |= self.failure.is_some() || lifetime.close_requested();
        if self.closing {
            self.picker.cancel()?;
        }
        if self.closing && !self.close_started {
            self.close_started = true;
            lifetime.close()?;
            self.scroll.shutdown()?;
        }
        if !self.closing {
            let size = self.window.inner_size();
            if size.width > 0 && size.height > 0 {
                let next = FrameShape::new(size.width, size.height, self.window.scale_factor())?;
                if self.shape != Some(next) {
                    lifetime.resize(next)?;
                    self.shape = Some(next);
                }
            }
        }
        if let Some(result) = self.picker.take_result() {
            let request =
                self.picker_request.take().ok_or("Native picker returned without a request")?;
            if self.closing {
                result?;
            } else {
                let result = result
                    .and_then(|path| {
                        path.map(|path| {
                            path.into_os_string().into_string().map_err(|_| {
                                Box::<dyn Error>::from(
                                    "Selected path cannot be represented in interface text",
                                )
                            })
                        })
                        .transpose()
                    })
                    .map_err(|cause| cause.to_string());
                lifetime.input(Event::Picked(request, result))?;
            }
        }
        if !self.closing
            && !self.picker.busy()
            && let Some(request) = lifetime.picker()
        {
            self.picker.request(request.kind)?;
            self.picker_request = Some(request);
        }
        if !self.closing
            && let Some(position) = self.scroll.take_position()?
        {
            lifetime.input(Event::Scroll(position))?;
        }
        lifetime.advance(self.started.elapsed())?;
        if !self.closing
            && let Some(transfer) = lifetime.clipboard()
        {
            let copied = super::copy_transfer(self.hwnd(), &transfer);
            lifetime.copied(copied)?;
        }
        if !self.closing
            && let Some(size) = lifetime.preferred_size()
        {
            if Some(size) != self.applied_size {
                super::apply_client_size(&self.window, size);
                self.applied_size = Some(size);
                if !self.revealed {
                    self.place()?;
                }
            }
            // Acknowledge every turn with the size the window really has, so a lifetime
            // waiting on its request prepares for this window, not the request.
            let current = self.window.inner_size();
            if current.width > 0 && current.height > 0 {
                let next =
                    FrameShape::new(current.width, current.height, self.window.scale_factor())?;
                self.shape = Some(next);
                lifetime.resize(next)?;
            }
        }
        if self.closing {
            return self.release(lifetime);
        }
        self.present(lifetime)?;
        Ok(false)
    }
}
