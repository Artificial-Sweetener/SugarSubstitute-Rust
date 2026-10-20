// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep native pointer/focus qualification on an owned desktop without switch rights.
use super::TestResult;
use windows::{
    Win32::System::{
        Com::CoCreateGuid,
        StationsAndDesktops::{
            CloseDesktop, CreateDesktopW, DESKTOP_CONTROL_FLAGS, DESKTOP_CREATEWINDOW,
            DESKTOP_READOBJECTS, DESKTOP_WRITEOBJECTS, GetThreadDesktop, HDESK, SetThreadDesktop,
        },
        Threading::GetCurrentThreadId,
    },
    core::PCWSTR,
};

/// This thread attaches before COM/windows exist and restores only after their retirement.
pub(super) struct Desktop {
    previous: HDESK,
    owned: Option<HDESK>,
    attached: bool,
}
impl Desktop {
    pub(super) fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // SAFETY: Passive thread identity and an OS-generated unique test resource name.
        let (previous, guid) =
            unsafe { (GetThreadDesktop(GetCurrentThreadId())?, CoCreateGuid()?) };
        let name: Vec<u16> =
            format!("SugarSubstitute-Console-{guid:?}").encode_utf16().chain(Some(0)).collect();
        // SAFETY: Create an inactive desktop without any ability to switch the user's desktop.
        let owned = unsafe {
            CreateDesktopW(
                PCWSTR(name.as_ptr()),
                None,
                None,
                DESKTOP_CONTROL_FLAGS(0),
                DESKTOP_CREATEWINDOW.0 | DESKTOP_READOBJECTS.0 | DESKTOP_WRITEOBJECTS.0,
                None,
            )
        }?;
        let mut result = Self { previous, owned: Some(owned), attached: false };
        // SAFETY: This fresh test thread has not initialized COM or created any windows.
        unsafe { SetThreadDesktop(owned) }?;
        result.attached = true;
        Ok(result)
    }
    /// Require the caller to retire its native windows and COM apartment before restoration.
    pub(super) fn close(&mut self) -> TestResult {
        if self.attached {
            // SAFETY: The fixture caller retired every native resource before this explicit close.
            unsafe { SetThreadDesktop(self.previous) }?;
            self.attached = false;
        }
        if let Some(owned) = self.owned {
            // SAFETY: No fixture thread or window remains attached to this exact owned desktop.
            unsafe { CloseDesktop(owned) }?;
            self.owned = None;
        }
        Ok(())
    }
}
impl Drop for Desktop {
    fn drop(&mut self) {
        if self.owned.is_some() {
            tracing::error!(
                operation = "console_input_desktop_retained",
                "Native test desktop was not retired"
            );
        }
    }
}
