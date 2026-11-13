// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify the authored texture phases independently of a native font's small glyph size.

use super::*;
use crate::text::FontOwner;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::configuration::Theme;
use sugarsubstitute_contracts::execution::{MemoryLease, MemoryRelease};

struct Release(Arc<AtomicUsize>);
impl MemoryRelease for Release {
    fn release(&self, _: u64) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// Replace the raster boundary with a larger ready/uninitialized coverage fixture.
/// Its real retained analysis remains owned but is never invoked for the synthetic rectangle.
fn fixture(
    ready: bool,
) -> Result<(GlyphPaint, Bitmap, Arc<AtomicUsize>), Box<dyn std::error::Error>> {
    let released = Arc::new(AtomicUsize::new(0));
    let lease = MemoryLease::new(6 * 1024 * 1024, Box::new(Release(released.clone())));
    let mut text = FontOwner::new("en")?;
    let mut cursor = text.prepare(
        "L",
        crate::text::fixtures::status(Theme::Dark, 1.0, 558, 558, true),
        lease.clone(),
    )?;
    let mut target = Bitmap::new(558, 558).ok_or("Cannot allocate budget fixture")?;
    assert!(!cursor.step(&mut target, NonZeroUsize::new(1).ok_or("Invalid budget")?)?);
    let native = cursor.texture.take().ok_or("No native glyph analysis")?;
    let analysis = native.analysis.clone();
    drop(native);
    let claim = lease.claim(TEXTURE_BYTES as u64)?;
    let mut coverage = memory::reserve(300 * 300 * 3, TEXTURE_BYTES)?;
    if ready {
        coverage.resize(300 * 300 * 3, 127);
    }
    cursor.clip = [0, 0, 558, 558];
    cursor.texture = Some(Texture {
        analysis,
        coverage,
        bounds: RECT { left: 0, top: 0, right: 300, bottom: 300 },
        width: 300,
        length: 300 * 300 * 3,
        position: 0,
        ready,
        _claim: Some(claim),
    });
    drop(lease);
    Ok((cursor, target, released))
}

/// Maximum caller budgets cannot cause whole-texture authored initialization/composition.
#[test]
fn status_texture_phases_clamp_work_and_release_cancelled_storage()
-> Result<(), Box<dyn std::error::Error>> {
    let max = NonZeroUsize::new(usize::MAX).ok_or("Invalid budget")?;
    let (mut cursor, mut target, released) = fixture(false)?;
    assert!(!cursor.step(&mut target, max)?);
    assert_eq!(cursor.texture.as_ref().ok_or("Missing texture")?.coverage.len(), 65_536 * 3);
    assert!(target.data().iter().all(|byte| *byte == 0));
    drop(cursor);
    assert_eq!(released.load(Ordering::SeqCst), 1);
    let (mut cursor, mut target, released) = fixture(true)?;
    assert!(!cursor.step(&mut target, max)?);
    assert_eq!(cursor.texture.as_ref().ok_or("Missing texture")?.position, 65_536);
    assert_eq!(target.data().chunks_exact(4).filter(|p| p[3] != 0).count(), 65_536);
    assert!(cursor.step(&mut target, max)?);
    assert_eq!(target.data().chunks_exact(4).filter(|p| p[3] != 0).count(), 90_000);
    drop(cursor);
    assert_eq!(released.load(Ordering::SeqCst), 1);
    Ok(())
}
