// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener

//! Nonblocking UI-thread construction; callbacks record outputs and wake only.

use super::*;
use owned_state::{Shared, Stage};
use std::fmt;

pub(crate) fn begin_builder(
  parent: &impl HasWindowHandle,
  attrs: WebViewAttributes<'_>,
  platform: super::super::PlatformSpecificWebViewAttributes,
  error: Result<()>,
  wake: impl Fn() + 'static,
) -> std::result::Result<WebViewConstruction, WebViewBuildFailure> {
  if let Err(error) = error {
    let shared = Shared::new(HWND::default(), wake);
    shared.native.borrow_mut().ledger.retired = true;
    return Err(WebViewBuildFailure {
      error,
      retirement: retirement::WebViewRetirement::empty(shared),
    });
  }
  WebViewConstruction::begin(parent, attrs, platform, wake)
}

/// Progress of one constructor stage, without pumping or waiting for callbacks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstructionProgress {
  /// A bounded stage advanced; call again on a subsequent native drain.
  Advanced,
  /// Native creation is outstanding; normal native message pumping must continue.
  Pending,
  /// A native environment exists, but owned controller admission awaits trusted profile validation.
  /// Query its bounded effective folder and authorize only through the native desktop owner.
  EnvironmentPrepared,
  /// Exactly one success or failure result is available from `take_result`.
  /// This does not establish document readiness or resource retirement.
  Finished,
}

pub(super) struct Driver {
  pub config: owned_config::Config,
  pub shared: Rc<Shared>,
}

/// UI-affine, nonblocking child construction with explicitly retained partial resources.
///
/// The parent window and original host deadline must outlive checked retirement.
/// The outer UI owner must initialize and retain its STA across all COM references.
/// Drive `progress` from the normal event-loop drain, never a WebView2 callback.
/// Dropping an unresolved owner retains the transaction until process death.
#[must_use]
pub struct WebViewConstruction {
  pub(super) driver: Option<Box<Driver>>,
}

/// Original construction cause together with independent checked retirement ownership.
#[must_use]
pub struct WebViewBuildFailure {
  error: Error,
  retirement: retirement::WebViewRetirement,
}

impl fmt::Debug for WebViewBuildFailure {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("WebViewBuildFailure")
      .field("error", &self.error)
      .finish_non_exhaustive()
  }
}
impl fmt::Display for WebViewBuildFailure {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    self.error.fmt(f)
  }
}
impl std::error::Error for WebViewBuildFailure {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    Some(&self.error)
  }
}
impl WebViewBuildFailure {
  /// Borrow the unchanged native or input failure cause.
  pub fn error(&self) -> &Error {
    &self.error
  }
  /// Transfer the original cause once, retaining all partial native resources separately.
  pub fn into_parts(self) -> (Error, retirement::WebViewRetirement) {
    (self.error, self.retirement)
  }
}

impl WebViewConstruction {
  pub(crate) fn begin(
    parent: &impl HasWindowHandle,
    attrs: WebViewAttributes<'_>,
    platform: super::super::PlatformSpecificWebViewAttributes,
    wake: impl Fn() + 'static,
  ) -> std::result::Result<Self, WebViewBuildFailure> {
    let prepared = (|| {
      let hwnd = match parent.window_handle()?.as_raw() {
        RawWindowHandle::Win32(handle) => HWND(handle.hwnd.get() as _),
        _ => return Err(Error::UnsupportedWindowHandle),
      };
      let config = owned_config::Config::new(attrs, platform, true, true)?;
      Ok((hwnd, config))
    })();
    match prepared {
      Ok((parent, config)) => {
        // No HWND/COM effect occurred; transfer the prevalidated owned inputs now.
        let shared = Shared::new(parent, wake);
        Ok(Self {
          driver: Some(Box::new(Driver { shared, config })),
        })
      }
      Err(error) => {
        let shared = Shared::new(HWND::default(), wake);
        shared.native.borrow_mut().ledger.retired = true;
        Err(WebViewBuildFailure {
          error,
          retirement: retirement::WebViewRetirement::empty(shared),
        })
      }
    }
  }

  /// Execute at most one bounded initialization stage on the owning UI thread.
  /// Operational errors are retained for `take_result`; errors returned here signal misuse.
  pub fn progress(&mut self) -> Result<ConstructionProgress> {
    let driver = self
      .driver
      .as_mut()
      .ok_or_else(|| owned_config::invalid("construction result already transferred"))?;
    if driver.shared.callback_depth.get() != 0 || driver.shared.busy.get() {
      return Err(owned_config::invalid(
        "construction progress from native callback",
      ));
    }
    if driver.shared.native.borrow().ledger.retiring {
      if driver.shared.native.borrow().failure.is_some() {
        return Ok(ConstructionProgress::Finished);
      }
      return Ok(ConstructionProgress::Pending);
    }
    match construction_effects::advance(driver) {
      Ok(progress) => Ok(progress),
      Err(error) => {
        driver.shared.fail(error);
        Ok(ConstructionProgress::Finished)
      }
    }
  }

  /// Transfer the completed webview, or the original failure and partial-resource owner, once.
  pub fn take_result(
    &mut self,
  ) -> Option<std::result::Result<crate::WebView, WebViewBuildFailure>> {
    let driver = self.driver.as_ref()?;
    if driver.shared.busy.get()
      || driver.shared.callback_depth.get() != 0
      || driver.shared.native.borrow().ledger.stage != Stage::Finished
    {
      return None;
    }
    if driver.shared.native.borrow().ledger.retiring
      && driver.shared.native.borrow().failure.is_none()
    {
      return None;
    }
    let driver = self.driver.take()?;
    let cause = driver.shared.native.borrow_mut().failure.take();
    if let Some(error) = cause {
      return Some(Err(WebViewBuildFailure {
        error,
        retirement: retirement::WebViewRetirement::new(driver),
      }));
    }
    match construction_effects::completed_inner(&driver) {
      Ok(webview) => Some(Ok(crate::WebView { webview })),
      Err(error) => Some(Err(WebViewBuildFailure {
        error,
        retirement: retirement::WebViewRetirement::new(driver),
      })),
    }
  }

  /// Close initialization admission. Late callbacks remain owned and cannot initialize or navigate.
  pub fn cancel(&mut self) {
    if let Some(driver) = &self.driver {
      driver.shared.native.borrow_mut().ledger.retiring = true;
    }
  }
  /// Consume a native close intent while construction is still pending.
  /// The callback has already closed initialization admission without performing cleanup.
  pub fn take_close_requested(&self) -> bool {
    self
      .driver
      .as_ref()
      .is_some_and(|driver| driver.shared.close_requested.replace(false))
  }
  /// Request cancellation and perform one checked retirement step without waiting.
  /// `false` retains resources and pending native callbacks; native failure preserves its slot.
  pub fn checked_retire(&mut self) -> Result<bool> {
    self.cancel();
    match &self.driver {
      Some(driver) => retirement::step(&driver.shared),
      None => Ok(true),
    }
  }
}

impl Drop for WebViewConstruction {
  fn drop(&mut self) {
    if let Some(driver) = self.driver.take() {
      retirement::drop_driver(driver);
    }
  }
}

// The derivative's convenience path shares the outer initialized-STA precondition.
// Its synchronous driver pumps only after resource ownership has been recorded.
pub(super) fn legacy(parent: HWND, config: owned_config::Config) -> Result<InnerWebView> {
  let pending_wake: Rc<RefCell<Option<mpsc::Sender<()>>>> = Rc::new(RefCell::new(None));
  let wake_target = pending_wake.clone();
  let shared = Shared::new(parent, move || {
    if let Some(sender) = wake_target.borrow().as_ref() {
      let _ = sender.send(());
    }
  });
  let mut construction = WebViewConstruction {
    driver: Some(Box::new(Driver { shared, config })),
  };
  loop {
    let (tx, rx) = mpsc::channel();
    *pending_wake.borrow_mut() = Some(tx);
    match construction.progress()? {
      ConstructionProgress::Advanced => {}
      ConstructionProgress::Pending => {
        webview2_com::wait_with_pump(rx)?;
      }
      ConstructionProgress::EnvironmentPrepared => {
        return Err(owned_config::invalid(
          "legacy environment unexpectedly requires authorization",
        ))
      }
      ConstructionProgress::Finished => break,
    }
  }
  match construction
    .take_result()
    .ok_or_else(|| owned_config::invalid("construction finished without result"))?
  {
    Ok(view) => Ok(view.webview),
    Err(failure) => {
      let (error, mut owner) = failure.into_parts();
      for _ in 0..6 {
        match owner.checked_retire() {
          Ok(true) => break,
          Ok(false) => {}
          Err(cleanup) => {
            #[cfg(feature = "tracing")]
            tracing::error!(operation="legacy_build_retirement", error=%cleanup);
            break;
          }
        }
      }
      Err(error)
    }
  }
}
