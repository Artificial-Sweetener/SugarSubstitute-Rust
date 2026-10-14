// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Supply a bounded thread-safe COM sink without filesystem operations or blocking locks.

use super::buffer::{Buffer, BufferFailure, IMAGE_BYTES, Image};
use crate::DesktopError;
use std::{
    ffi::c_void,
    sync::{
        Arc, Mutex,
        atomic::{AtomicI32, Ordering},
    },
};
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};
use windows::{
    Win32::{
        Foundation::{
            E_NOTIMPL, E_POINTER, S_FALSE, S_OK, STG_E_ACCESSDENIED, STG_E_INVALIDFUNCTION,
            STG_E_LOCKVIOLATION, STG_E_MEDIUMFULL,
        },
        System::Com::*,
    },
    core::{HRESULT, Ref, implement},
};

pub(super) const METADATA_BYTES: u64 = 8192;

/// COM callers share only bounded storage, not the UI-affine WebView interface.
pub(super) struct Shared {
    buffer: Mutex<Buffer>,
    failure: AtomicI32,
    metadata: Arc<MemoryClaim>,
}

impl Shared {
    /// Preclaim fixed COM/callback metadata and the exact image before native admission.
    pub(super) fn new(lease: &MemoryLease) -> Result<Arc<Self>, DesktopError> {
        let metadata = lease
            .claim(METADATA_BYTES)
            .map_err(|cause| DesktopError::new("claim_preview_callback", cause))?;
        let buffer = Buffer::new(lease)?;
        Ok(Arc::new(Self {
            buffer: Mutex::new(buffer),
            failure: AtomicI32::new(0),
            metadata: Arc::new(metadata),
        }))
    }

    /// Preserve the actual first HRESULT without allocating in native stream calls.
    pub(super) fn fail(&self, status: HRESULT) -> HRESULT {
        match self.failure.compare_exchange(0, status.0, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => status,
            Err(original) => HRESULT(original),
        }
    }

    /// Observe failure independently of the native capture completion HRESULT.
    pub(super) fn status(&self) -> HRESULT {
        HRESULT(self.failure.load(Ordering::Acquire))
    }

    /// Share the same fixed metadata charge with the immutable completed receipt.
    pub(super) fn metadata(&self) -> Arc<MemoryClaim> {
        self.metadata.clone()
    }

    fn access<T>(
        &self,
        effect: impl FnOnce(&mut Buffer) -> Result<T, BufferFailure>,
    ) -> std::result::Result<T, HRESULT> {
        self.status().ok().map_err(|_| self.status())?;
        let mut buffer = self.buffer.try_lock().map_err(|_| self.fail(STG_E_LOCKVIOLATION))?;
        effect(&mut buffer).map_err(|cause| {
            self.fail(match cause {
                BufferFailure::Full => STG_E_MEDIUMFULL,
                BufferFailure::InvalidPosition => STG_E_INVALIDFUNCTION,
                BufferFailure::Frozen | BufferFailure::MissingStorage => STG_E_ACCESSDENIED,
            })
        })
    }

    /// Freeze after the real completion; contention retains a failed qualification result.
    pub(super) fn freeze(&self) -> std::result::Result<(), HRESULT> {
        self.access(Buffer::freeze)
    }

    /// Extract only after actual callback return; the allocation's original claim moves with it.
    pub(super) fn take(&self) -> std::result::Result<Image, HRESULT> {
        self.access(Buffer::take)
    }
}

#[implement(IStream, IAgileObject)]
struct Sink {
    shared: Arc<Shared>,
}

impl IAgileObject_Impl for Sink_Impl {}

/// Return a COM reference to the same admitted sink; no unbounded system stream is used.
pub(super) fn interface(shared: &Arc<Shared>) -> IStream {
    Sink { shared: shared.clone() }.into()
}

impl ISequentialStream_Impl for Sink_Impl {
    fn Read(&self, pointer: *mut c_void, count: u32, actual: *mut u32) -> HRESULT {
        // SAFETY: COM supplies a writable optional scalar output throughout this call.
        unsafe { set_count(actual, 0) };
        if count as usize > IMAGE_BYTES || (count != 0 && pointer.is_null()) {
            return self.shared.fail(E_POINTER);
        }
        let output = if count == 0 {
            &mut []
        } else {
            // SAFETY: ISequentialStream caller owns count writable bytes; count was bounded
            // and a nonempty slice cannot be constructed from a null pointer.
            unsafe { std::slice::from_raw_parts_mut(pointer.cast::<u8>(), count as usize) }
        };
        match self.shared.access(|buffer| buffer.read(output)) {
            Ok(read) => {
                // SAFETY: The optional scalar belongs to this same native call.
                unsafe { set_count(actual, read as u32) };
                if read < count as usize { S_FALSE } else { S_OK }
            }
            Err(status) => status,
        }
    }

    fn Write(&self, pointer: *const c_void, count: u32, actual: *mut u32) -> HRESULT {
        // SAFETY: COM supplies a writable optional scalar output throughout this call.
        unsafe { set_count(actual, 0) };
        if count as usize > IMAGE_BYTES {
            return self.shared.fail(STG_E_MEDIUMFULL);
        }
        if count != 0 && pointer.is_null() {
            return self.shared.fail(E_POINTER);
        }
        let input = if count == 0 {
            &[]
        } else {
            // SAFETY: ISequentialStream caller owns count readable bytes; count was bounded
            // before constructing a slice, and the nonempty pointer is nonnull.
            unsafe { std::slice::from_raw_parts(pointer.cast::<u8>(), count as usize) }
        };
        match self.shared.access(|buffer| buffer.write(input)) {
            Ok(()) => {
                // SAFETY: The optional scalar belongs to this same native call.
                unsafe { set_count(actual, count) };
                S_OK
            }
            Err(status) => status,
        }
    }
}

impl IStream_Impl for Sink_Impl {
    fn Seek(
        &self,
        displacement: i64,
        origin: STREAM_SEEK,
        actual: *mut u64,
    ) -> windows::core::Result<()> {
        let position = self
            .shared
            .access(|buffer| buffer.seek(displacement, origin.0))
            .map_err(windows::core::Error::from)?;
        if !actual.is_null() {
            // SAFETY: COM caller provides this optional writable scalar for the current call.
            unsafe { actual.write(position) };
        }
        Ok(())
    }

    fn SetSize(&self, size: u64) -> windows::core::Result<()> {
        self.shared.access(|buffer| buffer.set_size(size)).map_err(windows::core::Error::from)
    }

    fn CopyTo(
        &self,
        _: Ref<IStream>,
        _: u64,
        read: *mut u64,
        written: *mut u64,
    ) -> windows::core::Result<()> {
        for pointer in [read, written] {
            if !pointer.is_null() {
                // SAFETY: COM provides each optional writable scalar throughout this call.
                unsafe { pointer.write(0) };
            }
        }
        Err(self.shared.fail(E_NOTIMPL).into())
    }

    fn Commit(&self, _: &STGC) -> windows::core::Result<()> {
        self.shared.status().ok()
    }

    fn Revert(&self) -> windows::core::Result<()> {
        Err(self.shared.fail(E_NOTIMPL).into())
    }

    fn LockRegion(&self, _: u64, _: u64, _: &LOCKTYPE) -> windows::core::Result<()> {
        Err(self.shared.fail(E_NOTIMPL).into())
    }

    fn UnlockRegion(&self, _: u64, _: u64, _: u32) -> windows::core::Result<()> {
        Err(self.shared.fail(E_NOTIMPL).into())
    }

    fn Stat(&self, pointer: *mut STATSTG, flags: &STATFLAG) -> windows::core::Result<()> {
        if pointer.is_null() {
            return Err(self.shared.fail(E_POINTER).into());
        }
        if *flags != STATFLAG_DEFAULT && *flags != STATFLAG_NONAME {
            return Err(self.shared.fail(STG_E_INVALIDFUNCTION).into());
        }
        let length =
            self.shared.access(|buffer| Ok(buffer.length())).map_err(windows::core::Error::from)?;
        // SAFETY: COM caller supplies a live writable STATSTG. No name allocation is made;
        // null pwcsName is the unnamed memory stream, and all metadata is fixed scalar data.
        unsafe {
            pointer.write(STATSTG {
                r#type: STGTY_STREAM.0 as u32,
                cbSize: length as u64,
                grfMode: STGM_READWRITE,
                ..STATSTG::default()
            })
        };
        Ok(())
    }

    fn Clone(&self) -> windows::core::Result<IStream> {
        Err(self.shared.fail(E_NOTIMPL).into())
    }
}

/// Set only a caller-owned optional count; no pointer survives the native call.
unsafe fn set_count(pointer: *mut u32, value: u32) {
    if !pointer.is_null() {
        // SAFETY: Caller guarantees the native optional output remains writable for this call.
        unsafe { pointer.write(value) };
    }
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod tests;
