// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Construct bulk-origin responses and the read-only stream that serves one staged message.
//!
//! WebView2 may read a response stream from a background thread, so the stream is agile and
//! holds only immutable bytes, an atomic cursor and the bytes' memory claim, which drop when
//! WebView2 releases its last reference.

use super::*;
use crate::main_bulk::Download;
use std::sync::atomic::{AtomicU64, Ordering};
use sugarsubstitute_contracts::execution::MemoryClaim;
use windows::{
    Win32::{
        Foundation::{E_NOTIMPL, S_FALSE, S_OK, STG_E_ACCESSDENIED, STG_E_INVALIDFUNCTION},
        System::Com::*,
    },
    core::{Ref, implement},
};

/// Every response forbids caching and content sniffing.
const PRIVATE: &str = "Cache-Control: no-store\r\nX-Content-Type-Options: nosniff";

/// Serve one staged message to the page origin that requested it.
pub(super) fn message(
    environment: &ICoreWebView2Environment,
    origin: &str,
    download: Download,
) -> Result<ICoreWebView2WebResourceResponse, DesktopError> {
    let (bytes, claim) = download.into_parts();
    let stream: IStream = Body { bytes, position: AtomicU64::new(0), _claim: claim }.into();
    let headers = format!(
        "Content-Type: application/json; charset=utf-8\r\n{PRIVATE}\r\nAccess-Control-Allow-Origin: {origin}\r\nVary: Origin"
    );
    create(environment, Some(&stream), 200, w!("OK"), &headers)
}

/// Acknowledge a received upload to the page origin; the lane envelope names it next.
pub(super) fn accepted(
    environment: &ICoreWebView2Environment,
    origin: &str,
) -> Result<ICoreWebView2WebResourceResponse, DesktopError> {
    let headers = format!("{PRIVATE}\r\nAccess-Control-Allow-Origin: {origin}\r\nVary: Origin");
    create(environment, None, 204, w!("No Content"), &headers)
}

/// Answer every unmatched or refused request without CORS access or a body.
pub(super) fn missing(
    environment: &ICoreWebView2Environment,
) -> Result<ICoreWebView2WebResourceResponse, DesktopError> {
    create(environment, None, 404, w!("Not Found"), PRIVATE)
}

fn create(
    environment: &ICoreWebView2Environment,
    content: Option<&IStream>,
    status: i32,
    reason: windows::core::PCWSTR,
    headers: &str,
) -> Result<ICoreWebView2WebResourceResponse, DesktopError> {
    let headers = HSTRING::from(headers);
    // SAFETY: The environment, optional stream and both strings are live for this synchronous
    // UI-thread call; WebView2 takes its own stream reference.
    unsafe { environment.CreateWebResourceResponse(content, status, reason, &headers) }
        .map_err(|cause| DesktopError::new("create_bulk_response", cause))
}

/// One staged message served read-only; its claim is released with the stream.
#[implement(IStream, IAgileObject)]
struct Body {
    bytes: Vec<u8>,
    position: AtomicU64,
    _claim: MemoryClaim,
}

impl IAgileObject_Impl for Body_Impl {}

impl Body_Impl {
    /// Advance the cursor atomically and return the byte range now owned by this read.
    fn advance(&self, count: u64) -> (usize, usize) {
        let length = self.bytes.len() as u64;
        let step = |position: u64| Some(position.saturating_add(count).min(length).max(position));
        let start = self
            .position
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, step)
            .unwrap_or_else(|position| position);
        let end = start.saturating_add(count).min(length).max(start);
        (start.min(length) as usize, end.min(length) as usize)
    }
}

impl ISequentialStream_Impl for Body_Impl {
    fn Read(&self, pointer: *mut c_void, count: u32, actual: *mut u32) -> HRESULT {
        // SAFETY: COM supplies an optional writable count for the duration of this call.
        unsafe { set_count(actual, 0) };
        if count != 0 && pointer.is_null() {
            return E_POINTER;
        }
        let (start, end) = self.advance(u64::from(count));
        let read = end - start;
        if read != 0 {
            // SAFETY: The caller owns `count` writable bytes at `pointer`; `read <= count`,
            // and the source range lies within the immutable body.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    self.bytes[start..end].as_ptr(),
                    pointer.cast::<u8>(),
                    read,
                )
            };
        }
        // SAFETY: As above; `read` fits because it never exceeds `count`.
        unsafe { set_count(actual, read as u32) };
        if read == count as usize { S_OK } else { S_FALSE }
    }

    fn Write(&self, _: *const c_void, _: u32, actual: *mut u32) -> HRESULT {
        // SAFETY: COM supplies an optional writable count for the duration of this call.
        unsafe { set_count(actual, 0) };
        STG_E_ACCESSDENIED
    }
}

impl IStream_Impl for Body_Impl {
    fn Seek(
        &self,
        displacement: i64,
        origin: STREAM_SEEK,
        actual: *mut u64,
    ) -> windows::core::Result<()> {
        let base = match origin {
            STREAM_SEEK_SET => 0,
            STREAM_SEEK_CUR => self.position.load(Ordering::Acquire),
            STREAM_SEEK_END => self.bytes.len() as u64,
            _ => return Err(STG_E_INVALIDFUNCTION.into()),
        };
        let position = base.checked_add_signed(displacement).ok_or(STG_E_INVALIDFUNCTION)?;
        self.position.store(position, Ordering::Release);
        if !actual.is_null() {
            // SAFETY: COM supplies this optional writable scalar for the current call.
            unsafe { actual.write(position) };
        }
        Ok(())
    }

    fn SetSize(&self, _: u64) -> windows::core::Result<()> {
        Err(STG_E_ACCESSDENIED.into())
    }

    fn CopyTo(
        &self,
        destination: Ref<IStream>,
        count: u64,
        read: *mut u64,
        written: *mut u64,
    ) -> windows::core::Result<()> {
        let destination = destination.ok()?;
        let (start, end) = self.advance(count);
        let mut copied = 0_u64;
        for chunk in self.bytes[start..end].chunks(1024 * 1024) {
            let mut wrote = 0_u32;
            // SAFETY: The chunk is immutable owned storage at most 1 MiB long, so its length
            // fits `u32`; the count output is a live local for this synchronous call.
            unsafe {
                destination.Write(chunk.as_ptr().cast(), chunk.len() as u32, Some(&mut wrote))
            }
            .ok()?;
            copied += u64::from(wrote);
        }
        for (pointer, value) in [(read, (end - start) as u64), (written, copied)] {
            if !pointer.is_null() {
                // SAFETY: COM supplies each optional writable scalar for the current call.
                unsafe { pointer.write(value) };
            }
        }
        Ok(())
    }

    fn Commit(&self, _: &STGC) -> windows::core::Result<()> {
        Ok(())
    }

    fn Revert(&self) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn LockRegion(&self, _: u64, _: u64, _: &LOCKTYPE) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn UnlockRegion(&self, _: u64, _: u64, _: u32) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn Stat(&self, pointer: *mut STATSTG, flags: &STATFLAG) -> windows::core::Result<()> {
        if pointer.is_null() {
            return Err(E_POINTER.into());
        }
        if *flags != STATFLAG_DEFAULT && *flags != STATFLAG_NONAME {
            return Err(STG_E_INVALIDFUNCTION.into());
        }
        // SAFETY: COM supplies a live writable STATSTG; no name is allocated, so the null
        // `pwcsName` describes an unnamed memory stream.
        unsafe {
            pointer.write(STATSTG {
                r#type: STGTY_STREAM.0 as u32,
                cbSize: self.bytes.len() as u64,
                grfMode: STGM_READ,
                ..STATSTG::default()
            })
        };
        Ok(())
    }

    fn Clone(&self) -> windows::core::Result<IStream> {
        Err(E_NOTIMPL.into())
    }
}

/// Set only a caller-owned optional count; no pointer survives the native call.
///
/// # Safety
/// `pointer` must be null or writable for the duration of the current COM call.
unsafe fn set_count(pointer: *mut u32, value: u32) {
    if !pointer.is_null() {
        // SAFETY: Guaranteed by this function's contract.
        unsafe { pointer.write(value) };
    }
}

#[cfg(test)]
#[path = "response_tests.rs"]
mod tests;
