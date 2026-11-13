// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the real COM sink, pointer bounds and nonblocking admission without WebView effects.

use super::*;
use std::error::Error;
use sugarsubstitute_contracts::execution::MemoryRelease;

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
struct Released;
impl MemoryRelease for Released {
    fn release(&self, _: u64) {}
}
fn shared() -> Result<Arc<Shared>> {
    let lease = MemoryLease::new(4 * 1024 * 1024, Box::new(Released));
    Ok(Shared::new(&lease)?)
}

#[test]
fn actual_com_write_seek_read_and_stat_retain_exact_stream_extent() -> Result {
    let shared = shared()?;
    assert_eq!(shared.metadata().bytes(), METADATA_BYTES);
    assert!(Arc::ptr_eq(&shared.metadata(), &shared.metadata()));
    let stream = interface(&shared);
    let input = b"actual bytes";
    let mut written = 99;
    let mut read = 99;
    let mut output = [0_u8; 16];
    let mut stat = STATSTG::default();
    // SAFETY: Each input/output allocation lives through the synchronous COM method;
    // native stream has no external effects and receives checked writable scalar outputs.
    unsafe {
        stream.Write(input.as_ptr().cast(), input.len() as u32, Some(&mut written)).ok()?;
        stream.Seek(0, STREAM_SEEK_SET, None)?;
        assert_eq!(
            stream.Read(output.as_mut_ptr().cast(), output.len() as u32, Some(&mut read)),
            S_FALSE
        );
        stream.Stat(&mut stat, STATFLAG_NONAME)?;
    }
    assert_eq!(written as usize, input.len());
    assert_eq!(read as usize, input.len());
    assert_eq!(&output[..read as usize], input);
    assert_eq!(stat.cbSize, input.len() as u64);
    assert!(stat.pwcsName.is_null());
    Ok(())
}

#[test]
fn overflowing_native_count_rejects_before_dereferencing_input() -> Result {
    let shared = shared()?;
    let stream = interface(&shared);
    let mut actual = 99;
    // SAFETY: The deliberately oversized count must be rejected before reading the null
    // input. The count output is a live writable scalar for this exact COM call.
    let result = unsafe { stream.Write(std::ptr::null(), u32::MAX, Some(&mut actual)) };
    assert_eq!(result, STG_E_MEDIUMFULL);
    assert_eq!(actual, 0);
    assert_eq!(shared.status(), STG_E_MEDIUMFULL);
    assert_eq!(shared.freeze(), Err(STG_E_MEDIUMFULL));
    Ok(())
}

#[test]
fn null_nonempty_input_is_sticky_even_after_zero_length_calls() -> Result {
    let shared = shared()?;
    let stream = interface(&shared);
    let mut actual = 99;
    // SAFETY: Null nonempty input is deliberately invalid and must be rejected before
    // a memory read; the optional output points to this live scalar.
    unsafe {
        assert_eq!(stream.Write(std::ptr::null(), 1, Some(&mut actual)), E_POINTER);
        assert_eq!(stream.Write(std::ptr::null(), 0, None), E_POINTER);
    }
    assert_eq!(actual, 0);
    assert_eq!(shared.status(), E_POINTER);
    Ok(())
}

#[test]
fn zero_length_null_input_is_valid_before_capture_completion() -> Result {
    let shared = shared()?;
    let stream = interface(&shared);
    // SAFETY: A zero-byte COM operation has no readable/writable buffer requirement;
    // the sink never constructs a slice from either null pointer.
    unsafe {
        assert_eq!(stream.Write(std::ptr::null(), 0, None), S_OK);
        assert_eq!(stream.Read(std::ptr::null_mut(), 0, None), S_OK);
    }
    assert_eq!(shared.status(), S_OK);
    Ok(())
}

#[test]
fn reentrant_stream_access_fails_without_waiting_or_mutating_storage() -> Result {
    let shared = shared()?;
    let stream = interface(&shared);
    let buffer = shared
        .buffer
        .try_lock()
        .map_err(|_| std::io::Error::other("Preview test lock was unavailable"))?;
    let input = [1_u8];
    let mut actual = 99;
    // SAFETY: Input and optional output are live. The already-held real owner lock
    // reproduces reentrant admission and must produce an immediate checked failure.
    let result = unsafe { stream.Write(input.as_ptr().cast(), 1, Some(&mut actual)) };
    assert_eq!(result, STG_E_LOCKVIOLATION);
    assert_eq!(actual, 0);
    assert_eq!(buffer.length(), 0);
    drop(buffer);
    assert_eq!(shared.freeze(), Err(STG_E_LOCKVIOLATION));
    Ok(())
}

#[test]
fn native_setsize_seek_and_unsupported_methods_cannot_escape_capacity() -> Result {
    let shared = shared()?;
    let stream = interface(&shared);
    // SAFETY: These methods contain only bounded scalar inputs, and no output pointer
    // is used. The real COM sink checks all extents before touching its allocation.
    unsafe {
        stream.SetSize(IMAGE_BYTES as u64)?;
        let result = stream.Seek(-1, STREAM_SEEK_SET, None);
        assert!(matches!(result, Err(cause) if cause.code() == STG_E_INVALIDFUNCTION));
        assert!(matches!(stream.Clone(), Err(cause) if cause.code() == STG_E_INVALIDFUNCTION));
    }
    assert_eq!(shared.status(), STG_E_INVALIDFUNCTION);
    Ok(())
}

#[test]
fn completed_sink_transfers_one_image_and_rejects_late_native_write() -> Result {
    let shared = shared()?;
    let stream = interface(&shared);
    let input = b"real encoded bytes";
    // SAFETY: Input lives through this exact bounded call, with no optional count output.
    unsafe { stream.Write(input.as_ptr().cast(), input.len() as u32, None).ok()? };
    shared.freeze().map_err(windows::core::Error::from)?;
    let image = shared.take().map_err(windows::core::Error::from)?;
    assert_eq!(image.bytes, input);
    assert_eq!(image.bytes.capacity(), IMAGE_BYTES);
    assert_eq!(image.claim.bytes(), IMAGE_BYTES as u64);
    // SAFETY: The input remains valid; frozen sink rejects it before modifying bytes.
    assert_eq!(unsafe { stream.Write(input.as_ptr().cast(), 1, None) }, STG_E_ACCESSDENIED);
    assert_eq!(image.bytes, input);
    Ok(())
}
