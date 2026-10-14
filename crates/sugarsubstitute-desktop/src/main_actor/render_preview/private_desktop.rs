// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject capture on the active desktop while retaining every owned observation handle.

use crate::DesktopError;
use windows::Win32::{
    Foundation::HANDLE,
    System::{StationsAndDesktops::*, Threading::GetCurrentThreadId},
};

#[derive(Default)]
pub(super) struct PrivateDesktop {
    input: Option<HDESK>,
    admitted: bool,
}

impl PrivateDesktop {
    /// Observe the creating thread and current input desktop without switching or modifying either.
    /// Adopt the only owned handle before interpreting its names or any failed output.
    pub(super) fn observe(&mut self) -> Result<(), DesktopError> {
        if self.input.is_some() || self.admitted {
            return Err(DesktopError::message(
                "observe_preview_desktop",
                "desktop already observed",
            ));
        }
        // SAFETY: Read-only desktop access creates one owned handle. No inheritance,
        // input routing, desktop activation or default-window operation is requested.
        let input =
            unsafe { OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_READOBJECTS) }
                .map_err(|cause| DesktopError::new("open_preview_input_desktop", cause))?;
        self.input = Some(input);
        let observed = (|| {
            // SAFETY: These getters borrow the calling process/thread's live station/desktop;
            // their handles are not owned here and must never be closed by this adapter.
            let station = unsafe { GetProcessWindowStation() }
                .map_err(|cause| DesktopError::new("observe_preview_station", cause))?;
            // SAFETY: The current thread identifier is queried synchronously; no thread handle.
            let current = unsafe { GetThreadDesktop(GetCurrentThreadId()) }
                .map_err(|cause| DesktopError::new("observe_preview_thread_desktop", cause))?;
            let station_name = name(HANDLE(station.0))?;
            let current_name = name(HANDLE(current.0))?;
            let input_name = name(HANDLE(input.0))?;
            if !admits(station_name.units(), current_name.units(), input_name.units()) {
                return Err(DesktopError::message(
                    "observe_preview_desktop",
                    "qualification desktop is active or unregistered",
                ));
            }
            Ok(())
        })();
        let closed = self.retire();
        match (observed, closed) {
            (Ok(()), Ok(())) => {
                self.admitted = true;
                Ok(())
            }
            (Err(cause), Ok(())) | (Ok(()), Err(cause)) => Err(cause),
            (Err(original), Err(close)) => {
                tracing::error!(operation="close_preview_input_desktop", cause=%close,
                    "Read-only desktop observation handle remains owned");
                Err(original)
            }
        }
    }

    /// Require both positive private-desktop observation and actual handle closure.
    pub(super) fn admitted(&self) -> bool {
        self.admitted && self.input.is_none()
    }

    /// Recheck current input state without replacing an unresolved handle or renewing a cutoff.
    pub(super) fn recheck(&mut self) -> Result<(), DesktopError> {
        if self.input.is_some() {
            return Err(DesktopError::message("observe_preview_desktop", "prior handle remains"));
        }
        self.admitted = false;
        self.observe()
    }

    /// Keep the exact input handle on close failure; neither Drop nor a deadline settles it.
    pub(super) fn retire(&mut self) -> Result<(), DesktopError> {
        if let Some(handle) = self.input {
            // SAFETY: This exact owned read-only handle has not been successfully closed;
            // borrowed process/thread station/desktop handles are never stored in this field.
            unsafe { CloseDesktop(handle) }
                .map_err(|cause| DesktopError::new("close_preview_input_desktop", cause))?;
            self.input = None;
        }
        Ok(())
    }
}

impl Drop for PrivateDesktop {
    fn drop(&mut self) {
        if self.input.is_some() {
            tracing::error!(
                operation = "preview_desktop_unresolved_drop",
                "Retaining unresolved read-only desktop handle until process termination"
            );
        }
    }
}

struct Name {
    units: [u16; 128],
    length: usize,
}
impl Name {
    fn units(&self) -> &[u16] {
        &self.units[..self.length]
    }
}

/// Bound the native output to fixed stack storage and validate the reported terminator.
fn name(handle: HANDLE) -> Result<Name, DesktopError> {
    let mut result = Name { units: [0; 128], length: 0 };
    let mut needed = 0;
    // SAFETY: The borrowed/owned handle remains live and Windows receives this exact
    // writable stack allocation's byte length plus one valid scalar output.
    unsafe {
        GetUserObjectInformationW(
            handle,
            UOI_NAME,
            Some(result.units.as_mut_ptr().cast()),
            size_of_val(&result.units) as u32,
            Some(&mut needed),
        )
    }
    .map_err(|cause| DesktopError::new("observe_preview_desktop_name", cause))?;
    if needed < 2 || needed as usize > size_of_val(&result.units) || needed % 2 != 0 {
        return Err(DesktopError::message("observe_preview_desktop_name", "native name bound"));
    }
    let units = needed as usize / 2;
    if result.units[units - 1] != 0 || result.units[..units - 1].contains(&0) {
        return Err(DesktopError::message("observe_preview_desktop_name", "native name shape"));
    }
    result.length = units - 1;
    Ok(result)
}

/// Accept only the native launcher's exact private namespace on WinSta0, currently inactive.
fn admits(station: &[u16], current: &[u16], input: &[u16]) -> bool {
    let expected = "WinSta0".encode_utf16();
    let prefix = "SugarSubstitute-K1-".encode_utf16();
    let prefix_length = prefix.clone().count();
    station.iter().copied().eq(expected)
        && current != input
        && current.len() == prefix_length + 32
        && current[..prefix_length].iter().copied().eq(prefix)
        && current[prefix_length..].iter().all(|unit| {
            (u16::from(b'0')..=u16::from(b'9')).contains(unit)
                || (u16::from(b'a')..=u16::from(b'f')).contains(unit)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_exact_inactive_launcher_desktop_is_eligible() {
        let station: Vec<_> = "WinSta0".encode_utf16().collect();
        let private: Vec<_> =
            "SugarSubstitute-K1-0123456789abcdef0123456789abcdef".encode_utf16().collect();
        let input: Vec<_> = "Default".encode_utf16().collect();
        assert!(admits(&station, &private, &input));
        assert!(!admits(&station, &private, &private));
        assert!(!admits(&station, &input, &private));
    }

    #[test]
    fn foreign_namespace_station_and_malformed_private_names_are_rejected() {
        let station: Vec<_> = "WinSta0".encode_utf16().collect();
        let input: Vec<_> = "Default".encode_utf16().collect();
        for name in [
            "SugarSubstitute-K1",
            "SugarSubstitute-K1-0123",
            "SugarSubstitute-K1-gggggggggggggggggggggggggggggggg",
            "OtherApp-K1-0123456789abcdef0123456789abcdef",
        ] {
            let current: Vec<_> = name.encode_utf16().collect();
            assert!(!admits(&station, &current, &input));
        }
        let private: Vec<_> =
            "SugarSubstitute-K1-0123456789abcdef0123456789abcdef".encode_utf16().collect();
        let foreign: Vec<_> = "ForeignStation".encode_utf16().collect();
        assert!(!admits(&foreign, &private, &input));
    }
}
