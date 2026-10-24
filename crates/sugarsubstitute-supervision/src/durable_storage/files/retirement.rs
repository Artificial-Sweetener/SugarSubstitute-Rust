// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe ordinary ancestor-pin closure using the caller's retained native custody charge.
use super::Directory;
use crate::{
    ClaimError,
    native_claims::files::{Budget, Handle},
};
use std::{io, os::windows::io::IntoRawHandle};
use windows::Win32::Foundation::HANDLE;
impl Directory {
    /// Check every retained parent close, retaining failed outputs' original accounting.
    /// This consumes no new admission and performs no path lookup or deletion.
    pub(crate) fn retire(self, budget: &Budget) -> io::Result<()> {
        let mut outcome = Ok(());
        for file in self._pins.into_iter().rev() {
            let mut handle = Handle::new(HANDLE(file.into_raw_handle()), budget.clone());
            outcome = ClaimError::finish(outcome, handle.close());
        }
        outcome.map_err(io::Error::other)
    }
}
