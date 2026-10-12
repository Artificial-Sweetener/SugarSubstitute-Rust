// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener

//! Checked UI-affine retirement. Intent never clears native resource slots.

use super::*;
use construction::Driver;
use owned_state::{RetireAction, Shared};

const CONTAINER_OBSERVER: usize = 0x5353_5752;

/// Independent owner of a partial construction's outstanding callbacks and native resources.
///
/// The parent HWND must remain owned while this object reports `false` or an error.
/// Retry from normal native event-loop drains under the original host deadline.
/// Never infer clean shutdown from dropping the owner or requesting cancellation.
#[must_use]
pub struct WebViewRetirement {
  driver: Option<Box<Driver>>,
  shared: Rc<Shared>,
}
impl WebViewRetirement {
  pub(super) fn empty(shared: Rc<Shared>) -> Self {
    Self {
      driver: None,
      shared,
    }
  }
  pub(super) fn new(driver: Box<Driver>) -> Self {
    driver.shared.native.borrow_mut().ledger.retiring = true;
    Self {
      shared: driver.shared.clone(),
      driver: Some(driver),
    }
  }
  /// Perform one checked native teardown step. Errors retain the failed resource slot.
  /// `true` requires completed callbacks, checked controller Close and actual window destruction.
  pub fn checked_retire(&mut self) -> Result<bool> {
    step(&self.shared)
  }
}
impl Drop for WebViewRetirement {
  fn drop(&mut self) {
    if !self.shared.native.borrow().ledger.retired {
      if let Some(driver) = self.driver.take() {
        drop_driver(driver);
      } else {
        fallback(self.shared.clone());
      }
    }
  }
}

pub(super) fn drop_driver(driver: Box<Driver>) {
  if driver.shared.native.borrow().ledger.retired {
    return;
  }
  retain_before_diagnostic(driver, diagnose);
}
pub(super) fn fallback(shared: Rc<Shared>) {
  if shared.native.borrow().ledger.retired {
    return;
  }
  retain_before_diagnostic(shared, diagnose);
}
pub(super) fn retain_before_diagnostic<T>(owner: T, diagnostic: impl FnOnce()) {
  // A subscriber can panic. Make ownership retention irreversible before invoking it.
  std::mem::forget(owner);
  diagnostic();
}
fn diagnose() {
  #[cfg(feature = "tracing")]
  tracing::error!(
    operation = "webview_unresolved_drop",
    "retaining UI-affine transaction until process termination"
  );
}

pub(super) fn step(shared: &Rc<Shared>) -> Result<bool> {
  step_with(shared, |action| unsafe {
    match action {
      RetireAction::RemoveClose | RetireAction::RemovePopup => {
        let (webview, token) = {
          let state = shared.native.borrow();
          let token = if action == RetireAction::RemoveClose {
            state.close_token
          } else {
            state.popup_token
          };
          (
            state
              .webview
              .clone()
              .ok_or_else(|| owned_config::invalid("missing event webview"))?,
            token.ok_or_else(|| owned_config::invalid("missing retained event token"))?,
          )
        };
        if action == RetireAction::RemoveClose {
          webview.remove_WindowCloseRequested(token)?;
        } else {
          webview.remove_NewWindowRequested(token)?;
        }
      }
      RetireAction::Close => {
        let controller = shared
          .native
          .borrow()
          .controller
          .clone()
          .ok_or_else(|| owned_config::invalid("missing retained controller"))?;
        controller.Close()?;
      }
      RetireAction::RemoveParent => super::retirement_subclass::remove_parent(shared)?,
      RetireAction::RemoveDispatcher => {
        let hwnd = shared
          .native
          .borrow()
          .hwnd
          .ok_or_else(|| owned_config::invalid("missing retained dispatcher window"))?;
        if !RemoveWindowSubclass(
          hwnd,
          Some(InnerWebView::main_thread_dispatcher_proc),
          MAIN_THREAD_DISPATCHER_SUBCLASS_ID as _,
        )
        .as_bool()
        {
          return Err(bool_failure());
        }
      }
      RetireAction::Destroy => {
        let hwnd = shared
          .native
          .borrow()
          .hwnd
          .ok_or_else(|| owned_config::invalid("missing retained window"))?;
        DestroyWindow(hwnd)?;
        // Successful destruction permits reclaim if no subclass callback was delivered.
        let installed = shared.native.borrow_mut().container_ref.take();
        if let Some(raw) = installed {
          drop(Rc::from_raw(raw as *const Shared));
        }
      }
      RetireAction::Release | RetireAction::Busy | RetireAction::Done => {}
    }
    Ok(())
  })
}

pub(super) fn step_with(
  shared: &Rc<Shared>,
  mut effect: impl FnMut(RetireAction) -> Result<()>,
) -> Result<bool> {
  {
    let mut state = shared.native.borrow_mut();
    state.ledger.retiring = true;
    if state.ledger.retired {
      return Ok(true);
    }
  }
  let busy = shared.busy.get() || shared.callback_depth.get() != 0;
  let action = shared.native.borrow().ledger.action(busy);
  if action == RetireAction::Busy {
    return Ok(false);
  }
  let _call = shared.begin_call()?;
  effect(action)?;
  let release = {
    let mut state = shared.native.borrow_mut();
    state.ledger.commit(action);
    if action == RetireAction::RemoveClose {
      state.close_token = None;
    }
    if action == RetireAction::RemovePopup {
      state.popup_token = None;
    }
    if action == RetireAction::Destroy {
      state.hwnd = None;
    }
    if action == RetireAction::Release {
      Some((
        state.webview.take(),
        state.controller.take(),
        state.env.take(),
        state.drag_drop.take(),
      ))
    } else {
      None
    }
  };
  drop(release);
  Ok(shared.native.borrow().ledger.retired)
}

pub(super) fn bool_failure() -> Error {
  let code = unsafe { GetLastError() };
  error_for_bool(code.0)
}

pub(super) fn error_for_bool(code: u32) -> Error {
  let code = if code == 0 { ERROR_GEN_FAILURE.0 } else { code };
  windows::core::Error::from(windows::core::HRESULT::from_win32(code)).into()
}

pub(super) fn observe_container(shared: &Rc<Shared>, hwnd: HWND) -> Result<()> {
  retirement_reference::install(shared, retirement_reference::Reference::Container, |raw| {
    let installed =
      unsafe { SetWindowSubclass(hwnd, Some(container_proc), CONTAINER_OBSERVER, raw) };
    if !installed.as_bool() {
      return Err(bool_failure());
    }
    Ok(())
  })
}

unsafe extern "system" fn container_proc(
  hwnd: HWND,
  message: u32,
  wparam: WPARAM,
  lparam: LPARAM,
  _: usize,
  raw: usize,
) -> LRESULT {
  let borrowed = std::mem::ManuallyDrop::new(Rc::from_raw(raw as *const Shared));
  let shared = Rc::clone(&borrowed);
  shared.callback_depth.set(shared.callback_depth.get() + 1);
  let result = DefSubclassProc(hwnd, message, wparam, lparam);
  if message == WM_NCDESTROY {
    retirement_reference::destroyed(&shared, retirement_reference::Reference::Container);
  }
  shared.callback_depth.set(shared.callback_depth.get() - 1);
  result
}
