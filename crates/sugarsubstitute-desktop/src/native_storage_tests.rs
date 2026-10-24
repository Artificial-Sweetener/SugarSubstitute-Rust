// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise copied-storage charge ownership and actual hidden Windows GDI lifetime.

use super::*;
use std::{
    error::Error,
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::{
    execution::{Generation, MemoryLease, MemoryRelease, SurfaceId},
    presentation::{FrameIdentity, FrameShape, FrameValidator},
};

#[derive(Default)]
struct Releases {
    calls: AtomicUsize,
    bytes: AtomicU64,
}
struct Release(Arc<Releases>);
impl MemoryRelease for Release {
    fn release(&self, bytes: u64) {
        self.0.calls.fetch_add(1, Ordering::SeqCst);
        self.0.bytes.fetch_add(bytes, Ordering::SeqCst);
    }
}

fn frame(
    width: u32,
    height: u32,
    reserved: u64,
) -> Result<(PreparedFrame, Arc<Releases>), Box<dyn Error>> {
    frame_at(width, height, reserved, 1)
}

fn frame_at(
    width: u32,
    height: u32,
    reserved: u64,
    sequence: u64,
) -> Result<(PreparedFrame, Arc<Releases>), Box<dyn Error>> {
    let shape = FrameShape::new(width, height, 1.0)?;
    let identity = FrameIdentity {
        surface: SurfaceId::new(1)?,
        state_generation: Generation::new(1)?,
        sequence: Generation::new(sequence)?,
    };
    let releases = Arc::new(Releases::default());
    // This isolated fixture models already-admitted memory; production takes only PreparedFrame.
    let lease = MemoryLease::new(reserved, Box::new(Release(releases.clone())));
    let mut pixels = allocate_bytes(shape.byte_len())?;
    for rgba in pixels.chunks_exact_mut(4) {
        rgba.copy_from_slice(&[20, 40, 60, 255]);
    }
    let mut validator = FrameValidator::new(shape, identity, pixels, lease)?;
    assert!(validator.validate_next(NonZeroUsize::new(65536).ok_or("Nonzero budget")?)?);
    Ok((validator.finish()?, releases))
}

/// Copy actual native-endian ARGB bytes while charging the complete vector capacity.
#[test]
fn native_bytes_copy_capacity_and_claim_are_retained_until_final_storage_drop()
-> Result<(), Box<dyn Error>> {
    let (source, released) = frame(2, 2, 32)?;
    let mut bytes = NativeBytes::new(&source)?;
    assert_eq!(bytes.bytes.capacity(), 16);
    assert_eq!(bytes._claim.bytes(), 16);
    assert_eq!(&bytes.as_mut()[..4], &0xff14283cu32.to_ne_bytes());
    assert!(source.claim_storage(1).is_err());
    let copy = source.clone();
    drop(source);
    drop(copy);
    assert_eq!(released.calls.load(Ordering::SeqCst), 0);
    assert_eq!(bytes.as_mut().len(), 16);
    drop(bytes);
    assert_eq!(released.calls.load(Ordering::SeqCst), 1);
    assert_eq!(released.bytes.load(Ordering::SeqCst), 32);
    Ok(())
}

/// Exhaustion rejects before copying and released allocation capacity becomes reusable.
#[test]
fn native_bytes_reject_duplicate_storage_and_reuse_only_released_capacity()
-> Result<(), Box<dyn Error>> {
    let (source, released) = frame(2, 2, 32)?;
    let first = NativeBytes::new(&source)?;
    let error = NativeBytes::new(&source).err().ok_or("Duplicate native copy was admitted")?;
    assert!(error.to_string().starts_with("claim_native_bytes:"));
    assert!(error.source().is_some());
    drop(first);
    let replacement = NativeBytes::new(&source)?;
    assert_eq!(replacement.bytes.capacity(), 16);
    drop(source);
    assert_eq!(released.calls.load(Ordering::SeqCst), 0);
    drop(replacement);
    assert_eq!(released.calls.load(Ordering::SeqCst), 1);
    Ok(())
}

/// Preserve the original allocator error rather than panicking on an impossible capacity.
#[test]
fn native_bytes_allocator_capacity_overflow_is_a_typed_failure() -> Result<(), Box<dyn Error>> {
    let error = allocate_bytes(usize::MAX).err().ok_or("Impossible allocation succeeded")?;
    assert!(error.to_string().starts_with("allocate_native_bytes:"));
    assert!(
        error
            .source()
            .and_then(|cause| cause.downcast_ref::<std::collections::TryReserveError>())
            .is_some()
    );
    Ok(())
}

#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::{BITMAP, GetObjectW};

#[cfg(target_os = "windows")]
fn bitmap_live(dib: &Dib) -> bool {
    let mut bitmap = BITMAP::default();
    // SAFETY: The queried bitmap is live; the output has its documented size.
    unsafe {
        GetObjectW(
            dib.bitmap.into(),
            std::mem::size_of::<BITMAP>() as i32,
            Some((&raw mut bitmap).cast()),
        ) > 0
    }
}

/// Query actual GDI destruction and admission release without showing a native window.
#[cfg(target_os = "windows")]
#[test]
fn native_dib_drop_releases_bitmap_after_restoring_selection() -> Result<(), Box<dyn Error>> {
    let (source, released) = frame(2, 2, 32)?;
    let dib = Dib::new(&source, source.shape())?;
    let handle = dib.bitmap.into();
    let mut bitmap = BITMAP::default();
    // SAFETY: The owned bitmap is live and the output buffer has its documented size.
    let before = unsafe {
        GetObjectW(handle, std::mem::size_of::<BITMAP>() as i32, Some((&raw mut bitmap).cast()))
    };
    assert!(before > 0);
    assert_eq!(bitmap.bmWidth, 2);
    assert_eq!(bitmap.bmHeight, 2);
    assert_eq!(dib.claim.as_ref().ok_or("Lost native charge")?.bytes(), 16);
    assert!(source.claim_storage(1).is_err());
    drop(source);
    assert_eq!(released.calls.load(Ordering::SeqCst), 0);
    drop(dib);
    // SAFETY: Invalid GDI handle queries return zero without dereferencing native storage.
    let after = unsafe {
        GetObjectW(handle, std::mem::size_of::<BITMAP>() as i32, Some((&raw mut bitmap).cast()))
    };
    assert_eq!(after, 0);
    assert_eq!(released.calls.load(Ordering::SeqCst), 1);
    assert_eq!(released.bytes.load(Ordering::SeqCst), 32);
    Ok(())
}

/// Same-size publication reuses the DIB and keeps the original reservation after source replacement.
#[cfg(target_os = "windows")]
#[test]
fn native_dib_reuse_does_not_recharge_replacement_source() -> Result<(), Box<dyn Error>> {
    let (original, first_release) = frame(2, 2, 32)?;
    let mut retained = None;
    let first_handle = Dib::for_frame(&mut retained, &original)?.bitmap;
    drop(original);
    assert_eq!(first_release.calls.load(Ordering::SeqCst), 0);
    let (replacement, second_release) = frame(2, 2, 32)?;
    assert_eq!(Dib::for_frame(&mut retained, &replacement)?.bitmap, first_handle);
    let available = replacement.claim_storage(16)?;
    drop(available);
    drop(replacement);
    assert_eq!(second_release.calls.load(Ordering::SeqCst), 1);
    assert_eq!(first_release.calls.load(Ordering::SeqCst), 0);
    drop(retained);
    assert_eq!(first_release.calls.load(Ordering::SeqCst), 1);
    Ok(())
}

/// Resize creates a fully charged replacement while old storage remains live; rejection preserves old.
#[cfg(target_os = "windows")]
#[test]
fn native_dib_resize_rejection_preserves_old_allocation_and_success_releases_it()
-> Result<(), Box<dyn Error>> {
    let (original, first_release) = frame(2, 2, 32)?;
    let mut retained = None;
    let old_handle = Dib::for_frame(&mut retained, &original)?.bitmap;
    drop(original);
    let (insufficient, failed_release) = frame(3, 2, 24)?;
    let error = Dib::for_frame(&mut retained, &insufficient)
        .err()
        .ok_or("Uncharged resize was admitted")?;
    assert!(error.to_string().starts_with("claim_dib_storage:"));
    let old = retained.as_ref().ok_or("Failed resize discarded original DIB")?;
    assert_eq!(old.bitmap, old_handle);
    assert!(bitmap_live(old));
    assert_eq!(first_release.calls.load(Ordering::SeqCst), 0);
    drop(insufficient);
    assert_eq!(failed_release.calls.load(Ordering::SeqCst), 1);
    let (replacement, next_release) = frame(3, 2, 48)?;
    let next = Dib::for_frame(&mut retained, &replacement)?;
    assert_ne!(next.bitmap, old_handle);
    assert_eq!(next.pixels_mut()?.len(), 24);
    assert!(bitmap_live(next));
    assert_eq!(first_release.calls.load(Ordering::SeqCst), 1);
    drop(replacement);
    assert_eq!(next_release.calls.load(Ordering::SeqCst), 0);
    drop(retained);
    assert_eq!(next_release.calls.load(Ordering::SeqCst), 1);
    assert_eq!(next_release.bytes.load(Ordering::SeqCst), 48);
    Ok(())
}

/// Explicit release returns native copy capacity once and later Drop cannot delete it twice.
#[cfg(target_os = "windows")]
#[test]
fn native_dib_explicit_close_is_idempotent_and_returns_claim_only_after_release()
-> Result<(), Box<dyn Error>> {
    let (source, released) = frame(2, 2, 32)?;
    let mut dib = Dib::new(&source, source.shape())?;
    dib.close()?;
    assert!(dib.bitmap.is_invalid());
    assert!(dib.dc.is_invalid());
    assert!(dib.pixels.is_null());
    assert!(dib.claim.is_none());
    let borrow = dib.pixels_mut().err().ok_or("Closed DIB allowed pixel borrowing")?;
    assert!(borrow.to_string().starts_with("borrow_dib_pixels:"));
    let reusable = source.claim_storage(16)?;
    dib.close()?;
    drop(dib);
    assert_eq!(released.calls.load(Ordering::SeqCst), 0);
    drop(reusable);
    drop(source);
    assert_eq!(released.calls.load(Ordering::SeqCst), 1);
    Ok(())
}

/// A defined invalid GDI selection returns a typed failure and recovered storage returns its claim.
#[cfg(target_os = "windows")]
#[test]
fn native_dib_failed_selection_recovery_preserves_cause_and_deletes_storage()
-> Result<(), Box<dyn Error>> {
    let (source, released) = frame(2, 2, 32)?;
    let mut dib = Dib::new(&source, source.shape())?;
    let bitmap = dib.bitmap.into();
    // Only this fixture's owned DC is affected; SelectObject rejects an invalid opaque object.
    dib.previous = HGDIOBJ(std::ptr::dangling_mut::<std::ffi::c_void>());
    let error = dib.close().err().ok_or("Invalid GDI selection did not report a failure")?;
    assert!(error.to_string().starts_with("restore_dib_selection:"));
    assert!(error.source().is_some());
    assert!(dib.dc.is_invalid());
    assert!(dib.bitmap.is_invalid());
    assert!(dib.claim.is_none());
    let mut metadata = BITMAP::default();
    // SAFETY: Invalid native handle queries return zero; no memory is dereferenced.
    let bitmap_after = unsafe {
        GetObjectW(bitmap, std::mem::size_of::<BITMAP>() as i32, Some((&raw mut metadata).cast()))
    };
    assert_eq!(bitmap_after, 0);
    let reusable = source.claim_storage(16)?;
    drop(reusable);
    dib.close()?;
    drop(dib);
    drop(source);
    assert_eq!(released.calls.load(Ordering::SeqCst), 1);
    Ok(())
}

/// Retained exposure reuses real copied storage within a source-plus-one-copy reservation.
#[test]
fn copied_image_retained_exposure_reuses_pointer_and_original_claim_at_exact_two_n()
-> Result<(), Box<dyn Error>> {
    let (source, released) = frame(2, 2, 32)?;
    let mut retained = CopiedImage::default();
    retained.prepare(&source, NativeBytes::new)?;
    let pointer = retained.image().ok_or("Missing copied image")?.bytes.as_ptr();
    let source_clone = source.clone();
    retained.prepare(&source_clone, NativeBytes::new)?;
    assert_eq!(retained.image().ok_or("Retained exposure lost image")?.bytes.as_ptr(), pointer);
    assert!(source.claim_storage(1).is_err());
    drop(source);
    drop(source_clone);
    assert_eq!(released.calls.load(Ordering::SeqCst), 0);
    drop(retained);
    assert_eq!(released.calls.load(Ordering::SeqCst), 1);
    assert_eq!(released.bytes.load(Ordering::SeqCst), 32);
    Ok(())
}

/// A new identity replaces its image only on successful copying, preserving the old on rejection.
#[test]
fn copied_image_new_identity_replaces_and_failed_copy_preserves_old_source()
-> Result<(), Box<dyn Error>> {
    let (first, first_release) = frame_at(2, 2, 32, 1)?;
    let mut retained = CopiedImage::default();
    retained.prepare(&first, NativeBytes::new)?;
    let pointer = retained.image().ok_or("Missing first image")?.bytes.as_ptr();
    drop(first);
    let (rejected, rejected_release) = frame_at(2, 2, 16, 2)?;
    assert!(retained.prepare(&rejected, NativeBytes::new).is_err());
    assert_eq!(retained.image().ok_or("Failed copy lost original image")?.bytes.as_ptr(), pointer);
    assert_eq!(
        retained.image.as_ref().ok_or("Failed copy lost original identity")?.0.sequence,
        Generation::new(1)?
    );
    assert_eq!(first_release.calls.load(Ordering::SeqCst), 0);
    drop(rejected);
    assert_eq!(rejected_release.calls.load(Ordering::SeqCst), 1);
    let (next, next_release) = frame_at(2, 2, 32, 2)?;
    retained.prepare(&next, NativeBytes::new)?;
    assert_ne!(retained.image().ok_or("New copy missing")?.bytes.as_ptr(), pointer);
    assert_eq!(retained.image.as_ref().ok_or("New identity missing")?.0, next.identity());
    assert_eq!(first_release.calls.load(Ordering::SeqCst), 1);
    drop(next);
    assert_eq!(next_release.calls.load(Ordering::SeqCst), 0);
    drop(retained);
    assert_eq!(next_release.calls.load(Ordering::SeqCst), 1);
    Ok(())
}

/// Region admission spends only physical viewport pixels and rejects oversized geometry atomically.
#[cfg(target_os = "windows")]
#[test]
fn native_dib_viewport_uses_exact_credit_and_rejects_outside_source() -> Result<(), Box<dyn Error>>
{
    let (source, released) = frame(8, 8, 272)?;
    let mut retained = None;
    let shape = FrameShape::new(2, 2, 1.0)?;
    let dib = Dib::for_shape(&mut retained, &source, shape)?;
    let handle = dib.bitmap;
    assert_eq!((dib.width(), dib.height(), dib.pixels_mut()?.len()), (2, 2, 16));
    assert!(source.claim_storage(1).is_err());
    assert!(Dib::for_shape(&mut retained, &source, FrameShape::new(9, 1, 1.0)?).is_err());
    let dib = retained.as_mut().ok_or("original viewport lost")?;
    assert_eq!(dib.bitmap, handle);
    assert!(bitmap_live(dib));
    dib.close()?;
    drop(retained);
    drop(source);
    assert_eq!(released.calls.load(Ordering::SeqCst), 1);
    assert_eq!(released.bytes.load(Ordering::SeqCst), 272);
    Ok(())
}
