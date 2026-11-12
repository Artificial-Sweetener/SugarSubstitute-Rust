// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject ambiguous entry modes before any native handle can be adopted.
use super::*;
use std::os::windows::io::FromRawHandle;
use windows::Win32::Foundation::{
    GetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT, SetHandleInformation,
};

#[test]
fn capture_narrows_the_unique_handle_before_any_descendant_can_inherit_it() -> io::Result<()> {
    use std::os::windows::io::IntoRawHandle;
    let file = tempfile::tempfile()?;
    let handle = HANDLE(file.as_raw_handle());
    // SAFETY: the test uniquely owns this file and changes only its inheritance bit.
    unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT.0, HANDLE_FLAG_INHERIT) }
        .map_err(io::Error::other)?;
    let raw = file.into_raw_handle();
    // SAFETY: the original owner was relinquished; this unowned duplicate cannot be closed
    // or reused by another test. The capture routine receives exclusive ownership once.
    let captured =
        unsafe { capture_launch_selection([PREFIX.into(), (raw as usize).to_string().into()]) }?
            .ok_or_else(|| io::Error::other("Captured selection absent"))?;
    let mut flags = 0;
    // SAFETY: captured retains the valid handle throughout this read-only query.
    unsafe { GetHandleInformation(HANDLE(captured.file.as_raw_handle()), &mut flags) }
        .map_err(io::Error::other)?;
    assert_eq!(flags & HANDLE_FLAG_INHERIT.0, 0);
    assert_eq!(captured.file.metadata()?.len(), 0);
    Ok(())
}

#[test]
fn protocol_requires_exact_version_canonical_handle_and_no_extra_arguments() {
    for input in [
        vec![PREFIX],
        vec![PREFIX, "0"],
        vec![PREFIX, "-1"],
        vec![PREFIX, "+4"],
        vec![PREFIX, "04"],
        vec![PREFIX, "4", "extra"],
        vec!["--installed-launch-v1", "4"],
        vec!["--installed-launch-v2", "4"],
        vec!["--installed-launch-v5", "4"],
    ] {
        assert!(parse(input.into_iter().map(OsString::from)).is_err());
    }
    assert!(parse([PREFIX.to_owned(), usize::MAX.to_string()].map(OsString::from)).is_err());
}

#[test]
fn ordinary_entry_roles_are_not_interpreted_as_inherited_launches() -> io::Result<()> {
    for input in [
        vec![],
        vec!["--production"],
        vec!["--internal-production-desktop"],
        vec!["--locale", "en"],
    ] {
        assert_eq!(parse(input.into_iter().map(OsString::from))?, None);
    }
    assert_eq!(parse([PREFIX, "128"].map(OsString::from))?, Some(128));
    Ok(())
}

#[test]
fn rejected_noninheritable_file_remains_owned_and_usable() -> io::Result<()> {
    use std::os::windows::io::IntoRawHandle;
    let raw = tempfile::tempfile()?.into_raw_handle();
    let arguments = [OsString::from(PREFIX), (raw as usize).to_string().into()];
    // SAFETY: the test relinquished its unique owner and no other code can close this handle.
    assert!(unsafe { capture_launch_selection(arguments) }.is_err());
    // SAFETY: rejection of a non-inheritable input occurs before adoption; the original
    // live handle still belongs to the test, which restores its unique owner here.
    let file = unsafe { File::from_raw_handle(raw) };
    assert_eq!(file.metadata()?.len(), 0);
    Ok(())
}
