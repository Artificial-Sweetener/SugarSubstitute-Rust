// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project immutable retained GPU backings without holding the image-operation renderer lock.
use super::*;

struct VerificationWorkspace {
    native: DisplayReadbackWorkspace,
    source: Option<DisplayReadbackWorkspace>,
}
/// Canonical CPU references retained only while checking the actual native composite.
struct VerificationSources<'a> {
    primary: Option<&'a CpuSurface>,
    underlay: Option<(&'a GpuUnderlay, &'a ProjectionSamples)>,
}

/// Never wait for the image-operation renderer lock while drawing a retained native image.
pub(super) fn prepare(
    resources: &Resources,
    frame: &GpuFrame,
    reference: Option<&CpuSurface>,
    underlay: Option<&GpuUnderlay>,
    view: ViewProjection,
    context: &JobContext,
) -> Result<Option<NativeFrame>, WorkError> {
    let samples = if view.source() != view.target() || underlay.is_some() {
        Some(ProjectionSamples::new(view, context)?)
    } else {
        None
    };
    let underlay_samples =
        underlay.map(|underlay| ProjectionSamples::new(underlay.view, context)).transpose()?;
    let matte = Pixel::linear(0.018, 0.018, 0.018, 1.0)?;
    let mut verification = if resources.verify_native {
        let native = match retry_gpu_readback(
            || frame.image().readback_workspace(context),
            || reclaim_gpu_cache(resources),
        )? {
            ReadbackOutcome::Complete(workspace) => workspace,
            ReadbackOutcome::Deferred => return Ok(None),
        };
        let source = if samples.is_none() {
            match retry_gpu_readback(
                || frame.image().readback_workspace(context),
                || reclaim_gpu_cache(resources),
            )? {
                ReadbackOutcome::Complete(workspace) => Some(workspace),
                ReadbackOutcome::Deferred => return Ok(None),
            }
        } else {
            None
        };
        Some(VerificationWorkspace { native, source })
    } else {
        None
    };
    let mut display = resources
        .display
        .lock()
        .map_err(|_| WorkError::invalid("Canvas native presentation state failed"))?;
    let native = match display.as_mut() {
        #[cfg(all(windows, feature = "native-composition"))]
        Some(DisplayTarget::Composition(target)) => {
            let sources = super::composition::Sources {
                frame,
                samples: samples.as_ref(),
                underlay: underlay.zip(underlay_samples.as_ref()),
                matte,
            };
            return target
                .draw(&sources, context, || reclaim_gpu_cache(resources))
                .map(|frame| frame.map(NativeFrame::Composition));
        }
        Some(DisplayTarget::Window(window)) => loop {
            let prepared = match (&samples, underlay, &underlay_samples) {
                (Some(samples), Some(underlay), Some(underlay_samples)) => window
                    .prepare_composited(
                        frame.image(),
                        samples.gpu_mapping(matte)?,
                        underlay.frame.image(),
                        underlay_samples.gpu_mapping(matte)?,
                        context,
                    ),
                (Some(samples), _, _) => {
                    window.prepare_projected(frame.image(), samples.gpu_mapping(matte)?, context)
                }
                (None, _, _) => window.prepare(frame.image(), context),
            };
            match prepared {
                Err(cause) if cause.is_memory_pressure() => match reclaim_gpu_cache(resources)? {
                    CacheReclaim::Progress => {}
                    CacheReclaim::Busy => break None,
                    CacheReclaim::Exhausted => return Err(cause.into()),
                },
                result => break result?,
            }
        },
        None => None,
    };
    if let Some(verification) = &mut verification
        && let Some(native) = &native
    {
        let sources = VerificationSources {
            primary: reference,
            underlay: underlay.zip(underlay_samples.as_ref()),
        };
        verify_gpu(frame, native, sources, samples.as_ref(), matte, verification, context)?;
    }
    Ok(native.map(NativeFrame::Window))
}

fn reclaim_gpu_cache(resources: &Resources) -> Result<CacheReclaim, WorkError> {
    let mut engine = match resources.engine.try_lock() {
        Ok(engine) => engine,
        Err(TryLockError::WouldBlock) => return Ok(CacheReclaim::Busy),
        Err(TryLockError::Poisoned(_)) => {
            return Err(WorkError::invalid("Canvas preparation state failed"));
        }
    };
    let evicted = match engine.renderer.as_mut() {
        Some(Renderer::Gpu(renderer)) => renderer.evict_one(),
        _ => false,
    };
    Ok(if evicted { CacheReclaim::Progress } else { CacheReclaim::Exhausted })
}

/// Proof-only readback is independent of normal presentation timing and source evaluation.
fn verify_gpu(
    frame: &GpuFrame,
    native: &WindowFrame,
    sources: VerificationSources<'_>,
    samples: Option<&ProjectionSamples>,
    matte: Pixel,
    workspace: &mut VerificationWorkspace,
    context: &JobContext,
) -> Result<(), WorkError> {
    if let Some(samples) = samples {
        let reference = sources.primary.ok_or_else(|| {
            WorkError::invalid("GPU projection qualification lacks its canonical CPU reference")
        })?;
        if reference.current() != Some(frame.target()) {
            return Err(WorkError::invalid(
                "GPU projection reference represents another source generation",
            ));
        }
        let underlay_reference = sources
            .underlay
            .map(|(underlay, samples)| {
                let reference = underlay.reference.as_deref().ok_or_else(|| {
                    WorkError::invalid(
                        "GPU underlay qualification lacks its canonical CPU reference",
                    )
                })?;
                if reference.current() != Some(underlay.frame.target()) {
                    return Err(WorkError::invalid(
                        "GPU underlay reference represents another generation",
                    ));
                }
                Ok::<_, WorkError>((reference, samples))
            })
            .transpose()?;
        let rgba = sugarsubstitute_image_ops::SrgbEncoder::encode_constant(matte)?;
        let background = u32::from_be_bytes([255, rgba[0], rgba[1], rgba[2]]);
        for region in samples.view().target().surface().region().tiles() {
            let actual = native.read_region_into(region, &mut workspace.native, context)?;
            for y in region.y()..region.bottom() {
                for x in region.x()..region.right() {
                    let expected = match underlay_reference {
                        Some((underlay, underlay_samples)) if !samples.covers_pixel(x, y) => {
                            underlay_samples.sample_pixel(underlay, x, y, background)
                        }
                        _ => samples.sample_pixel(reference, x, y, background),
                    };
                    let index = ((y - region.y()) * region.width() + x - region.x()) as usize;
                    if actual[index] != expected {
                        return Err(WorkError::invalid(
                            "Native GPU projection differs from canonical display sampling",
                        ));
                    }
                }
            }
        }
    } else {
        for region in frame.image().extent().region().tiles() {
            let source_workspace = workspace.source.as_mut().ok_or_else(|| {
                WorkError::invalid("Canonical GPU verification lacks preallocated readback storage")
            })?;
            let source = native.read_reference_region_into(
                frame.image(),
                region,
                source_workspace,
                context,
            )?;
            let actual = native.read_region_into(region, &mut workspace.native, context)?;
            if actual != source {
                return Err(WorkError::invalid(
                    "Native GPU presentation differs from its completed display image",
                ));
            }
        }
    }
    Ok(())
}
