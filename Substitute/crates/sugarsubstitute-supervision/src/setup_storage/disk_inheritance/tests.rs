// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own native disk-handle duplication and unique initial-entry adoption for installation processes.
use super::*;

#[test]
fn gui_standard_stream_absence_is_allowed_but_native_query_failure_is_preserved() -> io::Result<()>
{
    use windows::core::{Error, HRESULT};
    assert_eq!(optional_standard(Err(Error::from_hresult(HRESULT(0))))?, None);
    assert!(optional_standard(Err(Error::from_hresult(HRESULT(0x80070006_u32 as i32)))).is_err());
    Ok(())
}

/// The slot-held event slot takes only an inherited non-file object; a disk handle offered there
/// is refused before adoption, so its owner keeps it.
#[test]
fn the_event_slot_adopts_an_inherited_event_and_refuses_a_disk_handle() -> io::Result<()> {
    use windows::Win32::{Security::SECURITY_ATTRIBUTES, System::Threading::CreateEventW};
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: true.into(),
    };
    // SAFETY: a private unnamed manual-reset event whose handle is relinquished just below.
    let event =
        unsafe { CreateEventW(Some(&attributes), true, false, None) }.map_err(io::Error::other)?;
    // SAFETY: this test relinquished the only owner of the inherited event handle above.
    let adopted = unsafe { capture_event(event.0 as usize) }?;
    let mut flags = 0;
    // SAFETY: the adopted owner keeps the handle live for this observation.
    unsafe { GetHandleInformation(HANDLE(adopted.as_raw_handle()), &mut flags) }
        .map_err(io::Error::other)?;
    assert_eq!(flags & HANDLE_FLAG_INHERIT.0, 0);
    let file = tempfile::tempfile()?;
    // SAFETY: the test keeps sole ownership of this temporary file's handle throughout.
    unsafe {
        SetHandleInformation(
            HANDLE(file.as_raw_handle()),
            HANDLE_FLAG_INHERIT.0,
            HANDLE_FLAG_INHERIT,
        )
    }
    .map_err(io::Error::other)?;
    // SAFETY: a disk object is refused before adoption, so the test's owner keeps the handle.
    assert!(unsafe { capture_event(file.as_raw_handle() as usize) }.is_err());
    assert_eq!(file.metadata()?.len(), 0);
    Ok(())
}
