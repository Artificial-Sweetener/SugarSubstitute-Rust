// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep complete prepared image adoption zero-copy and preserve its exact frame identity.
use super::*;
use crate::tests::harness;
use sugarsubstitute_contracts::execution::Generation;

#[test]
fn complete_prepared_pixels_keep_their_storage_and_identity_when_adopted() -> harness::TestResult {
    harness::run(8192, |context| {
        let extent = Extent::new(7, 9)?;
        let target = FrameStamp {
            surface: SurfaceId::new(1).map_err(|cause| ImageError::Source(Box::new(cause)))?,
            generation: Generation::new(1).map_err(|cause| ImageError::Source(Box::new(cause)))?,
        };
        let update = PreparedUpdate::prepare(
            target,
            None,
            DamagePlan::full(extent),
            context.memory(),
            |_, pixels| {
                pixels.fill(0xff123456);
                Ok(())
            },
        )
        .map_err(|cause| ImageError::Source(Box::new(cause)))?;
        let allocation = update.patches[0]
            .as_ref()
            .ok_or(ImageError::Invalid("Missing prepared pixels"))?
            .pixels
            .as_ptr();
        let surface = CpuSurface::from_prepared(update)
            .map_err(|cause| ImageError::Source(Box::new(cause)))?;
        assert_eq!(surface.pixels().as_ptr(), allocation);
        assert_eq!(surface.extent(), extent);
        assert_eq!(surface.current(), Some(target));
        assert!(surface.pixels().iter().all(|word| *word == 0xff123456));
        let dependent = PreparedUpdate::prepare(
            FrameStamp {
                generation: Generation::new(2)
                    .map_err(|cause| ImageError::Source(Box::new(cause)))?,
                ..target
            },
            Some(target),
            DamagePlan::full(extent),
            context.memory(),
            |_, pixels| {
                pixels.fill(0xff654321);
                Ok(())
            },
        )
        .map_err(|cause| ImageError::Source(Box::new(cause)))?;
        assert!(CpuSurface::from_prepared(dependent).is_err());
        Ok(())
    })
}
