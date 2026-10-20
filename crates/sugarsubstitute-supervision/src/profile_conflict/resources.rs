// SPDX-License-Identifier: GPL-3.0-or-later
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
pub(super) struct Resources {
    session: Option<u32>,
}
impl Resources {
    /// Return a bounded snapshot of processes using one canonical protected file.
    pub(super) fn users(&mut self, path: &Path) -> Result<Vec<RM_UNIQUE_PROCESS>, ClaimError> {
        self.close()?;
        let mut session = 0;
        let mut key = [0; 33];
        // SAFETY: Windows writes only into the initialized session/key outputs.
        unsafe { RmStartSession(&mut session, None, PWSTR(key.as_mut_ptr())) }
            .ok()
            .map_err(|cause| ClaimError::new("profile_conflict_session", cause))?;
        self.session = Some(session);
        let path = crate::protected_storage::paths::wide(path)?;
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
        let users = items[..count as usize].iter().map(|item| item.Process).collect();
        self.close()?;
        Ok(users)
    }
    pub(super) fn close(&mut self) -> Result<(), ClaimError> {
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
