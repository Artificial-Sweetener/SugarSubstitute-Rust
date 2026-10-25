// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual selector hit geometry, native popup capacity and assigned typography.
use super::*;
use windows::Win32::{
    Foundation::RECT,
    UI::Controls::{COMBOBOXINFO, GetComboBoxInfo},
};

pub(super) fn qualify(peer: &Peer) -> TestResult {
    let mut info = COMBOBOXINFO { cbSize: size_of::<COMBOBOXINFO>() as u32, ..Default::default() };
    let mut closed = RECT::default();
    let mut list = RECT::default();
    // SAFETY: Inspect and expand only the original selector retained by this private-desktop fixture.
    let font = unsafe {
        GetComboBoxInfo(peer.hwnd, &mut info)?;
        GetWindowRect(peer.hwnd, &mut closed)?;
        SendMessageW(peer.hwnd, CB_SHOWDROPDOWN, Some(WPARAM(1)), None);
        GetClientRect(info.hwndList, &mut list)?;
        SendMessageW(peer.hwnd, WM_GETFONT, None, None)
    };
    // SAFETY: Read scalar row metrics and close the same original native popup before reporting failures.
    let row = unsafe {
        let height = SendMessageW(peer.hwnd, CB_GETITEMHEIGHT, Some(WPARAM(0)), None).0;
        SendMessageW(peer.hwnd, CB_SHOWDROPDOWN, Some(WPARAM(0)), None);
        height
    };
    let expected = peer.rect.bottom - peer.rect.top;
    if font.0 == 0
        || closed.bottom - closed.top != expected
        || row <= 0
        || list.bottom - list.top < row as i32 * 3
    {
        return Err(format!("Selector visual contract: assigned_font={}, closed_height={}, expected_height={expected}, popup_height={}, row_height={row}",font.0!=0,closed.bottom-closed.top,list.bottom-list.top).into());
    }
    Ok(())
}

/// Inspect pixels drawn by the actual OS popup, independently of the prepared closed field.
pub(super) fn qualify_palette(
    peer: &Peer,
    frame: &sugarsubstitute_contracts::presentation::PreparedFrame,
) -> TestResult {
    use crate::native_storage::Dib;
    use sugarsubstitute_contracts::presentation::FrameShape;
    use windows::Win32::Graphics::Gdi::GdiFlush;
    let popup = sugarsubstitute_ui::windows::select::popup(peer.hwnd)?;
    // SAFETY: Open only the original selector on this test's inactive desktop.
    unsafe { SendMessageW(peer.hwnd, CB_SHOWDROPDOWN, Some(WPARAM(1)), None) };
    let mut retained = None;
    let result = (|| -> TestResult {
        let mut bounds = RECT::default();
        // SAFETY: This popup is still owned by the fixture's original retained combo.
        unsafe { GetClientRect(popup, &mut bounds) }?;
        let dib = Dib::for_shape(
            &mut retained,
            frame,
            FrameShape::new(bounds.right as u32, bounds.bottom as u32, frame.scale())?,
        )?;
        // SAFETY: Ask the original list procedure to draw into this test's admitted DIB.
        unsafe {
            SendMessageW(
                popup,
                WM_PRINTCLIENT,
                Some(WPARAM(dib.dc().0 as usize)),
                Some(LPARAM((PRF_CLIENT | PRF_ERASEBKGND) as isize)),
            );
            GdiFlush().ok()?;
        }
        let pixels = dib.pixels_mut()?;
        let dark = pixels.chunks_exact(4).filter(|pixel| pixel[..3] == [41, 41, 41]).count();
        let light = pixels.chunks_exact(4).filter(|pixel| pixel[..3] == [255, 255, 255]).count();
        if dark < pixels.len() / 8 || light == 0 {
            return Err(format!("Native popup palette differs: dark={dark}, light={light}").into());
        }
        Ok(())
    })();
    sugarsubstitute_ui::windows::select::close(peer.hwnd)?;
    if let Some(dib) = &mut retained {
        dib.close()?;
    }
    result
}

/// Previewing a dropdown option must not start provisioning before the native selection commits.
pub(super) fn qualify_commit(peer: &Peer, events: &mut EventLoop<NativeAction>) -> TestResult {
    let _pending = drain_inputs(events);
    // SAFETY: Drive the fixture's original selector only on its private desktop and owning thread.
    let original = unsafe {
        let selected = SendMessageW(peer.hwnd, CB_GETCURSEL, None, None).0;
        SendMessageW(peer.hwnd, CB_SHOWDROPDOWN, Some(WPARAM(1)), None);
        SendMessageW(peer.hwnd, WM_KEYDOWN, Some(WPARAM(0x28)), Some(LPARAM(1)));
        selected
    };
    let preview = drain_inputs(events).0;
    // SAFETY: Escape delegates to this original popup's native cancellation behavior.
    let canceled = unsafe {
        SendMessageW(peer.hwnd, WM_KEYDOWN, Some(WPARAM(0x1b)), Some(LPARAM(1)));
        SendMessageW(peer.hwnd, CB_GETCURSEL, None, None).0
    };
    let cancel = drain_inputs(events).0;
    // SAFETY: A second popup gesture explicitly accepts the next real native option.
    unsafe {
        SendMessageW(peer.hwnd, CB_SHOWDROPDOWN, Some(WPARAM(1)), None);
        SendMessageW(peer.hwnd, WM_KEYDOWN, Some(WPARAM(0x28)), Some(LPARAM(1)));
        SendMessageW(peer.hwnd, WM_KEYDOWN, Some(WPARAM(0x0d)), Some(LPARAM(1)));
    }
    let committed = drain_inputs(events).0;
    if !preview.is_empty()
        || !cancel.is_empty()
        || canceled != original
        || committed != [peer.snapshot.id]
    {
        return Err(format!("Selector commit contract: preview={preview:?}, cancel={cancel:?}, before={original}, after_cancel={canceled}, commit={committed:?}").into());
    }
    Ok(())
}

/// Retained application acknowledgements must not reset an in-progress native preview.
pub(super) fn qualify_repaint(
    controls: &mut Controls,
    plan: &mut [Specification],
    events: &mut EventLoop<NativeAction>,
) -> TestResult {
    let hwnd = controls.creation.peers[3].value.hwnd;
    if let Role::Select { selected, .. } = &mut plan[3].role {
        *selected = Some(1);
    }
    // SAFETY: Start a second gesture before the earlier accepted application value is repainted.
    unsafe {
        SendMessageW(hwnd, CB_SHOWDROPDOWN, Some(WPARAM(1)), None);
        SendMessageW(hwnd, WM_KEYDOWN, Some(WPARAM(0x28)), Some(LPARAM(1)));
    }
    controls.update(plan)?;
    let preview = state(hwnd);
    // SAFETY: Cancel only this live original selector's preview.
    unsafe { SendMessageW(hwnd, WM_KEYDOWN, Some(WPARAM(0x1b)), Some(LPARAM(1))) };
    let canceled = state(hwnd);
    let echoed = drain_inputs(events).0;
    if preview != (2, true) || canceled != (1, false) || !echoed.is_empty() {
        return Err(format!("Selector repaint contract: preview={preview:?}, canceled={canceled:?}, echoed={echoed:?}").into());
    }
    // SAFETY: A genuine application replacement while previewing must close the old native gesture.
    unsafe {
        SendMessageW(hwnd, CB_SHOWDROPDOWN, Some(WPARAM(1)), None);
        SendMessageW(hwnd, WM_KEYDOWN, Some(WPARAM(0x28)), Some(LPARAM(1)));
    }
    if let Role::Select { selected, .. } = &mut plan[3].role {
        *selected = Some(0);
    }
    controls.update(plan)?;
    if state(hwnd) != (0, false) || !drain_inputs(events).0.is_empty() {
        return Err("Authoritative selection replacement retained or echoed a preview".into());
    }
    // SAFETY: Disable the original peer while its next popup gesture remains open.
    unsafe {
        SendMessageW(hwnd, CB_SHOWDROPDOWN, Some(WPARAM(1)), None);
        SendMessageW(hwnd, WM_KEYDOWN, Some(WPARAM(0x28)), Some(LPARAM(1)));
    }
    plan[3].control.enabled = false;
    controls.update(plan)?;
    controls.check()?;
    if state(hwnd) != (0, false) || !drain_inputs(events).0.is_empty() {
        return Err("Disabled selector retained or committed its preview".into());
    }
    Ok(())
}
/// Inspect only the still-owned native selection and popup visibility.
fn state(hwnd: HWND) -> (isize, bool) {
    // SAFETY: Caller retains this exact selector on the current private-desktop UI thread.
    unsafe {
        (
            SendMessageW(hwnd, CB_GETCURSEL, None, None).0,
            SendMessageW(hwnd, CB_GETDROPPEDSTATE, None, None).0 != 0,
        )
    }
}
