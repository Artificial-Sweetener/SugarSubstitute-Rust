// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Camera invariants and exhaustive exposed-edge repair against clean redraws.
use super::*;
use sugarsubstitute_image_ops::{Extent, Region};
type TestResult = Result<(), Box<dyn std::error::Error>>;
pub(crate) mod harness;
mod phase;

#[test]
fn cursor_centered_zoom_preserves_the_document_point() -> TestResult {
    let mut view = Viewport::new(Extent::new(1024, 768)?)?;
    view.pan_by([137.0, -81.0])?;
    let anchor = [177.25, 621.5];
    let point = view.document_at(anchor)?;
    view.zoom_at(3.75, anchor)?;
    let after = view.document_at(anchor)?;
    assert!((point[0] - after[0]).abs() < 1e-10);
    assert!((point[1] - after[1]).abs() < 1e-10);
    let mapped = view.device_at(point)?;
    assert!((mapped[0] - anchor[0]).abs() < 1e-10);
    assert!((mapped[1] - anchor[1]).abs() < 1e-10);
    view.actual_size(anchor)?;
    assert_eq!(view.zoom(), 1.0);
    assert_eq!(view.document_at(anchor)?, point);
    Ok(())
}

#[test]
fn fit_and_resize_keep_content_centered_without_document_mutations() -> TestResult {
    let mut view = Viewport::new(Extent::new(1000, 800)?)?;
    view.fit(Extent::new(10_000, 4_000)?)?;
    assert_eq!(view.zoom(), 0.1);
    assert_eq!(view.document_at([500.0, 400.0])?, [5000.0, 2000.0]);
    view.resize(Extent::new(1600, 1200)?)?;
    assert_eq!(view.document_at([800.0, 600.0])?, [5000.0, 2000.0]);
    Ok(())
}

#[test]
fn invalid_navigation_leaves_the_preceding_view_usable() -> TestResult {
    let mut view = Viewport::new(Extent::new(640, 480)?)?;
    let before = view;
    assert!(view.pan_by([f64::NAN, 0.0]).is_err());
    assert!(view.zoom_at(0.0, [0.0; 2]).is_err());
    assert!(view.zoom_at(f64::INFINITY, [0.0; 2]).is_err());
    assert!(view.actual_size([f64::NAN, 0.0]).is_err());
    assert!(view.resize(Extent::new(16385, 1)?).is_err());
    assert_eq!(view, before);
    Ok(())
}

#[test]
fn fractional_pan_zoom_and_resize_invalidate_sample_phase() -> TestResult {
    let before = Viewport::new(Extent::new(640, 480)?)?;
    let mut next = before;
    next.pan_by([8.0, -3.0])?;
    assert!(next.shares_sampling_grid(before));
    assert!(next.repair_from(before)?.blit().is_some());
    next.pan_by([0.25, 0.0])?;
    assert!(!next.shares_sampling_grid(before));
    assert_eq!(next.repair_from(before)?.dirty().pixels(), 640 * 480);
    next = before;
    next.zoom_at(2.0, [320.0, 240.0])?;
    assert!(!next.shares_sampling_grid(before));
    assert_eq!(next.repair_from(before)?.dirty().pixels(), 640 * 480);
    next = before;
    next.resize(Extent::new(641, 481)?)?;
    assert!(!next.shares_sampling_grid(before));
    assert_eq!(next.repair_from(before)?.dirty().pixels(), 641 * 481);
    Ok(())
}

#[test]
fn sampling_grid_identity_ignores_expansion_but_rejects_rebased_integer_phase() -> TestResult {
    let extent = Extent::new(640, 480)?;
    let mut translated = Viewport::new(extent)?;
    translated.reframe([0.0; 2], 3.5)?;
    translated.pan_by([1.0, 0.0])?;

    let mut expanded = translated;
    expanded.expand_surface(Extent::new(660, 500)?, [10, 10])?;
    assert!(translated.shares_sampling_grid(expanded));

    let mut rebased = Viewport::new(extent)?;
    rebased.reframe(translated.origin(), translated.zoom())?;
    let projection = ViewProjection::new(translated, rebased)?;
    assert_eq!(projection.scale(), 1.0);
    assert_eq!(projection.offset(), [0.0; 2]);
    assert!(!translated.shares_sampling_grid(rebased));
    assert_eq!(rebased.repair_from(translated)?.dirty().pixels(), extent.pixels());
    Ok(())
}

#[test]
fn pan_grid_aligns_fractional_cameras_to_padded_large_origins_with_bounded_repair() -> TestResult {
    let surface = Extent::new(640, 480)?;
    let mut base = Viewport::new(surface)?;
    base.reframe([1.0e12, -1.0e12], 1.375)?;
    base.pan_by([5.0, -3.0])?;
    let coverage = BackingCoverage::new(base, [128, 128])?;
    let reference = coverage.viewport();
    assert_eq!(reference.surface(), Extent::new(896, 736)?);
    assert_eq!(ViewProjection::new(reference, base)?.offset(), [-128.0; 2]);
    let initial = base.on_sampling_grid(reference)?;
    assert_eq!(initial.surface(), surface);
    assert!(initial.shares_sampling_grid(reference));

    for delta in [[13.375, -7.625], [-11.375, 9.625]] {
        let mut actual = base;
        actual.pan_by(delta)?;
        let aligned = actual.on_sampling_grid(reference)?;
        assert_eq!(aligned.surface(), surface);
        assert!(aligned.shares_sampling_grid(reference));
        let pose = ViewProjection::new(aligned, actual)?;
        assert_eq!(pose.scale(), 1.0);
        assert!(pose.offset().iter().all(|offset| offset.abs() <= 0.5));
        let damage = aligned.repair_from(initial)?;
        assert!(damage.blit().is_some());
        assert!(damage.dirty().pixels() < surface.pixels());
    }
    Ok(())
}

#[test]
fn pan_grid_repeated_fractional_reversal_returns_without_drift() -> TestResult {
    let surface = Extent::new(320, 240)?;
    let base = Viewport::new(surface)?;
    let reference = BackingCoverage::new(base, [128, 128])?.viewport();
    let initial = base.on_sampling_grid(reference)?;
    let mut actual = base;
    for _ in 0..128 {
        actual.pan_by([0.375, -0.375])?;
        let forward = actual.on_sampling_grid(reference)?;
        assert!(forward.shares_sampling_grid(initial));
        actual.pan_by([-0.375, 0.375])?;
        assert_eq!(actual.on_sampling_grid(reference)?, initial);
    }
    Ok(())
}

#[test]
fn pan_grid_alignment_rejects_zoom_and_translation_overflow_without_mutation() -> TestResult {
    let surface = Extent::new(64, 64)?;
    let target = Viewport::new(surface)?;
    let before = target;
    let mut different_zoom = target;
    different_zoom.zoom_at(2.0, [0.0; 2])?;
    assert!(target.on_sampling_grid(different_zoom).is_err());
    assert_eq!(target, before);

    let mut overflow = target;
    overflow.reframe([-f64::from(i32::MAX) - 1.0, 0.0], 1.0)?;
    let overflow_before = overflow;
    assert!(overflow.on_sampling_grid(target).is_err());
    assert_eq!(overflow, overflow_before);
    Ok(())
}

#[test]
fn pan_repair_covers_each_pixel_exactly_once_in_all_directions() -> TestResult {
    let extent = Extent::new(11, 7)?;
    for dy in -9_i32..=9 {
        for dx in -13_i32..=13 {
            let plan = DamagePlan::pan(extent, dx, dy)?;
            let mut covered = [0_u8; 77];
            if dx == 0 && dy == 0 {
                assert!(plan.dirty().is_empty());
                continue;
            }
            if let Some(blit) = plan.blit() {
                for y in 0..blit.source.height() {
                    for x in 0..blit.source.width() {
                        let target = (y + blit.destination[1]) as usize * 11
                            + (x + blit.destination[0]) as usize;
                        covered[target] += 1;
                        assert_eq!(
                            i64::from(blit.destination[0] + x) - i64::from(blit.source.x() + x),
                            i64::from(dx)
                        );
                        assert_eq!(
                            i64::from(blit.destination[1] + y) - i64::from(blit.source.y() + y),
                            i64::from(dy)
                        );
                    }
                }
            }
            for region in plan.dirty().regions() {
                for y in region.y()..region.bottom() {
                    for x in region.x()..region.right() {
                        covered[y as usize * 11 + x as usize] += 1;
                    }
                }
            }
            assert!(covered.iter().all(|count| *count == 1), "Shift {dx},{dy}: {covered:?}");
            if dx.unsigned_abs() < 11 && dy.unsigned_abs() < 7 {
                assert_eq!(
                    plan.dirty().pixels(),
                    77 - u64::from((11 - dx.unsigned_abs()) * (7 - dy.unsigned_abs()))
                );
            }
        }
    }
    Ok(())
}

#[test]
fn extreme_translations_and_dirty_inventory_overflow_remain_bounded() -> TestResult {
    let extent = Extent::new(100, 100)?;
    for (dx, dy) in [(i32::MIN, 0), (0, i32::MAX), (i32::MAX, i32::MIN)] {
        let plan = DamagePlan::pan(extent, dx, dy)?;
        assert!(plan.blit().is_none());
        assert_eq!(plan.dirty().pixels(), 10_000);
    }
    let mut dirty = DirtyRegions::empty(extent);
    for x in 0..33 {
        dirty.insert(Region::new(x, 0, 1, 1)?);
    }
    assert_eq!(dirty.pixels(), 10_000);
    dirty.insert(Region::new(1000, 1000, 1, 1)?);
    assert_eq!(dirty.pixels(), 10_000);
    Ok(())
}

#[test]
fn document_history_preserves_source_geometry_and_publishes_monotonic_revisions()
-> harness::TestResult {
    harness::run(2 * 1024 * 1024, |context| {
        document_history(context)
            .map_err(|cause| sugarsubstitute_image_ops::ImageError::Source(Box::new(cause)))
    })
}
fn document_history(
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> Result<(), CanvasError> {
    use sugarsubstitute_image_ops::{AssetId, Pixel, VectorRectangle};
    let extent = Extent::new(64, 64)?;
    let mut document = Document::new(1, extent, context.memory())?;
    let image =
        Layer::new(LayerId::new(1)?, LayerContent::Raster { asset: AssetId([7; 32]), extent })?;
    assert!(document.apply(DocumentEdit::Insert { index: 0, layer: image })?);
    let original = document.snapshot();
    let geometry =
        VectorRectangle::new([0.5, 1.25, 25.5, 40.25], Pixel::linear(1.0, 0.0, 0.0, 1.0)?)?;
    let vector = Layer::new(LayerId::new(2)?, LayerContent::VectorRectangle(geometry))?;
    document.apply(DocumentEdit::Insert { index: 1, layer: vector })?;
    let edited = document.snapshot();
    assert_eq!(original.layers(), &[image]);
    assert_eq!(edited.layers()[1].content(), LayerContent::VectorRectangle(geometry));
    assert!(document.undo()?);
    assert_eq!(document.snapshot().layers(), original.layers());
    assert!(document.snapshot().revision() > edited.revision());
    let undo_revision = document.snapshot().revision();
    assert!(document.redo()?);
    assert_eq!(document.snapshot().layers(), edited.layers());
    assert!(document.snapshot().revision() > undo_revision);
    let before = document.snapshot().revision();
    assert!(!document.apply(DocumentEdit::Replace(vector))?);
    assert_eq!(document.snapshot().revision(), before);
    Ok(())
}

#[test]
fn rejected_edits_keep_revision_and_redo_branch_intact() -> harness::TestResult {
    harness::run(2 * 1024 * 1024, |context| {
        document_rejections(context)
            .map_err(|cause| sugarsubstitute_image_ops::ImageError::Source(Box::new(cause)))
    })
}
fn document_rejections(
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> Result<(), CanvasError> {
    use sugarsubstitute_image_ops::AssetId;
    let extent = Extent::new(64, 64)?;
    let mut document = Document::new(1, extent, context.memory())?;
    let image =
        Layer::new(LayerId::new(1)?, LayerContent::Raster { asset: AssetId([7; 32]), extent })?;
    document.apply(DocumentEdit::Insert { index: 0, layer: image })?;
    let hidden = image.with_visibility(false);
    document.apply(DocumentEdit::Replace(hidden))?;
    document.undo()?;
    let revision = document.snapshot().revision();
    assert!(document.apply(DocumentEdit::Insert { index: 0, layer: image }).is_err());
    assert!(document.apply(DocumentEdit::Remove(LayerId::new(99)?)).is_err());
    assert!(
        document.apply(DocumentEdit::Reorder { layer: image.id(), index: usize::MAX }).is_err()
    );
    assert_eq!(document.snapshot().revision(), revision);
    assert_eq!(document.snapshot().layers(), &[image]);
    assert!(document.redo()?);
    assert_eq!(document.snapshot().layers(), &[hidden]);
    document.undo()?;
    document.apply(DocumentEdit::Replace(image.with_opacity(0.5)?))?;
    assert!(!document.redo()?);
    assert!(LayerId::new(0).is_err());
    assert!(image.with_opacity(f32::NAN).is_err());
    assert!(Layer::new(LayerId::new(2)?, LayerContent::Exposure { stops: f32::INFINITY }).is_err());
    let wrong = Layer::new(
        LayerId::new(3)?,
        LayerContent::Raster { asset: AssetId([8; 32]), extent: Extent::new(32, 32)? },
    )?;
    assert!(document.apply(DocumentEdit::Insert { index: 1, layer: wrong }).is_err());
    Ok(())
}

#[test]
fn maximum_layer_stacks_admit_all_explicit_source_and_mask_pyramids() -> harness::TestResult {
    harness::run(4 * 1024 * 1024, |context| {
        maximum_stack(context)
            .map_err(|cause| sugarsubstitute_image_ops::ImageError::Source(Box::new(cause)))
    })
}
fn maximum_stack(
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> Result<(), CanvasError> {
    use sugarsubstitute_image_ops::{AssetId, Pixel};
    let extent = Extent::new(i32::MAX as u32, i32::MAX as u32)?;
    let mut document = Document::new(1, extent, context.memory())?;
    for index in 0..32 {
        let layer = Layer::new(
            LayerId::new(index as u64 + 1)?,
            LayerContent::Raster { asset: AssetId([index as u8; 32]), extent },
        )?
        .with_opacity(0.5)?
        .with_mask(Some(LayerMask::Raster { asset: AssetId([index as u8 + 32; 32]), extent }));
        document.apply(DocumentEdit::Insert { index, layer })?;
    }
    let snapshot = document.snapshot();
    let mut view = Viewport::new(Extent::new(64, 64)?)?;
    view.fit(extent)?;
    let compiled = snapshot.compile_view(
        view,
        None,
        Pixel::linear(0.0, 0.0, 0.0, 1.0)?,
        sugarsubstitute_image_ops::Reconstruction::Finished(
            sugarsubstitute_image_ops::Filter::Lanczos3,
        ),
        context.memory(),
    )?;
    assert_eq!(compiled.graph().extent(compiled.output())?, view.surface());
    Ok(())
}
