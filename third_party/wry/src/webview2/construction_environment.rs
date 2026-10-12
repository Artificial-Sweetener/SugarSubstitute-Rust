// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener

//! Observe bounded native folder data before trusted controller authorization.

use super::*;
use owned_state::Stage;

/// Exact observed UTF-16 folder data; it grants no profile, filesystem or process authority.
/// The caller preclaims the bounded known copy and holds that admission through consumption.
/// Opaque runtime/COM getter allocation is independently qualified, not covered by this Vec.
#[must_use]
pub struct UserDataFolder {
  units: Vec<u16>,
}
impl UserDataFolder {
  /// Maximum native code units including the NUL terminator.
  pub const MAX_UNITS: usize = 32767;
  /// Borrow the exact non-NUL UTF-16 data without lossy conversion.
  pub fn as_utf16(&self) -> &[u16] {
    &self.units
  }
  /// Transfer the copy into its caller's admitted observation owner.
  pub fn into_utf16(self) -> Vec<u16> {
    self.units
  }
}

struct NativeString(PWSTR);
impl Drop for NativeString {
  fn drop(&mut self) {
    unsafe {
      CoTaskMemFree(Some(self.0.as_ptr().cast()));
    }
  }
}

pub(super) fn consume_query<T>(
  native: T,
  status: windows::core::Result<()>,
  copy: impl FnOnce(&T) -> Result<UserDataFolder>,
) -> Result<UserDataFolder> {
  status?;
  copy(&native)
}

pub(super) fn copy_folder(mut read: impl FnMut(usize) -> Result<u16>) -> Result<UserDataFolder> {
  let mut length = None;
  for index in 0..UserDataFolder::MAX_UNITS {
    if read(index)? == 0 {
      length = Some(index);
      break;
    }
  }
  let length = length.filter(|&length| length != 0).ok_or_else(|| {
    owned_config::invalid("native user-data folder exceeds its bound or is empty")
  })?;
  let mut units = Vec::new();
  units
    .try_reserve_exact(length)
    .map_err(|cause| Error::Io(std::io::Error::other(cause)))?;
  if units.capacity() > UserDataFolder::MAX_UNITS - 1 {
    return Err(owned_config::invalid(
      "native folder copy capacity exceeded",
    ));
  }
  for index in 0..length {
    units.push(read(index)?);
  }
  Ok(UserDataFolder { units })
}

fn query(env: &ICoreWebView2Environment) -> Result<UserDataFolder> {
  let env = env.cast::<ICoreWebView2Environment7>()?;
  let mut actual = PWSTR::null();
  let status = unsafe { env.UserDataFolder(&mut actual) };
  let captured = NativeString(actual);
  consume_query(captured, status, |native| {
    if native.0.is_null() {
      return Err(windows::core::Error::from(E_POINTER).into());
    }
    // The COM getter owns a valid, NUL-terminated CoTaskMem UTF-16 string.
    // Scan only the declared maximum before allocating an authored copy.
    copy_folder(|index| Ok(unsafe { native.0.as_ptr().add(index).read() }))
  })
}

impl WebViewConstruction {
  /// Query one bounded effective folder on the owning UI thread at EnvironmentPrepared.
  ///
  /// Preclaim at least the maximum UTF-16 copy plus metadata before this call.
  /// The opaque native getter allocation is separate. Returns `None` when a native error
  /// was retained for Finished/take_result, or cancellation discarded the result.
  /// Errors returned directly are misuse; a successful query is data, not profile proof.
  pub fn user_data_folder(&mut self) -> Result<Option<UserDataFolder>> {
    let driver = self
      .driver
      .as_ref()
      .ok_or_else(|| owned_config::invalid("construction result already transferred"))?;
    let shared = driver.shared.clone();
    let _call = shared.begin_call()?;
    let env = {
      let state = shared.native.borrow();
      if !driver.config.owned
        || state.ledger.stage != Stage::EnvironmentPrepared
        || state.ledger.retiring
        || state.failure.is_some()
        || state.folder_queried
      {
        return Err(owned_config::invalid(
          "effective folder query is not admitted",
        ));
      }
      state
        .env
        .clone()
        .ok_or_else(|| owned_config::invalid("missing prepared environment"))?
    };
    match query(&env) {
      Ok(facts) => {
        let mut state = shared.native.borrow_mut();
        if state.ledger.retiring || state.failure.is_some() {
          return Ok(None);
        }
        state.folder_queried = true;
        Ok(Some(facts))
      }
      Err(cause) => {
        shared.fail(cause);
        Ok(None)
      }
    }
  }

  /// Authorize controller admission once after the trusted desktop validates this environment.
  ///
  /// The native caller must match its current application identity/generation and retain
  /// actual off-UI profile-file identity/live-lease proof. Observed folder data, decoded
  /// frontend input, or a lexical path comparison cannot authorize this operation.
  /// Rejects callbacks, in-flight calls, cancellation, failure and out-of-order/duplicate use.
  pub fn authorize_environment(&mut self) -> Result<()> {
    let driver = self
      .driver
      .as_ref()
      .ok_or_else(|| owned_config::invalid("construction result already transferred"))?;
    let shared = driver.shared.clone();
    let _call = shared.begin_call()?;
    let mut state = shared.native.borrow_mut();
    if !driver.config.owned
      || state.ledger.stage != Stage::EnvironmentPrepared
      || state.ledger.retiring
      || state.failure.is_some()
      || !state.folder_queried
    {
      return Err(owned_config::invalid(
        "environment authorization is not admitted",
      ));
    }
    state.ledger.stage = Stage::Controller;
    Ok(())
  }
}
