// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener

//! A native completion transfers its actual output before interpreting HRESULT.

use super::*;
use owned_state::{Ledger, Stage};

pub(super) fn adopt<T>(
  slot: &mut Option<T>,
  actual: Option<T>,
  status: windows::core::Result<()>,
  ledger: &mut Ledger,
  failure: &mut Option<Error>,
  next: Stage,
  controller: bool,
) {
  // WebView2 issues one completion per accepted async creation call.
  // A second output cannot safely be released as if it were the original transaction.
  if ledger.pending != 1 || slot.is_some() {
    std::mem::forget(actual);
    ledger.pending = 1;
    if failure.is_none() {
      *failure = Some(owned_config::invalid(
        "duplicate native construction completion",
      ));
    }
    ledger.stage = Stage::Finished;
    return;
  }
  *slot = actual;
  if controller {
    ledger.controller = slot.is_some();
  }
  ledger.pending = 0;
  let cause = status.err().map(Error::from).or_else(|| {
    slot
      .is_none()
      .then(|| windows::core::Error::from(E_POINTER).into())
  });
  if let Some(cause) = cause {
    if failure.is_none() {
      *failure = Some(cause);
    }
    ledger.stage = Stage::Finished;
  } else if !ledger.retiring && failure.is_none() {
    ledger.stage = next;
  }
}
