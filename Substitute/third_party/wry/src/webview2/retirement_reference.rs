// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener

//! Installed references are recorded before reentrant native installation.

use super::*;
use owned_state::Shared;

#[derive(Clone, Copy)]
pub(super) enum Reference {
  Container,
  Parent,
}

pub(super) fn install(
  shared: &Rc<Shared>,
  kind: Reference,
  effect: impl FnOnce(usize) -> Result<()>,
) -> Result<()> {
  let raw = Rc::into_raw(shared.clone()) as usize;
  {
    let mut state = shared.native.borrow_mut();
    let slot = match kind {
      Reference::Container => &mut state.container_ref,
      Reference::Parent => &mut state.parent_ref,
    };
    if slot.is_some() {
      unsafe {
        drop(Rc::from_raw(raw as *const Shared));
      }
      return Err(owned_config::invalid(
        "duplicate native subclass installation",
      ));
    }
    *slot = Some(raw);
    if matches!(kind, Reference::Parent) {
      state.ledger.parent_subclass = true;
    }
  }
  if let Err(cause) = effect(raw) {
    let rollback = {
      let mut state = shared.native.borrow_mut();
      let slot = match kind {
        Reference::Container => &mut state.container_ref,
        Reference::Parent => &mut state.parent_ref,
      };
      let rollback = if *slot == Some(raw) {
        slot.take()
      } else {
        None
      };
      if matches!(kind, Reference::Parent) {
        state.ledger.parent_subclass = false;
      }
      rollback
    };
    if let Some(raw) = rollback {
      unsafe {
        drop(Rc::from_raw(raw as *const Shared));
      }
    }
    return Err(cause);
  }
  Ok(())
}

// Called only after DefSubclassProc returned or an actual successful removal/destruction.
pub(super) fn destroyed(shared: &Rc<Shared>, kind: Reference) {
  let installed = {
    let mut state = shared.native.borrow_mut();
    state.ledger.retiring = true;
    shared.close_requested.set(true);
    match kind {
      Reference::Container => {
        state.ledger.window = false;
        state.ledger.dispatcher = false;
        state.hwnd = None;
        state.container_ref.take()
      }
      Reference::Parent => {
        state.ledger.parent_subclass = false;
        state.parent_ref.take()
      }
    }
  };
  if let Some(raw) = installed {
    unsafe {
      drop(Rc::from_raw(raw as *const Shared));
    }
  }
  (shared.wake)();
}
