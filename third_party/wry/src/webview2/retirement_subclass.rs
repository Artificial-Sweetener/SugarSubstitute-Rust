// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener

//! Parent subclass references stay installed until checked removal or NCDESTROY.

use super::*;
use owned_state::Shared;

pub(super) fn install_parent(shared: &Rc<Shared>, parent: HWND) -> Result<()> {
  retirement_reference::install(shared, retirement_reference::Reference::Parent, |raw| {
    if !unsafe { SetWindowSubclass(parent, Some(parent_proc), PARENT_SUBCLASS_ID as _, raw) }
      .as_bool()
    {
      return Err(retirement::bool_failure());
    }
    Ok(())
  })
}

pub(super) fn remove_parent(shared: &Rc<Shared>) -> Result<()> {
  if !unsafe {
    RemoveWindowSubclass(
      shared.parent.get(),
      Some(parent_proc),
      PARENT_SUBCLASS_ID as _,
    )
  }
  .as_bool()
  {
    return Err(retirement::bool_failure());
  }
  let raw = shared.native.borrow_mut().parent_ref.take();
  if let Some(raw) = raw {
    unsafe {
      drop(Rc::from_raw(raw as *const Shared));
    }
  }
  Ok(())
}

unsafe extern "system" fn parent_proc(
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
  let controller = shared.native.borrow().controller.clone();
  if let Some(controller) = controller {
    let result = match message {
      WM_SIZE if wparam.0 != SIZE_MINIMIZED as usize => match InnerWebView::parent_bounds(hwnd) {
        Ok(size) => {
          let container = shared.native.borrow().hwnd;
          if let Some(container) = container {
            InnerWebView::set_bounds_native(&controller, container, size, (0, 0).into())
          } else {
            Ok(())
          }
        }
        Err(cause) => Err(cause),
      },
      WM_SETFOCUS | WM_ENTERSIZEMOVE => controller
        .MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC)
        .map_err(Into::into),
      WM_MOVE | WM_MOVING => controller
        .NotifyParentWindowPositionChanged()
        .map_err(Into::into),
      _ => Ok(()),
    };
    if let Err(cause) = result {
      shared.fail(cause);
    }
  }
  let result = DefSubclassProc(hwnd, message, wparam, lparam);
  if message == WM_NCDESTROY {
    retirement_reference::destroyed(&shared, retirement_reference::Reference::Parent);
  }
  shared.callback_depth.set(shared.callback_depth.get() - 1);
  result
}
