// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Name native selectors independently of their selected value and retire the original annotation.
use super::*;
use windows::Win32::{
    System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
    UI::{
        Accessibility::{CAccPropServices, IAccPropServices, PROPID_ACC_NAME},
        WindowsAndMessaging::{CHILDID_SELF, OBJID_CLIENT},
    },
};

/// Dropdown-list controls reject WM_SETTEXT. Their accessible name belongs to the OS annotation
/// service, while CB_SETCURSEL remains the sole selected-value operation.
pub(super) fn name(peer: &Peer, name: &str) -> Result<(), DesktopError> {
    let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    // SAFETY: The existing UI STA and original peer remain owned throughout this synchronous call.
    let service: IAccPropServices =
        unsafe { CoCreateInstance(&CAccPropServices, None, CLSCTX_INPROC_SERVER) }
            .map_err(|cause| DesktopError::new("native_selector_accessibility", cause))?;
    // Retain cleanup before publication, including any partially failed OS annotation.
    peer.named.set(true);
    // SAFETY: Windows copies this bounded label for the exact retained native child identity.
    unsafe {
        service.SetHwndPropStr(
            peer.hwnd,
            OBJID_CLIENT.0 as u32,
            CHILDID_SELF,
            PROPID_ACC_NAME,
            PCWSTR(name.as_ptr()),
        )
    }
    .map_err(|cause| DesktopError::new("native_selector_name", cause))
}
/// Remove only our one property before destroying the original selector; failed cleanup stays owned.
pub(super) fn clear(peer: &Peer) -> Result<(), DesktopError> {
    if !peer.named.get() {
        return Ok(());
    }
    // SAFETY: Checked native retirement runs on the same retained UI STA before window destruction.
    let service: IAccPropServices =
        unsafe { CoCreateInstance(&CAccPropServices, None, CLSCTX_INPROC_SERVER) }
            .map_err(|cause| DesktopError::new("native_selector_accessibility", cause))?;
    // SAFETY: Clear only this original child and the exact property this owner published.
    unsafe {
        service.ClearHwndProps(peer.hwnd, OBJID_CLIENT.0 as u32, CHILDID_SELF, &[PROPID_ACC_NAME])
    }
    .map_err(|cause| DesktopError::new("native_selector_name_release", cause))?;
    peer.named.set(false);
    Ok(())
}
