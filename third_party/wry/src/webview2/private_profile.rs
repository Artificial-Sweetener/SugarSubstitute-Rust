// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the private environment alive through its actual browser-collection exit notification.
use super::*;
use std::cell::Cell;

/// UI-affine observation for an exclusively owned disposable browser profile.
/// Controller Close and window retirement never imply browser-collection retirement.
#[must_use]
pub struct PrivateProfileObserver {
  state: Rc<Observation>,
}
struct Observation {
  environment: ICoreWebView2Environment5,
  token: Cell<Option<i64>>,
  expected: Cell<Option<u32>>,
  exited: Cell<Option<u32>>,
}
impl PrivateProfileObserver {
  /// Observe the exact private profile's collection while retaining its environment.
  pub(super) fn install(environment: &ICoreWebView2Environment) -> Result<Self> {
    let state = Rc::new(Observation {
      environment: environment.cast()?, token: Cell::new(None),
      expected: Cell::new(None), exited: Cell::new(None),
    });
    let observer = Rc::downgrade(&state);
    let callback = BrowserProcessExitedEventHandler::create(Box::new(move |_, arguments| {
      if let (Some(state), Some(arguments)) = (observer.upgrade(), arguments) {
        let mut process = 0;
        // The callback's event args are retained by WebView2 for the duration of this call.
        unsafe { arguments.BrowserProcessId(&mut process)?; }
        if process != 0 { state.exited.set(Some(process)); }
      }
      Ok(())
    }));
    let mut token = 0;
    unsafe { state.environment.add_BrowserProcessExited(&callback, &mut token)?; }
    state.token.set(Some(token));
    Ok(Self { state })
  }
  /// Match the actual controller output, including an output delivered during cancellation.
  pub(super) fn controller(&self, controller: &ICoreWebView2Controller) -> Result<()> {
    let mut process = 0;
    unsafe { controller.CoreWebView2()?.BrowserProcessId(&mut process)?; }
    if process == 0 { return Err(owned_config::invalid("private browser process identity absent")); }
    self.state.expected.set(Some(process));
    Ok(())
  }
  /// Whether an actual controller supplied the browser identity needed to match native exit.
  pub fn has_controller_identity(&self) -> bool { self.state.expected.get().is_some() }
  /// True only after the native collection-exit event matches this actual controller's browser.
  /// Call after checked retirement released all views and controllers, on the same UI thread.
  pub fn released(&self) -> bool {
    self.state.expected.get().is_some_and(|process| self.state.exited.get() == Some(process))
  }
  /// Remove the actual event registration after retirement or a bounded unproven observation.
  /// Return the release fact before relinquishing the environment. False preserves the cache pin.
  pub fn finish(&mut self) -> Result<bool> {
    let released = self.released();
    if let Some(token) = self.state.token.get() {
      unsafe { self.state.environment.remove_BrowserProcessExited(token)?; }
      self.state.token.set(None);
    }
    Ok(released)
  }
  /// Retain the same observation through late native controller completion.
  pub(super) fn retained(&self) -> Self { Self { state: self.state.clone() } }
}
