// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Multi-plan sparse reads follow archive order without weakening exact output or integrity.
use super::{CountedReader, values};
use crate::{
    Packing, RasterArchive, RasterDescriptor, SharedRaster, Transfer, tests::harness, write_raster,
};
use std::{
    cell::{Cell, RefCell},
    io::{self, Cursor, Read, Seek, SeekFrom},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use sugarsubstitute_image_ops::{
    Extent, Filter, ImageError, Pixel, RasterSampleRequest, RasterSamples, RasterSource, Region,
    Sampling,
};

#[test]
fn adjacent_plans_read_each_complete_source_stripe_as_one_sequential_range() -> harness::TestResult
{
    harness::run(8 * 1024 * 1024, |context| {
        let descriptor = RasterDescriptor::new(
            Extent::new(1024, 256)?,
            Packing::Rgba8,
            Transfer::Gamma(1.0),
            [21; 32],
        )?;
        let mut bytes = Vec::new();
        write_raster(&mut bytes, descriptor, context, |y, row| {
            for (x, pixel) in row.chunks_exact_mut(4).enumerate() {
                for (channel, value) in values(x as u32, y, 255).into_iter().enumerate() {
                    pixel[channel] = value as u8;
                }
            }
            Ok(())
        })?;
        let reads = Cell::new(0);
        let seeks = Cell::new(0);
        let lengths = RefCell::new(Vec::new());
        let reader = CountedReader {
            bytes: Cursor::new(bytes),
            reads: &reads,
            seeks: &seeks,
            lengths: &lengths,
        };
        let archive = RasterArchive::open(reader, descriptor, context.memory(), context)?;
        let mut source = SharedRaster::new(archive, context.memory())?;
        let sampling = Sampling::new([4.0, 4.0], [0.0, 0.0])?.with_filter(Filter::Nearest);
        let left = RasterSamples::new(
            descriptor.layout().extent(),
            Region::new(0, 0, 128, 64)?,
            sampling,
        )?;
        let right = RasterSamples::new(
            descriptor.layout().extent(),
            Region::new(128, 0, 128, 64)?,
            sampling,
        )?;
        let mut left_output = vec![Pixel::TRANSPARENT; left.region().pixels() as usize];
        let mut right_output = vec![Pixel::TRANSPARENT; right.region().pixels() as usize];
        let before = source.stats()?;
        source.read_sample_batch(
            descriptor.asset(),
            &mut [
                RasterSampleRequest { samples: &left, output: &mut left_output },
                RasterSampleRequest { samples: &right, output: &mut right_output },
            ],
            context,
        )?;

        verify_pixels(&left, &left_output)?;
        verify_pixels(&right, &right_output)?;
        let stats = source.stats()?.since(before);
        assert_eq!(stats.chunks, 16);
        assert_eq!(stats.read_bytes, 1_049_088);
        assert_eq!(stats.range_reads, 2);
        assert_eq!(stats.range_seeks, 1);
        assert_eq!(reads.get(), 3);
        assert_eq!(seeks.get(), 3);
        assert_eq!(lengths.borrow().as_slice(), &[128, 524_544, 524_544]);
        assert_eq!(before.since(source.stats()?), Default::default());
        Ok(())
    })
}

#[test]
fn corrupt_chunk_stops_before_later_plan_pixels_are_published() -> harness::TestResult {
    harness::run(8 * 1024 * 1024, |context| {
        let descriptor = RasterDescriptor::new(
            Extent::new(1024, 128)?,
            Packing::Rgba8,
            Transfer::Gamma(1.0),
            [22; 32],
        )?;
        let mut bytes = Vec::new();
        write_raster(&mut bytes, descriptor, context, |y, row| {
            for (x, pixel) in row.chunks_exact_mut(4).enumerate() {
                for (channel, value) in values(x as u32, y, 255).into_iter().enumerate() {
                    pixel[channel] = value as u8;
                }
            }
            Ok(())
        })?;
        let corrupt = usize::try_from(descriptor.layout().offset(4, 0)?)
            .map_err(|_| crate::ImportError::Invalid("Image test offset exceeds address space"))?
            + 33;
        bytes[corrupt] ^= 1;
        let mut source =
            RasterArchive::open(Cursor::new(bytes), descriptor, context.memory(), context)?;
        let sampling = Sampling::new([4.0, 4.0], [0.0, 0.0])?.with_filter(Filter::Nearest);
        let left = RasterSamples::new(
            descriptor.layout().extent(),
            Region::new(0, 0, 128, 32)?,
            sampling,
        )?;
        let right = RasterSamples::new(
            descriptor.layout().extent(),
            Region::new(128, 0, 128, 32)?,
            sampling,
        )?;
        let mut left_output = vec![Pixel::TRANSPARENT; left.region().pixels() as usize];
        let mut right_output = vec![Pixel::TRANSPARENT; right.region().pixels() as usize];
        assert!(matches!(
            source.read_sample_batch(
                descriptor.asset(),
                &mut [
                    RasterSampleRequest { samples: &left, output: &mut left_output },
                    RasterSampleRequest { samples: &right, output: &mut right_output },
                ],
                context,
            ),
            Err(ImageError::Source(_))
        ));
        verify_pixels(&left, &left_output)?;
        assert!(right_output.iter().all(|pixel| *pixel == Pixel::TRANSPARENT));
        assert_eq!(source.stats().chunks, 5);
        assert_eq!(source.stats().read_bytes, 524_544);
        assert_eq!(source.stats().range_reads, 1);
        assert_eq!(source.stats().range_seeks, 1);
        Ok(())
    })
}

struct PausingReader<'a> {
    bytes: Cursor<Vec<u8>>,
    armed: Arc<AtomicBool>,
    point: &'a harness::CancelPoint,
}
impl Read for PausingReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if self.armed.swap(false, Ordering::SeqCst) {
            self.point.pause();
        }
        self.bytes.read(output)
    }
}
impl Seek for PausingReader<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(position)
    }
}

#[test]
fn cancellation_after_batch_read_leaves_every_destination_untouched() -> harness::TestResult {
    let output = Arc::new(Mutex::new((
        vec![Pixel::TRANSPARENT; 128 * 32],
        vec![Pixel::TRANSPARENT; 128 * 32],
    )));
    let observed = Arc::new(AtomicBool::new(false));
    let worker_output = Arc::clone(&output);
    let worker_observed = Arc::clone(&observed);
    harness::run_cancelled(8 * 1024 * 1024, move |context, point| {
        let descriptor = RasterDescriptor::new(
            Extent::new(1024, 128)?,
            Packing::Rgba8,
            Transfer::Gamma(1.0),
            [23; 32],
        )?;
        let mut bytes = Vec::new();
        write_raster(&mut bytes, descriptor, context, |_, row| {
            row.fill(255);
            Ok(())
        })?;
        let armed = Arc::new(AtomicBool::new(false));
        let reader = PausingReader { bytes: Cursor::new(bytes), armed: Arc::clone(&armed), point };
        let mut source = RasterArchive::open(reader, descriptor, context.memory(), context)?;
        let sampling = Sampling::new([4.0, 4.0], [0.0, 0.0])?.with_filter(Filter::Nearest);
        let left = RasterSamples::new(
            descriptor.layout().extent(),
            Region::new(0, 0, 128, 32)?,
            sampling,
        )?;
        let right = RasterSamples::new(
            descriptor.layout().extent(),
            Region::new(128, 0, 128, 32)?,
            sampling,
        )?;
        armed.store(true, Ordering::SeqCst);
        let mut output = worker_output.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let (left_output, right_output) = &mut *output;
        let cause = match source.read_sample_batch(
            descriptor.asset(),
            &mut [
                RasterSampleRequest { samples: &left, output: left_output },
                RasterSampleRequest { samples: &right, output: right_output },
            ],
            context,
        ) {
            Ok(()) => return Err(crate::ImportError::Invalid("Cancelled sparse batch completed")),
            Err(cause) => cause,
        };
        let mut current: Option<&dyn std::error::Error> = Some(&cause);
        while let Some(error) = current {
            if matches!(error.downcast_ref::<ImageError>(), Some(ImageError::Cancelled)) {
                worker_observed.store(true, Ordering::SeqCst);
                break;
            }
            current = error.source();
        }
        Err(crate::ImportError::from(cause))
    })?;
    assert!(observed.load(Ordering::SeqCst));
    let output = output.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(output.0.iter().chain(&output.1).all(|pixel| *pixel == Pixel::TRANSPARENT));
    Ok(())
}

fn verify_pixels(samples: &RasterSamples, output: &[Pixel]) -> Result<(), ImageError> {
    for (y, &source_y) in samples.rows().iter().enumerate() {
        for (x, &source_x) in samples.columns().iter().enumerate() {
            let rgba = values(source_x, source_y, 255);
            assert_eq!(
                output[y * samples.columns().len() + x],
                Pixel::linear(
                    rgba[0] as f32 / 255.0,
                    rgba[1] as f32 / 255.0,
                    rgba[2] as f32 / 255.0,
                    rgba[3] as f32 / 255.0,
                )?
            );
        }
    }
    Ok(())
}
