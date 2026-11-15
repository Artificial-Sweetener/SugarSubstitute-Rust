// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Query file users without asking Restart Manager to close or restart applications.
use crate::ClaimError;
use std::path::Path;
use windows::{
    Win32::System::RestartManager::*,
    core::{PCWSTR, PWSTR},
};

/// Retain a session until its checked native end succeeds, including after query failure.
#[derive(Default)]
pub(crate) struct Resources {
    session: Option<u32>,
}

/// One process Restart Manager reports using a file: its id and the name Windows shows for it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Holder {
    pub(crate) pid: u32,
    pub(crate) name: String,
}

impl std::fmt::Display for Holder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} (pid {})", self.name, self.pid)
    }
}
impl Resources {
    /// Return a bounded snapshot of processes using one canonical protected file.
    pub(super) fn users(&mut self, path: &Path) -> Result<Vec<RM_UNIQUE_PROCESS>, ClaimError> {
        Ok(self.processes(path)?.iter().map(|item| item.Process).collect())
    }

    /// The live processes holding `path` open, with the names Windows shows for them.
    pub(crate) fn holders(&mut self, path: &Path) -> Result<Vec<Holder>, ClaimError> {
        Ok(self
            .processes(path)?
            .iter()
            .map(|item| {
                let length = item
                    .strAppName
                    .iter()
                    .position(|unit| *unit == 0)
                    .unwrap_or(item.strAppName.len());
                Holder {
                    pid: item.Process.dwProcessId,
                    name: String::from_utf16_lossy(&item.strAppName[..length]),
                }
            })
            .collect())
    }

    fn processes(&mut self, path: &Path) -> Result<Vec<RM_PROCESS_INFO>, ClaimError> {
        self.close()?;
        let mut session = 0;
        let mut key = [0; 33];
        // SAFETY: Windows writes only into the initialized session/key outputs.
        unsafe { RmStartSession(&mut session, None, PWSTR(key.as_mut_ptr())) }
            .ok()
            .map_err(|cause| ClaimError::new("profile_conflict_session", cause))?;
        self.session = Some(session);
        let path = crate::durable_storage::protected::paths::wide(path)?;
        // SAFETY: The single terminated path remains owned throughout registration.
        unsafe { RmRegisterResources(session, Some(&[PCWSTR(path.as_ptr())]), None, None) }
            .ok()
            .map_err(|cause| ClaimError::new("profile_conflict_resources", cause))?;
        let mut items = vec![RM_PROCESS_INFO::default(); 64];
        let (mut needed, mut count, mut reasons) = (0, 64, 0);
        // SAFETY: The output capacity exactly matches the initialized array; no retry reallocates it.
        unsafe {
            RmGetList(session, &mut needed, &mut count, Some(items.as_mut_ptr()), &mut reasons)
        }
        .ok()
        .map_err(|cause| ClaimError::new("profile_conflict_users", cause))?;
        if count > 64 || needed > 64 {
            return Err(ClaimError::invalid("profile_conflict_user_bound"));
        }
        items.truncate(count as usize);
        self.close()?;
        Ok(items)
    }
    pub(crate) fn close(&mut self) -> Result<(), ClaimError> {
        if let Some(session) = self.session {
            // SAFETY: This owner alone ends the retained Restart Manager session.
            unsafe { RmEndSession(session) }
                .ok()
                .map_err(|cause| ClaimError::new("profile_conflict_session_close", cause))?;
            self.session = None;
        }
        Ok(())
    }
}
