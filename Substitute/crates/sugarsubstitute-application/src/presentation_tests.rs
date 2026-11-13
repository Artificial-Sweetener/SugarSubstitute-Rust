// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Verify publication against real validated and lease-retaining prepared frames.

use super::*;
use std::{
    error::Error,
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::{
    execution::{MemoryLease, MemoryRelease},
    presentation::{FrameIdentity, FrameValidator},
};

struct Release(Arc<AtomicUsize>);
impl MemoryRelease for Release {
    fn release(&self, _: u64) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn shape() -> Result<FrameShape, Box<dyn Error>> {
    Ok(FrameShape::new(2, 1, 1.0)?)
}

fn prepared(
    shape: FrameShape,
    surface: u64,
    state: u64,
    sequence: u64,
    released: &Arc<AtomicUsize>,
) -> Result<PreparedFrame, Box<dyn Error>> {
    let identity = FrameIdentity {
        surface: SurfaceId::new(surface)?,
        state_generation: Generation::new(state)?,
        sequence: Generation::new(sequence)?,
    };
    let pixels = vec![0; shape.byte_len()];
    let lease = MemoryLease::new(pixels.capacity() as u64, Box::new(Release(released.clone())));
    let mut validator = FrameValidator::new(shape, identity, pixels, lease)?;
    let budget = NonZeroUsize::new(65536).ok_or("Nonzero validation budget")?;
    if !validator.validate_next(budget)? {
        return Err("Publication fixture exceeds one validation step".into());
    }
    Ok(validator.finish()?)
}

fn gate(
    shape: FrameShape,
    generation: u64,
    visible: bool,
) -> Result<FramePublicationGate, IdentityError> {
    Ok(FramePublicationGate::new(SurfaceId::new(7)?, shape, Generation::new(generation)?, visible))
}

#[test]
fn rejects_foreign_stale_future_and_shape_mismatches_without_consuming_sequence()
-> Result<(), Box<dyn Error>> {
    let shape = shape()?;
    let releases = Arc::new(AtomicUsize::new(0));
    let mut gate = gate(shape, 3, true)?;
    for (frame, rejection) in [
        (prepared(shape, 8, 3, 50, &releases)?, FrameRejection::ForeignSurface),
        (prepared(shape, 7, 2, 50, &releases)?, FrameRejection::WrongState),
        (prepared(shape, 7, 4, 50, &releases)?, FrameRejection::WrongState),
        (
            prepared(FrameShape::new(3, 1, 1.0)?, 7, 3, 50, &releases)?,
            FrameRejection::ShapeMismatch,
        ),
        (
            prepared(FrameShape::new(2, 1, 1.25)?, 7, 3, 50, &releases)?,
            FrameRejection::ShapeMismatch,
        ),
    ] {
        assert_eq!(gate.accept(&frame), FramePublicationDecision::Rejected(rejection));
        assert_eq!(gate.last_accepted_sequence(), None);
    }
    let current = prepared(shape, 7, 3, 1, &releases)?;
    assert_eq!(gate.accept(&current), FramePublicationDecision::Accepted);
    assert_eq!(gate.last_accepted_sequence(), Some(Generation::new(1)?));
    Ok(())
}

#[test]
fn in_flight_animation_can_publish_while_newer_samples_are_pending() -> Result<(), Box<dyn Error>> {
    let shape = shape()?;
    let releases = Arc::new(AtomicUsize::new(0));
    let mut gate = gate(shape, 3, true)?;
    let first = prepared(shape, 7, 3, 1, &releases)?;
    let in_flight = prepared(shape, 7, 3, 2, &releases)?;
    let newer_pending = prepared(shape, 7, 3, 4, &releases)?;
    assert_eq!(gate.accept(&first), FramePublicationDecision::Accepted);
    assert_eq!(gate.accept(&in_flight), FramePublicationDecision::Accepted);
    assert_eq!(gate.last_accepted_sequence(), Some(Generation::new(2)?));
    assert_eq!(gate.accept(&newer_pending), FramePublicationDecision::Accepted);
    for older in [&first, &in_flight, &newer_pending] {
        assert_eq!(
            gate.accept(older),
            FramePublicationDecision::Rejected(FrameRejection::OutOfOrder)
        );
    }
    assert_eq!(gate.last_accepted_sequence(), Some(Generation::new(4)?));
    Ok(())
}

#[test]
fn state_invalidation_rejects_old_work_and_reopens_sample_sequence() -> Result<(), Box<dyn Error>> {
    let shape = shape()?;
    let releases = Arc::new(AtomicUsize::new(0));
    let mut gate = gate(shape, 3, true)?;
    let old = prepared(shape, 7, 3, 100, &releases)?;
    assert_eq!(gate.accept(&old), FramePublicationDecision::Accepted);
    assert_eq!(gate.invalidate_state()?, Generation::new(4)?);
    assert_eq!(gate.last_accepted_sequence(), None);
    assert_eq!(gate.accept(&old), FramePublicationDecision::Rejected(FrameRejection::WrongState));
    assert_eq!(
        gate.accept(&prepared(shape, 7, 4, 1, &releases)?),
        FramePublicationDecision::Accepted
    );
    Ok(())
}

#[test]
fn resize_and_visibility_advance_before_publication_and_equal_values_are_inert()
-> Result<(), Box<dyn Error>> {
    let initial = shape()?;
    let resized = FrameShape::new(3, 2, 1.5)?;
    let releases = Arc::new(AtomicUsize::new(0));
    let mut gate = gate(initial, 3, true)?;
    let old = prepared(initial, 7, 3, 9, &releases)?;
    assert_eq!(gate.accept(&old), FramePublicationDecision::Accepted);
    assert_eq!(gate.resize(initial)?, Generation::new(3)?);
    assert_eq!(gate.set_visible(true)?, Generation::new(3)?);
    assert_eq!(gate.last_accepted_sequence(), Some(Generation::new(9)?));
    assert_eq!(gate.resize(resized)?, Generation::new(4)?);
    assert_eq!(gate.shape(), resized);
    assert_eq!(gate.accept(&old), FramePublicationDecision::Rejected(FrameRejection::WrongState));
    assert_eq!(gate.set_visible(false)?, Generation::new(5)?);
    assert!(!gate.visible());
    let hidden = prepared(resized, 7, 5, 1, &releases)?;
    assert_eq!(gate.accept(&hidden), FramePublicationDecision::Rejected(FrameRejection::Hidden));
    assert_eq!(gate.last_accepted_sequence(), None);
    assert_eq!(gate.set_visible(false)?, Generation::new(5)?);
    assert_eq!(gate.set_visible(true)?, Generation::new(6)?);
    assert_eq!(
        gate.accept(&hidden),
        FramePublicationDecision::Rejected(FrameRejection::WrongState)
    );
    assert_eq!(
        gate.accept(&prepared(resized, 7, 6, 1, &releases)?),
        FramePublicationDecision::Accepted
    );
    Ok(())
}

#[test]
fn exhausted_generation_preserves_shape_visibility_and_accepted_sequence()
-> Result<(), Box<dyn Error>> {
    let initial = shape()?;
    let releases = Arc::new(AtomicUsize::new(0));
    let mut gate = gate(initial, u64::MAX, true)?;
    let frame = prepared(initial, 7, u64::MAX, 8, &releases)?;
    assert_eq!(gate.accept(&frame), FramePublicationDecision::Accepted);
    assert_eq!(gate.invalidate_state(), Err(IdentityError::Exhausted));
    assert_eq!(gate.resize(FrameShape::new(3, 2, 1.5)?), Err(IdentityError::Exhausted));
    assert_eq!(gate.set_visible(false), Err(IdentityError::Exhausted));
    assert_eq!(gate.surface(), SurfaceId::new(7)?);
    assert_eq!(gate.shape(), initial);
    assert_eq!(gate.state_generation(), Generation::new(u64::MAX)?);
    assert!(gate.visible());
    assert_eq!(gate.last_accepted_sequence(), Some(Generation::new(8)?));
    assert_eq!(gate.accept(&frame), FramePublicationDecision::Rejected(FrameRejection::OutOfOrder));
    Ok(())
}

#[test]
fn publication_borrows_storage_and_never_releases_the_retained_frame_charge()
-> Result<(), Box<dyn Error>> {
    let shape = shape()?;
    let releases = Arc::new(AtomicUsize::new(0));
    let frame = prepared(shape, 7, 3, 1, &releases)?;
    let retained = frame.clone();
    {
        let mut gate = gate(shape, 3, true)?;
        assert_eq!(gate.accept(&frame), FramePublicationDecision::Accepted);
        assert_eq!(
            gate.accept(&frame),
            FramePublicationDecision::Rejected(FrameRejection::OutOfOrder)
        );
    }
    drop(frame);
    assert_eq!(releases.load(Ordering::SeqCst), 0);
    assert_eq!(retained.pixels(), &[0; 8]);
    drop(retained);
    assert_eq!(releases.load(Ordering::SeqCst), 1);
    Ok(())
}
