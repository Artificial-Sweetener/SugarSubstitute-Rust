// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Sparse source addressing retains integer precision, alpha and complete chunk integrity.
use super::*;
use std::{
    cell::Cell,
    io::{Read, Seek, SeekFrom},
};
use sugarsubstitute_image_ops::{Filter, RasterSamples, Sampling};

/// Count actual read-boundary calls independently of the archive's work counters.
struct CountedReader<'a> {
    bytes: Cursor<Vec<u8>>,
    reads: &'a Cell<u64>,
}
impl Read for CountedReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.reads.set(self.reads.get() + 1);
        self.bytes.read(output)
    }
}
impl Seek for CountedReader<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(position)
    }
}

fn values(x: u32, y: u32, maximum: u32) -> [u32; 4] {
    [
        (x * 137 + y) % (maximum + 1),
        (y * 97 + x) % (maximum + 1),
        maximum,
        if (x + y).is_multiple_of(5) { 0 } else { maximum / 2 },
    ]
}
#[test]
fn sparse_decode_groups_original_samples_by_chunk_for_both_integer_precisions()
-> harness::TestResult {
    harness::run(4 * 1024 * 1024, |context| {
        for packing in [Packing::Rgba8, Packing::Rgba16] {
            let descriptor = RasterDescriptor::new(
                Extent::new(513, 259)?,
                packing,
                Transfer::Gamma(1.0),
                [7; 32],
            )?;
            let maximum = if packing == Packing::Rgba8 { 255 } else { 65535 };
            let mut bytes = Vec::new();
            write_raster(&mut bytes, descriptor, context, |y, row| {
                for (x, pixel) in row.chunks_exact_mut(packing.bytes() as usize).enumerate() {
                    for (channel, value) in values(x as u32, y, maximum).into_iter().enumerate() {
                        if packing == Packing::Rgba8 {
                            pixel[channel] = value as u8;
                        } else {
                            pixel[channel * 2..channel * 2 + 2]
                                .copy_from_slice(&(value as u16).to_be_bytes());
                        }
                    }
                }
                Ok(())
            })?;
            let reads = Cell::new(0);
            let reader = CountedReader { bytes: Cursor::new(bytes), reads: &reads };
            let mut source = RasterArchive::open(reader, descriptor, context.memory(), context)?;
            let samples = RasterSamples::new(
                descriptor.layout().extent(),
                Region::new(0, 0, 96, 81)?,
                Sampling::new([7.75, 4.25], [-60.25, -20.5])?.with_filter(Filter::Nearest),
            )?;
            let mut output = vec![Pixel::TRANSPARENT; samples.region().pixels() as usize];
            source.read_samples(descriptor.asset(), &samples, &mut output, context)?;
            for (y, &sy) in samples.rows().iter().enumerate() {
                for (x, &sx) in samples.columns().iter().enumerate() {
                    let rgba = values(sx, sy, maximum);
                    let expected = Pixel::linear(
                        (f64::from(rgba[0]) / f64::from(maximum)) as f32,
                        (f64::from(rgba[1]) / f64::from(maximum)) as f32,
                        1.0,
                        rgba[3] as f32 / maximum as f32,
                    )?;
                    assert_eq!(output[y * samples.columns().len() + x], expected);
                }
            }
            assert_eq!(source.stats().chunks, 15);
            assert_eq!(reads.get(), 16);
            assert_eq!(source.stats().read_bytes + 128, descriptor.layout().length());
        }
        Ok(())
    })
}

#[test]
fn sparse_decode_rejects_corruption_outside_the_selected_pixel_within_its_chunk()
-> harness::TestResult {
    harness::run(1024 * 1024, |context| {
        let descriptor =
            RasterDescriptor::new(Extent::new(2, 1)?, Packing::Rgba8, Transfer::Srgb, [8; 32])?;
        let mut bytes = Vec::new();
        write_raster(&mut bytes, descriptor, context, |_, row| {
            row.fill(255);
            Ok(())
        })?;
        bytes[160] ^= 1;
        let mut source =
            RasterArchive::open(Cursor::new(bytes), descriptor, context.memory(), context)?;
        let samples = RasterSamples::new(
            descriptor.layout().extent(),
            Region::new(0, 0, 1, 1)?,
            Sampling::new([100.0, 1.0], [0.0, 0.0])?.with_filter(Filter::Nearest),
        )?;
        assert_eq!(samples.columns(), &[1]);
        assert!(
            source
                .read_samples(descriptor.asset(), &samples, &mut [Pixel::TRANSPARENT], context)
                .is_err()
        );
        Ok(())
    })
}
