// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bounded sparse batches preserve admission, control, integrity and untouched chunk gaps.
use super::{CountedReader, values};
use crate::{
    ImportError, Packing, RasterArchive, RasterDescriptor, Transfer, tests::harness, write_raster,
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
    Extent, Filter, ImageError, Pixel, RasterSamples, RasterSource, Region, Sampling,
};

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
fn sparse_decode_does_not_batch_across_untouched_chunk_spans() -> harness::TestResult {
    harness::run(2 * 1024 * 1024, |context| {
        let descriptor = RasterDescriptor::new(
            Extent::new(1025, 1)?,
            Packing::Rgba8,
            Transfer::Gamma(1.0),
            [11; 32],
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
        let mut source = RasterArchive::open(reader, descriptor, context.memory(), context)?;
        let samples = RasterSamples::new(
            descriptor.layout().extent(),
            Region::new(0, 0, 4, 1)?,
            Sampling::new([256.0, 1.0], [0.0, 0.0])?.with_filter(Filter::Nearest),
        )?;
        let columns: Vec<_> = samples.columns().iter().map(|column| column / 128).collect();
        assert!(columns.windows(2).all(|pair| pair[1] > pair[0] + 1));
        let mut output = [Pixel::TRANSPARENT; 4];
        source.read_samples(descriptor.asset(), &samples, &mut output, context)?;
        for (index, (&x, pixel)) in samples.columns().iter().zip(output).enumerate() {
            let rgba = values(x, samples.rows()[0], 255);
            assert_eq!(
                pixel,
                Pixel::linear(
                    rgba[0] as f32 / 255.0,
                    rgba[1] as f32 / 255.0,
                    rgba[2] as f32 / 255.0,
                    rgba[3] as f32 / 255.0,
                )?,
                "sample {index}"
            );
        }
        let expected_bytes = columns.iter().try_fold(0_u64, |total, &column| {
            let chunk = descriptor.layout().region(column, 0)?;
            Ok::<_, ImportError>(total + chunk.pixels() * 4 + 32)
        })?;
        assert_eq!(source.stats().chunks, columns.len() as u64);
        assert_eq!(source.stats().read_bytes, expected_bytes);
        assert_eq!(reads.get(), 1 + columns.len() as u64);
        assert_eq!(seeks.get(), 2 + columns.len() as u64);
        assert_eq!(lengths.borrow()[0], 128);
        assert_eq!(
            lengths.borrow()[1..].iter().map(|length| *length as u64).sum::<u64>(),
            expected_bytes
        );
        Ok(())
    })
}

#[test]
fn sparse_workspace_uses_one_source_sized_claim_and_rejects_insufficient_admission()
-> harness::TestResult {
    harness::run(2 * 1024 * 1024, |context| {
        let descriptor = RasterDescriptor::new(
            Extent::new(513, 259)?,
            Packing::Rgba8,
            Transfer::Srgb,
            [12; 32],
        )?;
        let mut bytes = Vec::new();
        write_raster(&mut bytes, descriptor, context, |_, row| {
            row.fill(255);
            Ok(())
        })?;
        let available = || -> Result<u64, ImportError> {
            Ok(context
                .memory()
                .claim(u64::MAX)
                .err()
                .ok_or(ImportError::Invalid("Unbounded image test claim was accepted"))?
                .available_bytes)
        };
        let layout = descriptor.layout();
        let encoded = layout.stripe_bytes() + layout.columns() as usize * 32;
        let expected = encoded as u64 + 128 + 256 * 4 + 128;
        assert!(encoded + 128 <= 1024 * 1024);
        let before = available()?;
        let source =
            RasterArchive::open(Cursor::new(bytes.clone()), descriptor, context.memory(), context)?;
        assert_eq!(available()?, before - expected);
        drop(source);
        assert_eq!(available()?, before);

        let blocker = context.memory().claim(before - (expected - 1))?;
        assert!(matches!(
            RasterArchive::open(Cursor::new(bytes), descriptor, context.memory(), context),
            Err(ImportError::Image(ImageError::Memory(cause))) if cause.available_bytes < expected
        ));
        assert_eq!(available()?, expected - 1);
        drop(blocker);
        assert_eq!(available()?, before);
        Ok(())
    })
}

#[test]
fn sparse_batch_observes_cancellation_after_its_native_read_before_decoding_pixels()
-> harness::TestResult {
    let output = Arc::new(Mutex::new(vec![Pixel::TRANSPARENT; 96 * 81]));
    let observed = Arc::new(AtomicBool::new(false));
    let worker_output = output.clone();
    let worker_observed = observed.clone();
    harness::run_cancelled(4 * 1024 * 1024, move |context, point| {
        let descriptor = RasterDescriptor::new(
            Extent::new(513, 259)?,
            Packing::Rgba8,
            Transfer::Gamma(1.0),
            [13; 32],
        )?;
        let mut bytes = Vec::new();
        write_raster(&mut bytes, descriptor, context, |_, row| {
            row.fill(255);
            Ok(())
        })?;
        let armed = Arc::new(AtomicBool::new(false));
        let reader = PausingReader { bytes: Cursor::new(bytes), armed: armed.clone(), point };
        let mut source = RasterArchive::open(reader, descriptor, context.memory(), context)?;
        let samples = RasterSamples::new(
            descriptor.layout().extent(),
            Region::new(0, 0, 96, 81)?,
            Sampling::new([7.75, 4.25], [-60.25, -20.5])?.with_filter(Filter::Nearest),
        )?;
        armed.store(true, Ordering::SeqCst);
        let mut output = worker_output.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let cause = match source.read_samples(descriptor.asset(), &samples, &mut output, context) {
            Ok(()) => return Err(ImportError::Invalid("Cancelled sparse read completed")),
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
        Err(ImportError::from(cause))
    })?;
    assert!(observed.load(Ordering::SeqCst));
    assert!(
        output
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .all(|pixel| *pixel == Pixel::TRANSPARENT)
    );
    Ok(())
}

#[test]
fn sparse_batch_authenticates_each_touched_chunk_before_using_its_samples() -> harness::TestResult {
    harness::run(2 * 1024 * 1024, |context| {
        let descriptor =
            RasterDescriptor::new(Extent::new(257, 1)?, Packing::Rgba8, Transfer::Srgb, [14; 32])?;
        let mut bytes = Vec::new();
        write_raster(&mut bytes, descriptor, context, |_, row| {
            row.fill(255);
            Ok(())
        })?;
        let corrupt = usize::try_from(descriptor.layout().offset(1, 0)?)
            .map_err(|_| ImportError::Invalid("Image test offset exceeds address space"))?
            + 32
            + 1;
        bytes[corrupt] ^= 1;
        let reads = Cell::new(0);
        let seeks = Cell::new(0);
        let lengths = RefCell::new(Vec::new());
        let reader = CountedReader {
            bytes: Cursor::new(bytes),
            reads: &reads,
            seeks: &seeks,
            lengths: &lengths,
        };
        let mut source = RasterArchive::open(reader, descriptor, context.memory(), context)?;
        let samples = RasterSamples::new(
            descriptor.layout().extent(),
            Region::new(0, 0, 96, 1)?,
            Sampling::new([3.0, 1.0], [0.0, 0.0])?.with_filter(Filter::Nearest),
        )?;
        let columns: Vec<_> = samples.columns().iter().map(|column| column / 128).collect();
        assert_eq!(columns.first(), Some(&0));
        assert!(columns.contains(&1));
        assert_eq!(columns.last(), Some(&2));
        let corrupt_start = columns.partition_point(|column| *column == 0);
        let mut output = [Pixel::TRANSPARENT; 96];
        assert!(matches!(
            source.read_samples(descriptor.asset(), &samples, &mut output, context),
            Err(ImageError::Source(_))
        ));
        assert_eq!(reads.get(), 2);
        assert_eq!(source.stats().read_bytes + 128, descriptor.layout().length());
        assert_eq!(source.stats().chunks, 2);
        assert!(output.iter().take(corrupt_start).all(|pixel| *pixel != Pixel::TRANSPARENT));
        assert!(output.iter().skip(corrupt_start).all(|pixel| *pixel == Pixel::TRANSPARENT));
        Ok(())
    })
}
