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
        if cause
            .get_ref()
            .and_then(|cause| cause.downcast_ref::<windows::core::Error>())
            .is_some_and(|cause| cause.code() == windows::core::HRESULT::from_win32(32))
        {
            io::Error::new(io::ErrorKind::WouldBlock, WriterBusy(cause))
        } else {
            cause
        }
    })
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
