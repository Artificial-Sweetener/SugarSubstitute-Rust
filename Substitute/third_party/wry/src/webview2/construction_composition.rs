// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener

//! Adopt windowless controller outputs before querying their base controller interface.

use super::*;
use owned_state::{Ledger, Shared, Stage};

/// Dispatch one composition-controller creation under the original owned transaction.
pub(super) fn submit(
  hwnd: HWND,
  env: &ICoreWebView2Environment,
  incognito: bool,
  background: Option<(u8, u8, u8, u8)>,
  profile_name: Option<&str>,
  shared: Rc<Shared>,
) -> Result<()> {
  enum Prepared {
    Modern(ICoreWebView2Environment10, ICoreWebView2ControllerOptions),
    Legacy(ICoreWebView2Environment3),
  }
  let callback_owner = shared.clone();
  let handler = CreateCoreWebView2CompositionControllerCompletedHandler::create(Box::new(
    move |status, composition| {
      callback_owner.on_callback(|state| {
        adopt(
          &mut state.composition,
          composition,
          status,
          &mut state.ledger,
          &mut state.failure,
        );
      });
      Ok(())
    },
  ));
  construction_admission::submit(
    || {
      if let Some(modern) =
        construction_admission::optional(env.cast::<ICoreWebView2Environment10>())?
      {
        let options =
          InnerWebView::controller_options(&modern, incognito, background, profile_name)?;
        Ok(Prepared::Modern(modern, options))
      } else {
        if incognito || profile_name.is_some() {
          return Err(owned_config::invalid(
            "composition controller profile options unavailable",
          ));
        }
        Ok(Prepared::Legacy(env.cast::<ICoreWebView2Environment3>()?))
      }
    },
    || shared.arm_creation(Stage::AwaitController),
    |prepared| {
      // SAFETY: The retained STA, parent HWND and callback owner outlive native completion.
      unsafe {
        match prepared {
          Prepared::Modern(environment, options) => environment
            .CreateCoreWebView2CompositionControllerWithOptions(hwnd, &options, &handler)?,
          Prepared::Legacy(environment) => {
            environment.CreateCoreWebView2CompositionController(hwnd, &handler)?
          }
        }
      }
      Ok(())
    },
  )
}

/// Retain even a failed native output; retirement must resolve and close it before release.
pub(super) fn adopt<T>(
  slot: &mut Option<T>,
  actual: Option<T>,
  status: windows::core::Result<()>,
  ledger: &mut Ledger,
  failure: &mut Option<Error>,
) {
  construction_adoption::adopt(
    slot,
    actual,
    status,
    ledger,
    failure,
    Stage::ResolveComposition,
    false,
  );
  if slot.is_some() {
    ledger.composition_pending = true;
    ledger.composition_detach_pending = true;
  }
}

/// Query the retained output outside every callback and state borrow.
/// Failure leaves the original output and its unresolved cleanup obligation retained.
pub(super) fn resolve(shared: &Rc<Shared>) -> Result<ICoreWebView2Controller> {
  let composition = {
    let state = shared.native.borrow();
    if !state.ledger.composition_pending || state.controller.is_some() {
      return Err(owned_config::invalid(
        "composition controller resolution is not pending",
      ));
    }
    state
      .composition
      .clone()
      .ok_or_else(|| owned_config::invalid("retained composition controller missing"))?
  };
  let controller = composition.cast::<ICoreWebView2Controller>()?;
  {
    let mut state = shared.native.borrow_mut();
    if !state.ledger.composition_pending || state.controller.is_some() {
      return Err(owned_config::invalid(
        "composition controller resolution changed",
      ));
    }
    state.controller = Some(controller.clone());
    state.ledger.controller = true;
    state.ledger.composition_pending = false;
  }
  Ok(controller)
}
