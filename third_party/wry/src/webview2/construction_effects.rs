// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener

//! Native effects for the shared construction driver. No effect holds a state borrow.

use super::*;
use construction::{ConstructionProgress, Driver};
use owned_state::Stage;

pub(super) fn advance(driver: &mut Driver) -> Result<ConstructionProgress> {
  let shared = driver.shared.clone();
  let _call = shared.begin_call()?;
  let stage = shared.native.borrow().ledger.stage;
  match stage {
    Stage::Container => {
      let mut apartment = APTTYPE_CURRENT;
      let mut qualifier = APTTYPEQUALIFIER_NONE;
      unsafe {
        CoGetApartmentType(&mut apartment, &mut qualifier)?;
      }
      if apartment != APTTYPE_STA && apartment != APTTYPE_MAINSTA {
        return Err(windows::core::Error::from(RPC_E_CHANGED_MODE).into());
      }
      let attrs = driver
        .config
        .attrs
        .as_ref()
        .ok_or_else(|| owned_config::invalid("missing construction inputs"))?;
      let hwnd = InnerWebView::create_container_hwnd_raw(
        shared.parent.get(),
        attrs,
        driver.config.is_child,
      )?;
      {
        let mut state = shared.native.borrow_mut();
        state.hwnd = Some(hwnd);
        state.ledger.window = true;
      }
      if shared.native.borrow().ledger.retiring {
        return Ok(ConstructionProgress::Pending);
      }
      if driver.config.id.is_none() {
        driver.config.id = Some((hwnd.0 as isize).to_string());
      }
      retirement::observe_container(&shared, hwnd)?;
      if shared.native.borrow().ledger.retiring {
        return Ok(ConstructionProgress::Pending);
      }
      InnerWebView::raise_container(hwnd)?;
      shared.native.borrow_mut().ledger.stage = Stage::Environment;
    }
    Stage::Environment => {
      if let Some(env) = &driver.config.platform.environment {
        let mut state = shared.native.borrow_mut();
        state.env = Some(env.clone());
        state.ledger.stage = Stage::Controller;
      } else {
        let callback_owner = shared.clone();
        let next = if driver.config.owned {
          Stage::EnvironmentPrepared
        } else {
          Stage::Controller
        };
        let handler =
          CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(move |status, env| {
            callback_owner.on_callback(|state| {
              construction_adoption::adopt(
                &mut state.env,
                env,
                status,
                &mut state.ledger,
                &mut state.failure,
                next,
                false,
              );
            });
            Ok(())
          }));
        let attrs = driver
          .config
          .attrs
          .as_ref()
          .ok_or_else(|| owned_config::invalid("missing environment inputs"))?;
        InnerWebView::submit_environment(
          attrs,
          driver.config.data_directory.as_deref(),
          driver.config.platform.clone(),
          &handler,
          || shared.arm_creation(Stage::AwaitEnvironment),
        )?;
      }
    }
    Stage::Controller => {
      let (hwnd, env) = {
        let state = shared.native.borrow();
        (
          state
            .hwnd
            .ok_or_else(|| owned_config::invalid("missing container"))?,
          state
            .env
            .clone()
            .ok_or_else(|| owned_config::invalid("missing environment"))?,
        )
      };
      let attrs = driver
        .config
        .attrs
        .as_ref()
        .ok_or_else(|| owned_config::invalid("missing controller inputs"))?;
      let background = if attrs.transparent {
        Some((0, 0, 0, 0))
      } else {
        attrs.background_color
      };
      if driver.config.composition {
        construction_composition::submit(
          hwnd,
          &env,
          attrs.incognito,
          background,
          driver.config.platform.profile_name.as_deref(),
          shared.clone(),
        )?;
        return Ok(ConstructionProgress::Pending);
      }
      let callback_owner = shared.clone();
      let handler = CreateCoreWebView2ControllerCompletedHandler::create(Box::new(
        move |status, controller| {
          let actual = controller.clone();
          callback_owner.on_callback(|state| {
            construction_adoption::adopt(
              &mut state.controller,
              controller,
              status,
              &mut state.ledger,
              &mut state.failure,
              Stage::Initialize,
              true,
            );
          });
          let observer = callback_owner.native.borrow().private_profile.as_ref().map(PrivateProfileObserver::retained);
          if let (Some(observer), Some(actual)) = (observer, actual) {
            if let Err(cause) = observer.controller(&actual) { callback_owner.fail(cause); }
          }
          Ok(())
        },
      ));
      InnerWebView::submit_controller(
        hwnd,
        &env,
        attrs.incognito,
        background,
        driver.config.platform.profile_name.as_deref(),
        &handler,
        || shared.arm_creation(Stage::AwaitController),
      )?;
    }
    Stage::ResolveComposition => {
      let controller = construction_composition::resolve(&shared)?;
      if shared.native.borrow().ledger.retiring {
        return Ok(ConstructionProgress::Pending);
      }
      let observer = shared.native.borrow().private_profile.as_ref().map(PrivateProfileObserver::retained);
      if let Some(observer) = observer {
        observer.controller(&controller)?;
      }
      if shared.native.borrow().ledger.retiring || shared.native.borrow().failure.is_some() {
        return Ok(ConstructionProgress::Pending);
      }
      shared.native.borrow_mut().ledger.stage = Stage::Initialize;
    }
    Stage::Initialize => {
      super::construction_initialize::initialize(driver)?;
      let state = shared.native.borrow();
      if state.ledger.retiring || state.failure.is_some() {
        return Ok(ConstructionProgress::Pending);
      }
      drop(state);
      shared.native.borrow_mut().ledger.stage = Stage::Bounds;
    }
    Stage::Bounds => {
      let (controller, hwnd) = {
        let state = shared.native.borrow();
        (
          state
            .controller
            .clone()
            .ok_or_else(|| owned_config::invalid("missing bounds controller"))?,
          state
            .hwnd
            .ok_or_else(|| owned_config::invalid("missing bounds container"))?,
        )
      };
      if driver.config.is_child {
        InnerWebView::set_bounds_at(&controller, hwnd, driver.config.bounds)?;
      } else {
        let size = InnerWebView::parent_bounds(shared.parent.get())?;
        InnerWebView::set_bounds_native(&controller, hwnd, size, (0, 0).into())?;
      }
      shared.native.borrow_mut().ledger.stage = Stage::Finished;
    }
    Stage::AwaitEnvironment | Stage::AwaitController => return Ok(ConstructionProgress::Pending),
    Stage::EnvironmentPrepared => return Ok(ConstructionProgress::EnvironmentPrepared),
    Stage::Finished => return Ok(ConstructionProgress::Finished),
  }
  Ok(if shared.native.borrow().ledger.stage == Stage::Finished {
    ConstructionProgress::Finished
  } else {
    ConstructionProgress::Advanced
  })
}

pub(super) fn completed_inner(driver: &Driver) -> Result<InnerWebView> {
  let state = driver.shared.native.borrow();
  Ok(InnerWebView {
    id: driver.config.id.clone().unwrap_or_default(),
    parent: RefCell::new(driver.shared.parent.get()),
    hwnd: state
      .hwnd
      .ok_or_else(|| owned_config::invalid("missing completed container"))?,
    is_child: driver.config.is_child,
    controller: state
      .controller
      .clone()
      .ok_or_else(|| owned_config::invalid("missing completed controller"))?,
    webview: state
      .webview
      .clone()
      .ok_or_else(|| owned_config::invalid("missing completed webview"))?,
    env: state
      .env
      .clone()
      .ok_or_else(|| owned_config::invalid("missing completed environment"))?,
    drag_drop_controller: None,
    retained: Some(driver.shared.clone()),
    owned: driver.config.owned,
  })
}
