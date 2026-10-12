// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener

//! UI-thread construction and retirement decisions shared with native effects.

use super::*;
use std::cell::Cell;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Stage {
  Container,
  Environment,
  AwaitEnvironment,
  EnvironmentPrepared,
  Controller,
  AwaitController,
  Initialize,
  Bounds,
  Finished,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RetireAction {
  Busy,
  RemoveClose,
  RemovePopup,
  Close,
  RemoveParent,
  RemoveDispatcher,
  Destroy,
  Release,
  Done,
}

pub(super) struct Ledger {
  pub stage: Stage,
  pub retiring: bool,
  pub retired: bool,
  pub pending: usize,
  pub controller: bool,
  pub closed: bool,
  pub parent_subclass: bool,
  pub dispatcher: bool,
  pub window: bool,
  pub close_event: bool,
  pub popup_event: bool,
}

impl Ledger {
  pub fn new() -> Self {
    Self {
      stage: Stage::Container,
      retiring: false,
      retired: false,
      pending: 0,
      controller: false,
      closed: false,
      parent_subclass: false,
      dispatcher: false,
      window: false,
      close_event: false,
      popup_event: false,
    }
  }
  pub fn action(&self, busy: bool) -> RetireAction {
    if self.retired {
      RetireAction::Done
    } else if busy || self.pending != 0 {
      RetireAction::Busy
    } else if self.close_event {
      RetireAction::RemoveClose
    } else if self.popup_event {
      RetireAction::RemovePopup
    } else if self.controller && !self.closed {
      RetireAction::Close
    } else if self.parent_subclass {
      RetireAction::RemoveParent
    } else if self.dispatcher {
      RetireAction::RemoveDispatcher
    } else if self.window {
      RetireAction::Destroy
    } else {
      RetireAction::Release
    }
  }
  pub fn commit(&mut self, action: RetireAction) {
    match action {
      RetireAction::Close => self.closed = true,
      RetireAction::RemoveClose => self.close_event = false,
      RetireAction::RemovePopup => self.popup_event = false,
      RetireAction::RemoveParent => self.parent_subclass = false,
      RetireAction::RemoveDispatcher => self.dispatcher = false,
      RetireAction::Destroy => self.window = false,
      RetireAction::Release => self.retired = true,
      RetireAction::Busy | RetireAction::Done => {}
    }
  }
}

pub(super) struct NativeState {
  pub ledger: Ledger,
  pub hwnd: Option<HWND>,
  pub env: Option<ICoreWebView2Environment>,
  pub controller: Option<ICoreWebView2Controller>,
  pub webview: Option<ICoreWebView2>,
  pub failure: Option<Error>,
  pub parent_ref: Option<usize>,
  pub container_ref: Option<usize>,
  pub close_token: Option<i64>,
  pub popup_token: Option<i64>,
  pub drag_drop: Option<DragDropController>,
  pub folder_queried: bool,
}

pub(super) struct Shared {
  pub native: RefCell<NativeState>,
  pub parent: Cell<HWND>,
  pub busy: Cell<bool>,
  pub callback_depth: Cell<usize>,
  pub close_requested: Cell<bool>,
  pub wake: Box<dyn Fn()>,
}

impl Shared {
  pub fn new(parent: HWND, wake: impl Fn() + 'static) -> Rc<Self> {
    Rc::new(Self {
      native: RefCell::new(NativeState {
        ledger: Ledger::new(),
        hwnd: None,
        env: None,
        controller: None,
        webview: None,
        failure: None,
        parent_ref: None,
        container_ref: None,
        close_token: None,
        popup_token: None,
        drag_drop: None,
        folder_queried: false,
      }),
      parent: Cell::new(parent),
      busy: Cell::new(false),
      callback_depth: Cell::new(0),
      close_requested: Cell::new(false),
      wake: Box::new(wake),
    })
  }
  pub fn fail(&self, error: Error) {
    let mut native = self.native.borrow_mut();
    if native.failure.is_none() {
      native.failure = Some(error);
    } else {
      #[cfg(feature = "tracing")]
      tracing::error!(operation="webview_construction_secondary", error=%error);
    }
    native.ledger.stage = Stage::Finished;
  }
  pub fn begin_call(&self) -> Result<CallGuard<'_>> {
    if self.callback_depth.get() != 0 || self.busy.replace(true) {
      return Err(owned_config::invalid("webview operation is reentrant"));
    }
    Ok(CallGuard(self))
  }
  pub fn on_callback(&self, record: impl FnOnce(&mut NativeState)) {
    let _guard = self.callback_guard();
    record(&mut self.native.borrow_mut());
    (self.wake)();
  }
  pub fn callback_guard(&self) -> CallbackGuard<'_> {
    self.callback_depth.set(self.callback_depth.get() + 1);
    CallbackGuard(self)
  }
  pub fn request_native_close(&self) {
    self.on_callback(|state| {
      self.close_requested.set(true);
      state.ledger.retiring = true;
    });
  }
  pub fn arm_creation(&self, waiting: Stage) -> Result<()> {
    let mut state = self.native.borrow_mut();
    if state.ledger.retiring {
      return Err(windows::core::Error::from(E_ABORT).into());
    }
    if state.ledger.pending != 0 {
      return Err(owned_config::invalid("creation already outstanding"));
    }
    state.ledger.pending = 1;
    state.ledger.stage = waiting;
    Ok(())
  }
  pub fn initialization_effect<T>(&self, effect: impl FnOnce() -> Result<T>) -> Result<T> {
    let state = self.native.borrow();
    if state.ledger.retiring || state.failure.is_some() {
      return Err(windows::core::Error::from(E_ABORT).into());
    }
    drop(state);
    effect()
  }
  pub fn optional_interface<T: Interface>(&self, source: &impl Interface) -> Result<Option<T>> {
    self.initialization_effect(|| construction_admission::optional(source.cast::<T>()))
  }
}

pub(super) struct CallGuard<'a>(&'a Shared);
impl Drop for CallGuard<'_> {
  fn drop(&mut self) {
    self.0.busy.set(false);
  }
}
pub(super) struct CallbackGuard<'a>(&'a Shared);
impl Drop for CallbackGuard<'_> {
  fn drop(&mut self) {
    self.0.callback_depth.set(self.0.callback_depth.get() - 1);
  }
}
