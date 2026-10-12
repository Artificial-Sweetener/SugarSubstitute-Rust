// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener

//! Initialization after native creation callbacks have returned to the outer loop.

use super::*;
use construction::Driver;

pub(super) fn initialize(driver: &mut Driver) -> Result<()> {
  let (hwnd, controller, env) = {
    let state = driver.shared.native.borrow();
    (
      state
        .hwnd
        .ok_or_else(|| owned_config::invalid("missing initialization container"))?,
      state
        .controller
        .clone()
        .ok_or_else(|| owned_config::invalid("missing initialization controller"))?,
      state
        .env
        .clone()
        .ok_or_else(|| owned_config::invalid("missing initialization environment"))?,
    )
  };
  let mut raw = std::ptr::null_mut();
  let status = unsafe { (controller.vtable().CoreWebView2)(controller.as_raw(), &mut raw) };
  let actual = if raw.is_null() {
    None
  } else {
    Some(unsafe { ICoreWebView2::from_raw(raw) })
  };
  driver.shared.native.borrow_mut().webview = actual.clone();
  status.ok()?;
  let webview = actual.ok_or_else(|| windows::core::Error::from(E_POINTER))?;
  if driver.shared.native.borrow().ledger.retiring {
    return Ok(());
  }
  let mut attrs = driver
    .config
    .attrs
    .take()
    .ok_or_else(|| owned_config::invalid("initialization inputs already consumed"))?;
  if driver.config.owned {
    unsafe {
      InnerWebView::set_webview_settings(
        &webview,
        &attrs,
        &driver.config.platform,
        &driver.shared,
      )?;
    }
    if driver.shared.native.borrow().ledger.retiring {
      return Ok(());
    }
    InnerWebView::set_appearance(
      &webview,
      &controller,
      &attrs,
      driver.config.platform.theme,
      &driver.shared,
    )?;
    if driver.shared.native.borrow().ledger.retiring {
      return Ok(());
    }
    attach_owned_handlers(&driver.shared, &webview)?;
    if driver.shared.native.borrow().ledger.retiring {
      return Ok(());
    }
    driver
      .shared
      .initialization_effect(|| unsafe { Ok(controller.SetIsVisible(false)?) })?;
    if driver.shared.native.borrow().ledger.retiring {
      return Ok(());
    }
    driver
      .shared
      .initialization_effect(|| unsafe { Ok(webview.Navigate(w!("about:blank"))?) })?;
  } else {
    let handler = attrs.drag_drop_handler.take();
    let id = driver
      .config
      .id
      .clone()
      .unwrap_or_else(|| (hwnd.0 as isize).to_string());
    InnerWebView::init_webview(
      driver.shared.parent.get(),
      hwnd,
      id,
      attrs,
      &env,
      &controller,
      driver.config.platform.clone(),
      driver.config.is_child,
      &driver.shared,
      webview,
    )?;
    if let Some(handler) = handler {
      let disabled = unsafe {
        controller
          .cast::<ICoreWebView2Controller4>()?
          .SetAllowExternalDrop(false)
      };
      disabled?;
      let owner = DragDropController::new(hwnd, handler);
      driver.shared.native.borrow_mut().drag_drop = Some(owner);
    }
  }
  Ok(())
}

fn attach_owned_handlers(shared: &Rc<owned_state::Shared>, webview: &ICoreWebView2) -> Result<()> {
  let owner = Rc::downgrade(shared);
  let mut token = 0;
  let installed = unsafe {
    webview.add_WindowCloseRequested(
      &WindowCloseRequestedEventHandler::create(Box::new(move |_, _| {
        if let Some(shared) = owner.upgrade() {
          shared.request_native_close();
        }
        Ok(())
      })),
      &mut token,
    )
  };
  if installed.is_ok() || token != 0 {
    let mut state = shared.native.borrow_mut();
    state.close_token = Some(token);
    state.ledger.close_event = true;
  }
  installed?;
  if shared.native.borrow().ledger.retiring {
    return Ok(());
  }
  let owner = Rc::downgrade(shared);
  token = 0;
  let installed = unsafe {
    webview.add_NewWindowRequested(
      &NewWindowRequestedEventHandler::create(Box::new(move |_, args| {
        if let Some(shared) = owner.upgrade() {
          let _callback = shared.callback_guard();
          let handled = match args {
            Some(args) => args.SetHandled(true),
            None => Err(windows::core::Error::from(E_POINTER)),
          };
          if let Err(cause) = handled {
            shared.fail(cause.into());
            (shared.wake)();
          }
        }
        Ok(())
      })),
      &mut token,
    )
  };
  if installed.is_ok() || token != 0 {
    let mut state = shared.native.borrow_mut();
    state.popup_token = Some(token);
    state.ledger.popup_event = true;
  }
  installed?;
  Ok(())
}
