// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener

//! Checkpoint fakes exercise actual constructor gates without COM or window effects.

use super::*;
use owned_state::{Shared, Stage};
use std::cell::Cell;

#[test]
fn owned_diagnostic_panic_cannot_drop_the_already_retained_native_transaction() {
  struct Sentinel(Rc<Cell<u32>>);
  impl Drop for Sentinel {
    fn drop(&mut self) {
      self.0.set(self.0.get() + 1);
    }
  }
  impl Sentinel {
    fn touch(&self) {
      let _ = self.0.get();
    }
  }
  let dropped = Rc::new(Cell::new(0));
  let sentinel = Sentinel(dropped.clone());
  let owner = Shared::new(HWND::default(), move || sentinel.touch());
  owner.native.borrow_mut().ledger.pending = 1;
  let observed = Rc::downgrade(&owner);
  let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    retirement::retain_before_diagnostic(owner, || panic!("injected subscriber failure"));
  }));
  assert!(result.is_err());
  assert_eq!(dropped.get(), 0);
  assert_eq!(
    observed
      .upgrade()
      .map(|owner| owner.native.borrow().ledger.pending),
    Some(1)
  );
}

#[test]
fn owned_optional_interfaces_preserve_unexpected_native_failures_before_admission() -> Result<()> {
  assert_eq!(construction_admission::optional(Ok(7))?, Some(7));
  assert!(
    construction_admission::optional::<u32>(Err(windows::core::Error::from(E_NOINTERFACE)))?
      .is_none()
  );
  let armed = Cell::new(false);
  let result = construction_admission::submit(
    || construction_admission::optional::<u32>(Err(windows::core::Error::from(E_ACCESSDENIED))),
    || {
      armed.set(true);
      Ok(())
    },
    |_| Ok(()),
  );
  assert!(!armed.get());
  match result {
    Err(Error::WebView2Error(webview2_com::Error::WindowsError(cause))) => {
      assert_eq!(cause.code(), E_ACCESSDENIED)
    }
    _ => panic!("lost unexpected native interface cause"),
  }
  Ok(())
}

fn construction(shared: Rc<Shared>) -> Result<WebViewConstruction> {
  let mut config = owned_config::Config::new(
    WebViewAttributes::default(),
    super::super::PlatformSpecificWebViewAttributes::default(),
    false,
    true,
  )?;
  config.owned = true;
  Ok(WebViewConstruction {
    driver: Some(Box::new(construction::Driver { config, shared })),
  })
}

#[test]
fn owned_native_environment_adoption_pauses_before_every_controller_effect() -> Result<()> {
  let shared = Shared::new(HWND::default(), || {});
  shared.native.borrow_mut().ledger.pending = 1;
  let mut output = None;
  shared.on_callback(|state| {
    construction_adoption::adopt(
      &mut output,
      Some(()),
      Ok(()),
      &mut state.ledger,
      &mut state.failure,
      Stage::EnvironmentPrepared,
      false,
    )
  });
  let mut construction = construction(shared.clone())?;
  for _ in 0..3 {
    assert_eq!(
      construction.progress()?,
      ConstructionProgress::EnvironmentPrepared
    );
  }
  assert!(!shared.native.borrow().ledger.controller);
  assert!(construction.authorize_environment().is_err());
  assert_eq!(
    shared.native.borrow().ledger.stage,
    Stage::EnvironmentPrepared
  );
  shared.native.borrow_mut().folder_queried = true;
  construction.authorize_environment()?;
  assert_eq!(shared.native.borrow().ledger.stage, Stage::Controller);
  assert!(construction.authorize_environment().is_err());
  assert!(retirement::step_with(&shared, |_| Ok(()))?);
  Ok(())
}

#[test]
fn owned_checkpoint_cancel_callback_or_inflight_call_cannot_authorize() -> Result<()> {
  for kind in 0..3 {
    let shared = Shared::new(HWND::default(), || {});
    shared.native.borrow_mut().ledger.stage = Stage::EnvironmentPrepared;
    shared.native.borrow_mut().folder_queried = true;
    let mut construction = construction(shared.clone())?;
    if kind == 0 {
      construction.cancel();
    } else if kind == 1 {
      shared.callback_depth.set(1);
    } else {
      shared.busy.set(true);
    }
    assert!(construction.authorize_environment().is_err());
    assert!(construction.user_data_folder().is_err());
    assert_eq!(
      shared.native.borrow().ledger.stage,
      Stage::EnvironmentPrepared
    );
    shared.callback_depth.set(0);
    shared.busy.set(false);
    assert!(retirement::step_with(&shared, |_| Ok(()))?);
  }
  Ok(())
}

struct Returned(Rc<Cell<u32>>);
impl Drop for Returned {
  fn drop(&mut self) {
    self.0.set(self.0.get() + 1);
  }
}

#[test]
fn owned_failed_folder_hresult_frees_actual_output_without_scanning_it() {
  let freed = Rc::new(Cell::new(0));
  let copied = Cell::new(false);
  let result = construction_environment::consume_query(
    Returned(freed.clone()),
    Err(windows::core::Error::from(E_ACCESSDENIED)),
    |_| {
      copied.set(true);
      Err(owned_config::invalid("unexpected scan"))
    },
  );
  assert_eq!(freed.get(), 1);
  assert!(!copied.get());
  match result {
    Err(Error::WebView2Error(webview2_com::Error::WindowsError(cause))) => {
      assert_eq!(cause.code(), E_ACCESSDENIED)
    }
    _ => panic!("lost original native folder failure"),
  }
}

#[test]
fn owned_folder_copy_preserves_exact_utf16_and_enforces_terminator_inclusive_limit() -> Result<()> {
  let source = [b'E' as u16, b':' as u16, b'\\' as u16, 0xd800, 0];
  let folder = construction_environment::copy_folder(|index| Ok(source[index]))?;
  assert_eq!(folder.as_utf16(), &source[..source.len() - 1]);
  let mut maximum = vec![b'x' as u16; UserDataFolder::MAX_UNITS];
  maximum[UserDataFolder::MAX_UNITS - 1] = 0;
  let folder = construction_environment::copy_folder(|index| Ok(maximum[index]))?;
  assert_eq!(folder.as_utf16().len(), UserDataFolder::MAX_UNITS - 1);
  assert_eq!(
    folder.into_utf16().capacity(),
    UserDataFolder::MAX_UNITS - 1
  );
  assert!(construction_environment::copy_folder(|_| Ok(b'x' as u16)).is_err());
  assert!(construction_environment::copy_folder(|_| Ok(0)).is_err());
  Ok(())
}

#[test]
fn owned_recorded_close_stops_the_next_initialization_effect() -> Result<()> {
  let shared = Shared::new(HWND::default(), || {});
  let effects = Cell::new(0);
  shared.initialization_effect(|| {
    effects.set(effects.get() + 1);
    shared.request_native_close();
    Ok(())
  })?;
  assert!(shared
    .initialization_effect(|| {
      effects.set(effects.get() + 1);
      Ok(())
    })
    .is_err());
  assert_eq!(effects.get(), 1);
  assert!(retirement::step_with(&shared, |_| Ok(()))?);
  Ok(())
}
