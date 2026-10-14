// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read one complete native snapshot without resize, retry or partial projection.
use super::*;
use std::{error::Error, fmt};
use windows::Win32::{
    Foundation::ERROR_SUCCESS,
    NetworkManagement::IpHelper::{GetExtendedTcpTable, TCP_TABLE_CLASS},
    Networking::WinSock::AF_INET,
};

/// Preserve the actual native code and its returned required/estimated byte count.
/// An insufficient buffer is a failed snapshot, never a missing listener or peer.
#[derive(Debug)]
pub struct TcpTableFailure {
    cause: std::io::Error,
    supplied: u32,
    required: u32,
}
impl TcpTableFailure {
    /// Bytes advertised to this exact failed native query.
    pub const fn supplied_bytes(&self) -> u32 {
        self.supplied
    }
    /// Actual native output, including a larger required estimate on error 122.
    pub const fn required_bytes(&self) -> u32 {
        self.required
    }
}
impl fmt::Display for TcpTableFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TCP snapshot failed: supplied {} bytes, native estimate {} bytes",
            self.supplied, self.required
        )
    }
}
impl Error for TcpTableFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}
pub(super) fn read<'a>(
    storage: &'a mut TcpOwnerWorkspace,
    context: &JobContext,
    class: TCP_TABLE_CLASS,
) -> Result<&'a [u32], RegistryError> {
    storage.check(context)?;
    let supplied = size_of_val(storage.words.as_slice()) as u32;
    let mut length = supplied;
    // SAFETY: Original admitted initialized u32 storage is aligned and owns every advertised byte.
    // The single call either fills a complete table or reports failure; no partial row is inspected.
    let code = unsafe {
        GetExtendedTcpTable(
            Some(storage.words.as_mut_ptr().cast()),
            &mut length,
            false,
            u32::from(AF_INET.0),
            class,
            0,
        )
    };
    let after = storage.check(context);
    complete(code, supplied, length, after)?;
    let count = payload_words(&storage.words, length as usize)?;
    Ok(&storage.words[..count])
}
pub(super) fn complete(
    code: u32,
    supplied: u32,
    required: u32,
    after: Result<(), RegistryError>,
) -> Result<(), RegistryError> {
    if code != ERROR_SUCCESS.0 {
        let mut error = RegistryError::new(
            "listener_tcp_table",
            TcpTableFailure {
                cause: std::io::Error::from_raw_os_error(code as i32),
                supplied,
                required,
            },
        );
        if let Err(after) = after {
            error.retain(after);
        }
        return Err(error);
    }
    after
}
