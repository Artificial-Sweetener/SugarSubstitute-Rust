// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! PNG sample precision and integrity use real decoding and original execution admission.
use super::*;
use sha2::{Digest, Sha256};

fn encode(
    color: ::png::ColorType,
    depth: ::png::BitDepth,
    data: &[u8],
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let mut bytes = Vec::new();
    {
        let mut encoder = ::png::Encoder::new(&mut bytes, 2, 1);
        encoder.set_color(color);
        encoder.set_depth(depth);
        encoder.write_header()?.write_image_data(data)?;
    }
    Ok(bytes)
}
fn roundtrip(bytes: Vec<u8>, expected: [[f32; 4]; 2]) -> harness::TestResult {
    harness::run(32 * 1024 * 1024, move |context| {
        let import = PngImport::open(Cursor::new(&bytes), Sha256::digest(&bytes).into(), context)?;
        let descriptor = import.descriptor();
        let mut archive = Vec::new();
        import.write(&mut archive, context)?;
        let mut source =
            RasterArchive::open(Cursor::new(archive), descriptor, context.memory(), context)?;
        let mut pixels = [Pixel::TRANSPARENT; 2];
        source.read(
            descriptor.asset(),
            descriptor.layout().extent().region(),
            &mut pixels,
            context,
        )?;
        for (pixel, expected) in pixels.iter().zip(expected) {
            for (actual, expected) in pixel.channels().into_iter().zip(expected) {
                assert!((actual - expected).abs() < 1e-6, "actual={actual}, expected={expected}");
            }
        }
        Ok(())
    })
}
#[test]
fn rgba_png_uses_straight_color_then_linear_premultiplication() -> harness::TestResult {
    roundtrip(
        encode(::png::ColorType::Rgba, ::png::BitDepth::Eight, &[255, 0, 0, 128, 255, 128, 64, 0])?,
        [[128.0 / 255.0, 0.0, 0.0, 128.0 / 255.0], [0.0; 4]],
    )
}
#[test]
fn grayscale_png_expands_rgb_without_applying_transfer_to_coverage() -> harness::TestResult {
    roundtrip(
        encode(::png::ColorType::GrayscaleAlpha, ::png::BitDepth::Eight, &[255, 128, 0, 255])?,
        [[128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0], [0.0, 0.0, 0.0, 1.0]],
    )
}
#[test]
fn sixteen_bit_png_keeps_big_endian_sample_and_alpha_precision() -> harness::TestResult {
    roundtrip(
        encode(
            ::png::ColorType::Rgba,
            ::png::BitDepth::Sixteen,
            &[255, 255, 0, 0, 0, 0, 128, 1, 0, 0, 255, 255, 0, 0, 255, 255],
        )?,
        [[32769.0 / 65535.0, 0.0, 0.0, 32769.0 / 65535.0], [0.0, 1.0, 0.0, 1.0]],
    )
}
#[test]
fn truncated_png_cannot_produce_a_successful_complete_import() -> harness::TestResult {
    let mut bytes = encode(::png::ColorType::Rgb, ::png::BitDepth::Eight, &[255, 0, 0, 0, 255, 0])?;
    bytes.truncate(bytes.len() - 8);
    harness::run(32 * 1024 * 1024, move |context| {
        let result = (|| {
            let import =
                PngImport::open(Cursor::new(&bytes), Sha256::digest(&bytes).into(), context)?;
            import.write(&mut Vec::new(), context)
        })();
        assert!(result.is_err());
        Ok(())
    })
}

#[test]
fn palette_transparency_and_subbyte_indices_expand_without_losing_alpha() -> harness::TestResult {
    let mut bytes = Vec::new();
    {
        let mut encoder = ::png::Encoder::new(&mut bytes, 2, 1);
        encoder.set_color(::png::ColorType::Indexed);
        encoder.set_depth(::png::BitDepth::One);
        encoder.set_palette(vec![255, 0, 0, 0, 0, 255]);
        encoder.set_trns(vec![0, 128]);
        encoder.write_header()?.write_image_data(&[0b01000000])?;
    }
    roundtrip(bytes, [[0.0; 4], [0.0, 0.0, 128.0 / 255.0, 128.0 / 255.0]])
}

#[test]
fn tagged_gamma_decodes_into_linear_light_before_canonical_source_reads() -> harness::TestResult {
    let mut bytes = Vec::new();
    {
        let mut encoder = ::png::Encoder::new(&mut bytes, 2, 1);
        encoder.set_color(::png::ColorType::Grayscale);
        encoder.set_source_gamma(::png::ScaledFloat::new(0.5));
        encoder.write_header()?.write_image_data(&[128, 255])?;
    }
    let gray = (128.0_f32 / 255.0).powi(2);
    roundtrip(bytes, [[gray, gray, gray, 1.0], [1.0; 4]])
}
