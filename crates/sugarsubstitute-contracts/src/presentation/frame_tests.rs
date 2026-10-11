// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Exercise real incremental validation and single-charge immutable frame storage.

use super::*;
use crate::execution::MemoryRelease;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

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

fn identity() -> Result<FrameIdentity, crate::execution::IdentityError> {
    Ok(FrameIdentity {
        surface: SurfaceId::new(7)?,
        state_generation: Generation::new(3)?,
        sequence: Generation::new(11)?,
    })
}

fn lease(bytes: u64, releases: &Arc<Releases>) -> MemoryLease {
    MemoryLease::new(bytes, Box::new(Release(releases.clone())))
}

#[test]
fn shape_bounds_cover_dimensions_scales_and_exact_tight_storage() -> Result<(), FrameError> {
    for (width, height) in [(0, 1), (1, 0), (4097, 1), (1, 4097), (u32::MAX, u32::MAX)] {
        assert_eq!(FrameShape::new(width, height, 1.0), Err(FrameError::InvalidDimensions));
    }
    for scale in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, -0.0, 0.0, 4.01] {
        assert_eq!(FrameShape::new(1, 1, scale), Err(FrameError::InvalidScale));
    }
    for scale in [f64::MIN_POSITIVE, 1.0, 1.25, 1.5, 1.75, 2.0, 4.0] {
        let shape = FrameShape::new(4096, 4096, scale)?;
        assert_eq!(shape.width(), 4096);
        assert_eq!(shape.height(), 4096);
        assert_eq!(shape.stride(), 16384);
        assert_eq!(shape.byte_len(), 67_108_864);
        assert_eq!(shape.scale(), scale);
        assert_eq!(shape.format(), FrameFormat::PremultipliedRgba8);
    }
    Ok(())
}

#[test]
fn rejects_length_mismatch_and_releases_its_owned_reservation() -> Result<(), Box<dyn Error>> {
    let shape = FrameShape::new(2, 1, 1.0)?;
    for actual in [7, 9] {
        let releases = Arc::new(Releases::default());
        let result = FrameValidator::new(shape, identity()?, vec![0; actual], lease(16, &releases));
        assert!(
            matches!(result, Err(FrameError::LengthMismatch { expected: 8, actual: observed }) if observed == actual)
        );
        assert_eq!(releases.calls.load(Ordering::SeqCst), 1);
        assert_eq!(releases.bytes.load(Ordering::SeqCst), 16);
    }
    Ok(())
}

#[test]
fn admission_covers_vector_capacity_even_when_pixel_length_is_small() -> Result<(), Box<dyn Error>>
{
    let shape = FrameShape::new(1, 1, 1.0)?;
    let mut pixels = Vec::with_capacity(64);
    pixels.extend_from_slice(&[0, 0, 0, 0]);
    let capacity = pixels.capacity() as u64;
    let releases = Arc::new(Releases::default());
    let result = FrameValidator::new(shape, identity()?, pixels, lease(4, &releases));
    assert!(
        matches!(result, Err(FrameError::InsufficientReservation { required_bytes, reserved_bytes: 4 }) if required_bytes == capacity)
    );
    assert_eq!(releases.calls.load(Ordering::SeqCst), 1);
    Ok(())
}

#[test]
fn distinct_pixel_allocations_cannot_reuse_a_retained_frames_shared_charge()
-> Result<(), Box<dyn Error>> {
    let shape = FrameShape::new(1, 1, 1.0)?;
    let releases = Arc::new(Releases::default());
    let reservation = lease(4, &releases);
    let mut validator = FrameValidator::new(shape, identity()?, vec![0; 4], reservation.clone())?;
    assert!(validator.validate_next(NonZeroUsize::new(1).ok_or("Nonzero budget")?)?);
    let frame = validator.finish()?;
    let retained = frame.clone();
    let expected =
        FrameError::AllocationBudget(MemoryClaimError { requested_bytes: 4, available_bytes: 0 });
    assert!(expected.source().is_some());
    let result = FrameValidator::new(shape, identity()?, vec![0; 4], reservation.clone());
    assert!(matches!(result, Err(error) if error == expected));
    assert_eq!(releases.calls.load(Ordering::SeqCst), 0);
    drop(frame);
    let result = FrameValidator::new(shape, identity()?, vec![0; 4], reservation.clone());
    assert!(matches!(result, Err(error) if error == expected));
    drop(retained);
    let replacement = FrameValidator::new(shape, identity()?, vec![0; 4], reservation.clone())?;
    drop(reservation);
    assert_eq!(releases.calls.load(Ordering::SeqCst), 0);
    drop(replacement);
    assert_eq!(releases.calls.load(Ordering::SeqCst), 1);
    assert_eq!(releases.bytes.load(Ordering::SeqCst), 4);
    Ok(())
}

#[test]
fn validation_budget_is_clamped_and_final_pixel_failure_cannot_be_bypassed()
-> Result<(), Box<dyn Error>> {
    let shape = FrameShape::new(257, 256, 1.0)?;
    let mut pixels = vec![0; shape.byte_len()];
    let final_pixel = shape.byte_len() / 4 - 1;
    pixels[final_pixel * 4 + 2] = 1;
    let releases = Arc::new(Releases::default());
    let mut validator =
        FrameValidator::new(shape, identity()?, pixels, lease(shape.byte_len() as u64, &releases))?;
    let unlimited = NonZeroUsize::new(usize::MAX).ok_or("Nonzero budget")?;
    assert!(!validator.validate_next(unlimited)?);
    let invalid = FrameError::InvalidPremultiplication { pixel: final_pixel };
    assert_eq!(validator.validate_next(unlimited), Err(invalid));
    assert_eq!(validator.validate_next(unlimited), Err(invalid));
    assert!(matches!(validator.finish(), Err(error) if error == invalid));
    assert_eq!(releases.calls.load(Ordering::SeqCst), 1);
    Ok(())
}

#[test]
fn every_color_channel_requires_alpha_and_finish_requires_all_pixels() -> Result<(), Box<dyn Error>>
{
    let shape = FrameShape::new(2, 1, 1.0)?;
    let budget = NonZeroUsize::new(1).ok_or("Nonzero budget")?;
    for channel in 0..3 {
        let mut pixels = vec![0; shape.byte_len()];
        pixels[4 + channel] = 8;
        pixels[7] = 7;
        let releases = Arc::new(Releases::default());
        let mut validator = FrameValidator::new(shape, identity()?, pixels, lease(8, &releases))?;
        assert!(!validator.validate_next(budget)?);
        assert_eq!(
            validator.validate_next(budget),
            Err(FrameError::InvalidPremultiplication { pixel: 1 })
        );
    }
    let releases = Arc::new(Releases::default());
    let mut validator = FrameValidator::new(shape, identity()?, vec![0; 8], lease(8, &releases))?;
    assert!(!validator.validate_next(budget)?);
    assert!(matches!(
        validator.finish(),
        Err(FrameError::ValidationIncomplete { validated_pixels: 1, total_pixels: 2 })
    ));
    let validator = FrameValidator::new(shape, identity()?, vec![0; 8], lease(8, &releases))?;
    assert!(matches!(
        validator.finish(),
        Err(FrameError::ValidationIncomplete { validated_pixels: 0, total_pixels: 2 })
    ));
    Ok(())
}

#[test]
fn prepared_clones_share_pixels_and_release_one_charge_only_after_last_drop()
-> Result<(), Box<dyn Error>> {
    let shape = FrameShape::new(2, 1, 1.25)?;
    let identity = identity()?;
    let pixels = vec![1, 2, 3, 4, 128, 0, 128, 128];
    let original_allocation = pixels.as_ptr();
    let releases = Arc::new(Releases::default());
    let mut validator = FrameValidator::new(shape, identity, pixels, lease(8, &releases))?;
    assert!(validator.validate_next(NonZeroUsize::new(2).ok_or("Nonzero budget")?)?);
    let frame = validator.finish()?;
    let retained = frame.clone();
    assert_eq!(frame.shape(), shape);
    assert_eq!(frame.identity(), identity);
    assert_eq!(frame.width(), 2);
    assert_eq!(frame.height(), 1);
    assert_eq!(frame.stride(), 8);
    assert_eq!(frame.scale(), 1.25);
    assert_eq!(frame.format(), FrameFormat::PremultipliedRgba8);
    assert_eq!(frame.reserved_bytes(), 8);
    assert_eq!(frame.pixels(), &[1, 2, 3, 4, 128, 0, 128, 128]);
    assert_eq!(frame.pixels().as_ptr(), original_allocation);
    assert_eq!(retained.pixels().as_ptr(), original_allocation);
    drop(frame);
    assert_eq!(releases.calls.load(Ordering::SeqCst), 0);
    assert_eq!(retained.pixels(), &[1, 2, 3, 4, 128, 0, 128, 128]);
    drop(retained);
    assert_eq!(releases.calls.load(Ordering::SeqCst), 1);
    assert_eq!(releases.bytes.load(Ordering::SeqCst), 8);
    Ok(())
}
