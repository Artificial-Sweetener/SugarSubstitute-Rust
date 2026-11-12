// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Classify only actual writer-rendezvous contention; native errors remain owned and unchanged.
use super::*;
use std::{error::Error, fmt, fs::File};
#[derive(Debug)]
struct WriterBusy(io::Error);
impl fmt::Display for WriterBusy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Operational diagnostic writer is busy")
    }
}
impl Error for WriterBusy {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}
pub(super) fn acquire(
    protection: &protection::Protection,
    path: &Path,
    context: &JobContext,
) -> io::Result<File> {
    protection.exclusive_rendezvous(path, context).map_err(|cause| {
        if protection::is_busy(&cause) {
            io::Error::new(io::ErrorKind::WouldBlock, WriterBusy(cause))
        } else {
            cause
        }
    })
}
/// Classify a page another process holds open as contention: replacing it fails with access
/// denied or a sharing violation until that reader closes it, while the writer rendezvous already
/// proved the directory writable.
pub(super) fn page_held(cause: io::Error) -> io::Error {
    const ACCESS_DENIED: u32 = 5;
    const SHARING_VIOLATION: u32 = 32;
    let mut current: Option<&(dyn Error + 'static)> = Some(&cause);
    let mut held = false;
    for _ in 0..32 {
        let Some(link) = current else { break };
        held |=
            win32_code(link).is_some_and(|code| matches!(code, ACCESS_DENIED | SHARING_VIOLATION));
        current = link.source();
    }
    if held { io::Error::new(io::ErrorKind::WouldBlock, WriterBusy(cause)) } else { cause }
}
/// The original Win32 code one link carries, read through its owner's accessor.
fn win32_code(link: &(dyn Error + 'static)) -> Option<u32> {
    if let Some(claim) = link.downcast_ref::<crate::ClaimError>() {
        return claim.win32_code();
    }
    if let Some(native) =
        link.downcast_ref::<sugarsubstitute_execution::windows::NativeObservationError>()
    {
        return native.win32_code();
    }
    if let Some(io) = link.downcast_ref::<io::Error>() {
        return io.raw_os_error().and_then(|code| u32::try_from(code).ok());
    }
    let native = link.downcast_ref::<windows::core::Error>()?;
    let code = native.code().0 as u32;
    (code & 0xffff_0000 == 0x8007_0000).then_some(code & 0xffff)
}
pub(super) fn is_busy(cause: &io::Error) -> bool {
    let mut current: Option<&(dyn Error + 'static)> = Some(cause);
    for _ in 0..32 {
        let Some(cause) = current else { return false };
        if cause.is::<WriterBusy>()
            || cause
                .downcast_ref::<io::Error>()
                .and_then(io::Error::get_ref)
                .is_some_and(|inner| inner.is::<WriterBusy>())
        {
            return true;
        }
        current = cause.source();
    }
    false
}
