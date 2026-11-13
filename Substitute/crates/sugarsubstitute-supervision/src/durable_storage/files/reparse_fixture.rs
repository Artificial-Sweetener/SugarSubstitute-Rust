// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Create a real owned directory junction with synchronous file I/O and no process/window actor.
//! The test caller supplies owned temporary paths; this helper creates only an absent leaf.
use super::{BACKUP_SEMANTICS, Directory, OPEN_REPARSE_POINT, REPARSE_POINT, invalid, lexical};
use std::{
    fs::OpenOptions,
    io,
    os::windows::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawHandle,
    },
    path::Path,
};
use sugarsubstitute_contracts::execution::JobContext;
use windows::Win32::{Foundation::HANDLE, System::IO::DeviceIoControl};

/// Build the documented mount-point buffer and retain every path/handle through synchronous IOCTL.
/// See <https://learn.microsoft.com/windows-hardware/drivers/ddi/ntifs/ns-ntifs-_reparse_data_buffer>.
pub(crate) fn junction(link: &Path, target: &Path, context: &JobContext) -> io::Result<()> {
    super::super::check(context)?;
    lexical(link)?;
    let _parent =
        Directory::open(link.parent().ok_or_else(|| invalid("Junction parent absent"))?, false)?;
    let target = Directory::open(target, false)?.resolved()?;
    let display = target.path().as_os_str().encode_wide().collect::<Vec<_>>();
    let substitute = r"\??\".encode_utf16().chain(display.iter().copied()).collect::<Vec<_>>();
    let substitute_bytes = u16::try_from(substitute.len() * 2).map_err(io::Error::other)?;
    let display_bytes = u16::try_from(display.len() * 2).map_err(io::Error::other)?;
    let data_length = 8_usize + usize::from(substitute_bytes) + 2 + usize::from(display_bytes) + 2;
    if data_length + 8 > 16 * 1024 {
        return Err(invalid("Junction fixture buffer exceeds native bound"));
    }
    let mut buffer = Vec::new();
    buffer.try_reserve_exact(data_length + 8).map_err(io::Error::other)?;
    buffer.extend_from_slice(&0xa000_0003_u32.to_le_bytes()); // IO_REPARSE_TAG_MOUNT_POINT.
    buffer.extend_from_slice(&(data_length as u16).to_le_bytes());
    buffer.extend_from_slice(&0_u16.to_le_bytes());
    buffer.extend_from_slice(&0_u16.to_le_bytes());
    buffer.extend_from_slice(&substitute_bytes.to_le_bytes());
    buffer.extend_from_slice(&(substitute_bytes + 2).to_le_bytes());
    buffer.extend_from_slice(&display_bytes.to_le_bytes());
    for unit in substitute.into_iter().chain([0]).chain(display).chain([0]) {
        buffer.extend_from_slice(&unit.to_le_bytes());
    }
    super::super::check(context)?;
    std::fs::create_dir(link)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .custom_flags(OPEN_REPARSE_POINT | BACKUP_SEMANTICS)
        .open(link)?;
    let mut returned = 0;
    // SAFETY: File owns this synchronous directory handle. The complete bounded byte buffer,
    // returned DWORD and target/parent pins stay live; no output buffer or overlapped I/O exists.
    unsafe {
        DeviceIoControl(
            HANDLE(file.as_raw_handle()),
            0x0009_00a4, // FSCTL_SET_REPARSE_POINT from selected SDK.
            Some(buffer.as_ptr().cast()),
            buffer.len() as u32,
            None,
            0,
            Some(&mut returned),
            None,
        )
    }
    .map_err(io::Error::other)?;
    if file.metadata()?.file_attributes() & REPARSE_POINT == 0 {
        return Err(invalid("Fixture did not create a reparse point"));
    }
    super::super::check(context)
}
