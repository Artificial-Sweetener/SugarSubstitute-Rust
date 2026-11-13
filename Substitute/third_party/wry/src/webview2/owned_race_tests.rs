// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener

//! Reentrant boundary fakes use the same state owners as native callbacks.

use super::*;
use owned_state::{Shared, Stage};
use std::cell::Cell;

fn config() -> Result<owned_config::Config> {
  owned_config::Config::new(
    WebViewAttributes::default(),
    super::super::PlatformSpecificWebViewAttributes::default(),
    false,
    true,
  )
}
fn failed() -> Error {
  windows::core::Error::from(E_ACCESSDENIED).into()
}

#[test]
fn owned_preparation_failure_never_arms_or_invokes_native_factory() -> Result<()> {
  let shared = Shared::new(HWND::default(), || {});
  let called = Cell::new(false);
  let result = construction_admission::submit::<()>(
    || Err(failed()),
    || shared.arm_creation(Stage::AwaitEnvironment),
    |_| {
      called.set(true);
      Ok(())
    },
  );
  assert!(matches!(result, Err(Error::WebView2Error(_))));
  assert!(!called.get());
  assert_eq!(shared.native.borrow().ledger.pending, 0);
  assert!(retirement::step_with(&shared, |_| Ok(()))?);
  Ok(())
}

#[test]
fn owned_arm_precedes_synchronous_creation_callback_and_does_not_reset_it() -> Result<()> {
  let shared = Shared::new(HWND::default(), || {});
  let mut output = None;
  construction_admission::submit(
    || Ok(37u32),
    || shared.arm_creation(Stage::AwaitEnvironment),
    |actual| {
      shared.on_callback(|state| {
        construction_adoption::adopt(
          &mut output,
          Some(actual),
          Ok(()),
          &mut state.ledger,
          &mut state.failure,
          Stage::Controller,
          false,
        )
      });
      assert_eq!(shared.native.borrow().ledger.pending, 0);
      Ok(())
    },
  )?;
  assert_eq!(output, Some(37));
  assert_eq!(shared.native.borrow().ledger.stage, Stage::Controller);
  assert!(retirement::step_with(&shared, |_| Ok(()))?);
  Ok(())
}

#[test]
fn owned_native_close_intent_forbids_stage_and_success_transfer() -> Result<()> {
  let shared = Shared::new(HWND::default(), || {});
  shared.native.borrow_mut().ledger.stage = Stage::Finished;
  let mut construction = WebViewConstruction {
    driver: Some(Box::new(construction::Driver {
      config: config()?,
      shared: shared.clone(),
    })),
  };
  shared.request_native_close();
  assert!(construction.take_close_requested());
  assert!(!construction.take_close_requested());
  assert_eq!(construction.progress()?, ConstructionProgress::Pending);
  assert!(construction.take_result().is_none());
  assert!(construction.checked_retire()?);
  Ok(())
}

#[test]
fn owned_late_failure_after_cancel_is_finished_and_transfers_original_cause_once() -> Result<()> {
  let shared = Shared::new(HWND::default(), || {});
  shared.native.borrow_mut().ledger.pending = 1;
  let mut construction = WebViewConstruction {
    driver: Some(Box::new(construction::Driver {
      config: config()?,
      shared: shared.clone(),
    })),
  };
  construction.cancel();
  let mut output = None;
  shared.on_callback(|state| {
    construction_adoption::adopt(
      &mut output,
      Some(()),
      Err(windows::core::Error::from(E_ACCESSDENIED)),
      &mut state.ledger,
      &mut state.failure,
      Stage::Controller,
      false,
    )
  });
  assert_eq!(construction.progress()?, ConstructionProgress::Finished);
  let failure = construction
    .take_result()
    .ok_or_else(|| owned_config::invalid("missing late failure"))?
    .err()
    .ok_or_else(|| owned_config::invalid("unexpected success"))?;
  assert!(construction.take_result().is_none());
  let (cause, mut retirement) = failure.into_parts();
  match cause {
    Error::WebView2Error(webview2_com::Error::WindowsError(cause)) => {
      assert_eq!(cause.code(), E_ACCESSDENIED)
    }
    _ => panic!("lost late native cause"),
  }
  assert!(retirement.checked_retire()?);
  Ok(())
}

#[test]
fn owned_synchronous_destroy_during_install_reclaims_exactly_once_even_on_failure() -> Result<()> {
  for kind in [
    retirement_reference::Reference::Container,
    retirement_reference::Reference::Parent,
  ] {
    for fails in [false, true] {
      let shared = Shared::new(HWND::default(), || {});
      shared.native.borrow_mut().ledger.window = true;
      let count = Rc::strong_count(&shared);
      let result = retirement_reference::install(&shared, kind, |_| {
        assert_eq!(Rc::strong_count(&shared), count + 1);
        retirement_reference::destroyed(&shared, kind);
        assert_eq!(Rc::strong_count(&shared), count);
        if fails {
          Err(failed())
        } else {
          Ok(())
        }
      });
      assert_eq!(result.is_err(), fails);
      assert_eq!(Rc::strong_count(&shared), count);
      assert!(shared.native.borrow().container_ref.is_none());
      assert!(shared.native.borrow().parent_ref.is_none());
      assert!(!shared.native.borrow().ledger.parent_subclass);
      assert!(shared.native.borrow().ledger.retiring);
      assert!(shared.close_requested.get());
      shared.native.borrow_mut().ledger.window = false;
      assert!(retirement::step_with(&shared, |_| Ok(()))?);
    }
  }
  Ok(())
}
