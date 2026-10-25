// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Independently verify packed layout, source integrity and bounded import memory.
use super::*;
use std::io::{self, Cursor, Write};
use sugarsubstitute_image_ops::{Extent, Pixel, RasterSource, Region};
#[cfg(windows)]
mod cached;
mod harness;
mod png;
mod samples;

#[test]
fn adaptive_chunks_cover_exact_file_offsets_without_unbounded_stripe_memory()
-> Result<(), ImportError> {
    for packing in [Packing::Rgba8, Packing::Rgba16] {
        let layout = ChunkLayout::new(Extent::new(257, 131)?, packing)?;
        let mut offset = 128;
        for stripe in 0..layout.stripes() {
            for column in 0..layout.columns() {
                assert_eq!(layout.offset(column, stripe)?, offset);
                offset += 32 + layout.region(column, stripe)?.pixels() * u64::from(packing.bytes());
            }
        }
        assert_eq!(offset, layout.length());
        assert!(layout.region(layout.columns(), 0).is_err());
        assert!(layout.offset(0, layout.stripes()).is_err());
    }
    let wide = ChunkLayout::new(Extent::new(1_000_000, 1000)?, Packing::Rgba8)?;
    assert_eq!(wide.rows(), 2);
    assert!(wide.stripe_bytes() <= 8 * 1024 * 1024);
    assert!(ChunkLayout::new(Extent::new(1_000_000_000, 1_000_000_000)?, Packing::Rgba8).is_err());
    Ok(())
}

#[test]
fn archive_regions_cross_chunk_seams_with_independent_linear_alpha_reference() -> harness::TestResult
{
    harness::run(2 * 1024 * 1024, |context| {
        let descriptor =
            RasterDescriptor::new(Extent::new(257, 131)?, Packing::Rgba8, Transfer::Srgb, [3; 32])?;
        let mut bytes = Vec::new();
        write_raster(&mut bytes, descriptor, context, |y, row| {
            for (x, rgba) in row.chunks_exact_mut(4).enumerate() {
                rgba.copy_from_slice(&[x as u8, y as u8, 128, 128]);
            }
            Ok(())
        })?;
        assert_eq!(bytes.len() as u64, descriptor.layout().length());
        let mut source =
            RasterArchive::open(Cursor::new(bytes), descriptor, context.memory(), context)?;
        let region = Region::new(127, 125, 128, 6)?;
        let mut output = vec![Pixel::TRANSPARENT; region.pixels() as usize];
        source.read(descriptor.asset(), region, &mut output, context)?;
        let decode = |value: u8| {
            let encoded = f64::from(value) / 255.0;
            (if encoded <= 0.04045 {
                encoded / 12.92
            } else {
                ((encoded + 0.055) / 1.055).powf(2.4)
            }) as f32
        };
        for y in region.y()..region.bottom() {
            for x in region.x()..region.right() {
                let expected =
                    Pixel::linear(decode(x as u8), decode(y as u8), decode(128), 128.0 / 255.0)?;
                let actual = output[((y - region.y()) * region.width() + x - region.x()) as usize];
                assert_eq!(actual, expected);
            }
        }
        assert_eq!(source.stats().chunks, 4);
        assert!(source.stats().read_bytes < descriptor.layout().length());
        assert!(
            source
                .read(sugarsubstitute_image_ops::AssetId([9; 32]), region, &mut output, context)
                .is_err()
        );
        Ok(())
    })
}

#[test]
fn corrupt_header_truncated_archive_and_corrupt_chunk_are_distinct_rejections()
-> harness::TestResult {
    harness::run(2 * 1024 * 1024, |context| {
        let descriptor =
            RasterDescriptor::new(Extent::new(2, 1)?, Packing::Rgba8, Transfer::Srgb, [3; 32])?;
        let mut bytes = Vec::new();
        write_raster(&mut bytes, descriptor, context, |_, row| {
            row.fill(255);
            Ok(())
        })?;
        let mut header = bytes.clone();
        header[96] ^= 1;
        assert!(
            RasterArchive::open(Cursor::new(header), descriptor, context.memory(), context)
                .is_err()
        );
        assert!(
            RasterArchive::open(
                Cursor::new(bytes[..bytes.len() - 1].to_vec()),
                descriptor,
                context.memory(),
                context
            )
            .is_err()
        );
        bytes[160] ^= 1;
        let mut source =
            RasterArchive::open(Cursor::new(bytes), descriptor, context.memory(), context)?;
        let mut output = [Pixel::TRANSPARENT; 2];
        let failure = source.read(
            descriptor.asset(),
            descriptor.layout().extent().region(),
            &mut output,
            context,
        );
        assert!(failure.is_err());
        Ok(())
    })
}

#[test]
fn shared_source_keeps_original_admission_until_its_last_reader_clone_retires()
-> harness::TestResult {
    harness::run(2 * 1024 * 1024, |context| {
        let descriptor =
            RasterDescriptor::new(Extent::new(2, 1)?, Packing::Rgba8, Transfer::Srgb, [7; 32])?;
        let mut bytes = Vec::new();
        write_raster(&mut bytes, descriptor, context, |_, row| {
            row.fill(255);
            Ok(())
        })?;
        let source =
            RasterArchive::open(Cursor::new(bytes), descriptor, context.memory(), context)?;
        let first = SharedRaster::new(source, context.memory())?;
        let mut second = first.clone();
        drop(first);
        assert!(context.memory().claim(context.memory().bytes()).is_err());
        assert_eq!(second.descriptor(), descriptor);
        let mut pixels = [Pixel::TRANSPARENT; 2];
        second.read(
            descriptor.asset(),
            descriptor.layout().extent().region(),
            &mut pixels,
            context,
        )?;
        assert_eq!(pixels, [Pixel::linear(1.0, 1.0, 1.0, 1.0)?; 2]);
        drop(second);
        let full = context.memory().claim(context.memory().bytes())?;
        drop(full);
        Ok(())
    })
}

struct Count(u64);
impl Write for Count {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 += bytes.len() as u64;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn million_row_source_import_uses_a_sixty_four_kibibyte_original_reservation() -> harness::TestResult
{
    harness::run(64 * 1024, |context| {
        let descriptor = RasterDescriptor::new(
            Extent::new(1, 1_000_000)?,
            Packing::Rgba8,
            Transfer::Srgb,
            [1; 32],
        )?;
        let mut output = Count(0);
        write_raster(&mut output, descriptor, context, |y, row| {
            row.copy_from_slice(&[y as u8, 0, 0, 255]);
            Ok(())
        })?;
        assert_eq!(output.0, descriptor.layout().length());
        let full = context.memory().claim(context.memory().bytes())?;
        drop(full);
        Ok(())
    })
}
