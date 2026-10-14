// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Move original settled-family failure custody without cloning its cleanup vector or releasing its table pin.
use super::*;
use std::{error::Error, fmt};
/// Original failure and unchanged registry allocation, retained independently of record removal.
/// This is passive custody; it grants no new work, process settlement or receiving disposition.
pub struct SettledRegistryFailure {
    process: ProcessKey,
    error: RegistryError,
    _original: SharedOwner,
}
impl SettledRegistryFailure {
    /// Exact actual family whose original failure moved after genuine settlement.
    pub const fn process(&self) -> ProcessKey {
        self.process
    }
    /// Borrow the same original first cause and bounded cleanup causes without copying them.
    pub const fn cause(&self) -> &RegistryError {
        &self.error
    }
}
impl fmt::Debug for SettledRegistryFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl fmt::Display for SettledRegistryFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Registry retained its original settled-family failure")
    }
}
impl Error for SettledRegistryFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}
impl WindowsProcessRegistry {
    /// Move the actual original error only after checked native/work settlement.
    /// The record remains registered and failed; all other observations still require transfer.
    /// The returned owner retains the same table pin after later explicit record retirement.
    /// # Errors
    /// Rejects foreign/stale keys, unsettled families and repeated failure transfer.
    pub fn take_settled_failure(
        &mut self,
        key: ProcessKey,
    ) -> Result<Option<SettledRegistryFailure>, RegistryAdmissionError> {
        let index = self.index(key)?;
        let record = self.records[index].as_mut().ok_or(RegistryAdmissionError::Stale)?;
        if !record.settled {
            return Err(RegistryAdmissionError::Busy);
        }
        if record.failure_transferred {
            return Err(RegistryAdmissionError::Closed);
        }
        let Some(error) = record.error.take() else { return Ok(None) };
        record.failure_transferred = true;
        record.unread[3] = false;
        Ok(Some(SettledRegistryFailure { process: key, error, _original: record.shared.clone() }))
    }
}
