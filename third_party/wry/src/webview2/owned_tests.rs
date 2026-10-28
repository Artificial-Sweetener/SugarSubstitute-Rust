// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener

//! Deterministic boundary fakes exercise the production adoption/retirement owners.

use super::*;
use owned_state::{Ledger, RetireAction, Shared, Stage};
use std::cell::Cell;

fn shared() -> Rc<Shared> {
  Shared::new(HWND::default(), || {})
}
fn failed() -> Error {
  windows::core::Error::from(E_ACCESSDENIED).into()
}
struct Resource(Rc<Cell<u32>>);
impl Drop for Resource {
  fn drop(&mut self) {
    self.0.set(self.0.get() + 1);
  }
}

#[test]
fn owned_failed_hresult_retains_actual_partial_environment_and_controller() {
  for controller in [false, true] {
    let released = Rc::new(Cell::new(0));
    let mut slot = None;
    let mut ledger = Ledger::new();
    ledger.pending = 1;
    let mut cause = None;
    construction_adoption::adopt(
      &mut slot,
      Some(Resource(released.clone())),
      Err(windows::core::Error::from(E_ACCESSDENIED)),
      &mut ledger,
      &mut cause,
      Stage::Initialize,
      controller,
    );
    assert!(slot.is_some());
    assert_eq!(released.get(), 0);
    assert_eq!(ledger.stage, Stage::Finished);
    assert_eq!(ledger.controller, controller);
    assert!(matches!(cause, Some(Error::WebView2Error(_))));
    drop(slot);
    assert_eq!(released.get(), 1);
  }
}

#[test]
fn owned_cancel_pending_adopts_late_output_without_initializing() {
  let mut ledger = Ledger::new();
  ledger.stage = Stage::AwaitController;
  ledger.pending = 1;
  ledger.retiring = true;
  let mut slot = None;
  let mut cause = None;
  construction_adoption::adopt(
    &mut slot,
    Some(37u32),
    Ok(()),
    &mut ledger,
    &mut cause,
    Stage::Initialize,
    true,
  );
  assert_eq!(slot, Some(37));
  assert_eq!(ledger.stage, Stage::AwaitController);
  assert_eq!(ledger.pending, 0);
  assert_eq!(ledger.action(false), RetireAction::Close);
  assert!(cause.is_none());
}

#[test]
fn composition_failed_or_late_output_requires_resolution_before_close() {
  for failed in [false, true] {
    let released = Rc::new(Cell::new(0));
    let mut ledger = Ledger::new();
    ledger.stage = Stage::AwaitController;
    ledger.pending = 1;
    ledger.retiring = true;
    let mut slot = None;
    let mut cause = None;
    construction_composition::adopt(
      &mut slot,
      Some(Resource(released.clone())),
      if failed { Err(windows::core::Error::from(E_ACCESSDENIED)) } else { Ok(()) },
      &mut ledger,
      &mut cause,
    );
    assert_eq!(ledger.pending, 0);
    assert_eq!(ledger.action(false), RetireAction::ResolveComposition);
    assert_eq!(released.get(), 0);
    assert_eq!(cause.is_some(), failed);
    ledger.commit(RetireAction::ResolveComposition);
    assert_eq!(ledger.action(false), RetireAction::DetachComposition);
    ledger.commit(RetireAction::DetachComposition);
    assert_eq!(ledger.action(false), RetireAction::Close);
    ledger.commit(RetireAction::Close);
    assert_eq!(ledger.action(false), RetireAction::Release);
    drop(slot);
    assert_eq!(released.get(), 1);
  }
}

#[test]
fn composition_resolution_failure_keeps_the_cleanup_obligation() -> Result<()> {
  let shared = shared();
  shared.native.borrow_mut().ledger.composition_pending = true;
  shared.native.borrow_mut().ledger.composition_detach_pending = true;
  assert!(retirement::step_with(&shared, |action| {
    assert_eq!(action, RetireAction::ResolveComposition);
    Err(failed())
  }).is_err());
  assert_eq!(shared.native.borrow().ledger.action(false), RetireAction::ResolveComposition);
  assert!(!shared.native.borrow().ledger.retired);
  retirement::step_with(&shared, |action| {
    assert_eq!(action, RetireAction::ResolveComposition);
    Ok(())
  })?;
  assert_eq!(shared.native.borrow().ledger.action(false), RetireAction::DetachComposition);
  assert!(retirement::step_with(&shared, |_| Err(failed())).is_err());
  assert_eq!(shared.native.borrow().ledger.action(false), RetireAction::DetachComposition);
  retirement::step_with(&shared, |_| Ok(()))?;
  assert_eq!(shared.native.borrow().ledger.action(false), RetireAction::Close);
  retirement::step_with(&shared, |_| Ok(()))?;
  assert!(retirement::step_with(&shared, |_| Ok(()))?);
  Ok(())
}

#[test]
fn owned_callback_before_submission_returns_preserves_finished_original_failure() {
  let mut ledger = Ledger::new();
  ledger.stage = Stage::AwaitEnvironment;
  ledger.pending = 1;
  let mut slot = None;
  let mut cause = None;
  construction_adoption::adopt(
    &mut slot,
    Some(1u32),
    Ok(()),
    &mut ledger,
    &mut cause,
    Stage::Controller,
    false,
  );
  assert_eq!(ledger.stage, Stage::Controller);
  assert_eq!(ledger.pending, 0);
  let shared = shared();
  shared.native.borrow_mut().ledger = ledger;
  shared.fail(failed());
  assert_eq!(shared.native.borrow().ledger.stage, Stage::Finished);
  shared.native.borrow_mut().ledger.retired = true;
}

#[test]
fn owned_each_cleanup_failure_preserves_exact_slot_until_successful_retry() -> Result<()> {
  for failed_action in [
    RetireAction::RemoveClose,
    RetireAction::RemovePopup,
    RetireAction::Close,
    RetireAction::RemoveParent,
    RetireAction::RemoveDispatcher,
    RetireAction::Destroy,
  ] {
    let shared = shared();
    {
      let mut state = shared.native.borrow_mut();
      state.ledger.close_event = true;
      state.ledger.popup_event = true;
      state.ledger.controller = true;
      state.ledger.parent_subclass = true;
      state.ledger.dispatcher = true;
      state.ledger.window = true;
    }
    let mut attempts = Vec::new();
    while shared.native.borrow().ledger.action(false) != failed_action {
      retirement::step_with(&shared, |action| {
        attempts.push(action);
        Ok(())
      })?;
    }
    let result = retirement::step_with(&shared, |action| {
      attempts.push(action);
      Err(failed())
    });
    assert!(matches!(result, Err(Error::WebView2Error(_))));
    assert_eq!(shared.native.borrow().ledger.action(false), failed_action);
    assert!(!shared.native.borrow().ledger.retired);
    retirement::step_with(&shared, |action| {
      attempts.push(action);
      Ok(())
    })?;
    assert_eq!(attempts.iter().filter(|&&a| a == failed_action).count(), 2);
    while !retirement::step_with(&shared, |_| Ok(()))? {}
    assert!(retirement::step_with(&shared, |_| Err(failed()))?);
  }
  Ok(())
}

#[test]
fn owned_actual_pending_and_reentrant_callbacks_prevent_retirement_effects() -> Result<()> {
  let shared = shared();
  shared.native.borrow_mut().ledger.pending = 1;
  assert!(!retirement::step_with(&shared, |_| panic!(
    "pending callback executed cleanup"
  ))?);
  shared.native.borrow_mut().ledger.pending = 0;
  let active = shared.begin_call()?;
  assert!(!retirement::step_with(&shared, |_| panic!(
    "inflight native call executed cleanup"
  ))?);
  drop(active);
  {
    let callback = shared.callback_guard();
    assert!(!retirement::step_with(&shared, |_| panic!(
      "native callback executed cleanup"
    ))?);
    drop(callback);
  }
  assert!(retirement::step_with(&shared, |_| Ok(()))?);
  Ok(())
}

#[test]
fn owned_wake_observes_recorded_output_without_retained_state_borrow() {
  let target: Rc<RefCell<Option<Rc<Shared>>>> = Rc::new(RefCell::new(None));
  let observed = Rc::new(Cell::new(false));
  let captured = target.clone();
  let result = observed.clone();
  let shared = Shared::new(HWND::default(), move || {
    if let Some(shared) = captured.borrow().as_ref() {
      result.set(
        shared.native.borrow().ledger.stage == Stage::Controller
          && shared.callback_depth.get() == 1,
      );
    }
  });
  *target.borrow_mut() = Some(shared.clone());
  shared.on_callback(|state| state.ledger.stage = Stage::Controller);
  assert!(observed.get());
  assert_eq!(shared.callback_depth.get(), 0);
  *target.borrow_mut() = None;
  shared.native.borrow_mut().ledger.retired = true;
}

#[test]
fn owned_failure_result_moves_cause_once_and_keeps_retirement_separate() -> Result<()> {
  let shared = shared();
  shared.fail(failed());
  let config = owned_config::Config::new(
    WebViewAttributes::default(),
    super::super::PlatformSpecificWebViewAttributes::default(),
    false,
    true,
  )?;
  let mut construction = WebViewConstruction {
    driver: Some(Box::new(construction::Driver {
      config,
      shared: shared.clone(),
    })),
  };
  assert_eq!(construction.progress()?, ConstructionProgress::Finished);
  let failure = construction
    .take_result()
    .ok_or_else(|| owned_config::invalid("missing failure result"))?
    .err()
    .ok_or_else(|| owned_config::invalid("unexpected successful result"))?;
  assert!(construction.take_result().is_none());
  let (cause, mut retirement) = failure.into_parts();
  assert!(matches!(cause, Error::WebView2Error(_)));
  assert!(retirement.checked_retire()?);
  Ok(())
}

#[test]
fn owned_zero_last_error_is_an_actionable_failure() {
  let cause = retirement::error_for_bool(0);
  match cause {
    Error::WebView2Error(webview2_com::Error::WindowsError(cause)) => {
      assert!(cause.code().is_err());
      assert_eq!(
        cause.code(),
        windows::core::HRESULT::from_win32(ERROR_GEN_FAILURE.0)
      );
    }
    _ => panic!("missing original Windows cause"),
  }
}
