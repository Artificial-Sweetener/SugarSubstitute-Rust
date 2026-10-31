// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual Pillow quality-95 JPEG samples and independent bounded source rejection.
use super::*;
use sha2::{Digest, Sha256};
mod arithmetic;
mod controls;
mod storage;

// Generated with ComfyUI's installed Pillow: quality=95, compress_level=1, 19x17 flat
// RGB(128,64,32) or L(128). Sampling is explicit; RGB's Adobe APP14 was removed so its
// ASCII component identities, rather than unsupported metadata, define interpretation.
pub(super) const FIXTURES: [&[u8]; 5] = [
    include_bytes!("jpeg/fixtures/gray.jpg"),
    include_bytes!("jpeg/fixtures/444.jpg"),
    include_bytes!("jpeg/fixtures/422.jpg"),
    include_bytes!("jpeg/fixtures/420.jpg"),
    include_bytes!("jpeg/fixtures/rgb.jpg"),
];

#[test]
fn jpeg_gradient_row_order_and_subsampled_edges_match_independent_pillow_reference()
-> harness::TestResult {
    harness::run(2 * 1024 * 1024, |context| {
        let bytes = include_bytes!("jpeg/fixtures/gradient-420.jpg");
        // Same encoder options; the RGB reference is decoded independently by installed Pillow.
        let expected = include_bytes!("jpeg/fixtures/gradient-reference.rgb");
        let import = JpegImport::open(Cursor::new(bytes), Sha256::digest(bytes).into(), context)?;
        let descriptor = import.descriptor();
        assert_eq!(descriptor.layout().extent(), Extent::new(37, 29)?);
        let mut archive = Vec::new();
        import.write(&mut archive, context)?;
        let mut source =
            RasterArchive::open(Cursor::new(archive), descriptor, context.memory(), context)?;
        let mut pixels = vec![Pixel::TRANSPARENT; 37 * 29];
        source.read(
            descriptor.asset(),
            descriptor.layout().extent().region(),
            &mut pixels,
            context,
        )?;
        for (index, (pixel, reference)) in pixels.iter().zip(expected.chunks_exact(3)).enumerate() {
            for (channel, value) in pixel.channels()[..3].iter().zip(reference) {
                let low = Transfer::Srgb.linear(f64::from(value.saturating_sub(3)) / 255.0);
                let high = Transfer::Srgb.linear(f64::from(value.saturating_add(3)) / 255.0);
                assert!(
                    *channel >= low - 1e-6 && *channel <= high + 1e-6,
                    "pixel={index}, actual={channel}, reference={value}"
                );
            }
        }
        Ok(())
    })
}

/// Locate an existing header marker in these controlled fixtures.
pub(super) fn marker(bytes: &[u8], value: u8) -> Result<usize, ImportError> {
    bytes
        .windows(2)
        .position(|part| part == [255, value])
        .ok_or(ImportError::Invalid("JPEG test marker is absent"))
}

#[test]
fn jpeg_samples_keep_odd_extent_opaque_alpha_and_linear_srgb_interpretation() -> harness::TestResult
{
    harness::run(2 * 1024 * 1024, |context| {
        for (index, bytes) in FIXTURES.into_iter().enumerate() {
            let hash = Sha256::digest(bytes).into();
            let import = JpegImport::open(Cursor::new(bytes), hash, context)?;
            let descriptor = import.descriptor();
            assert_eq!(descriptor.original_hash(), hash);
            assert_eq!(descriptor.layout().extent(), Extent::new(19, 17)?);
            assert_eq!(descriptor.layout().packing(), Packing::Rgba8);
            assert_eq!(descriptor.transfer(), Transfer::Srgb);
            let mut archive = Vec::new();
            import.write(&mut archive, context)?;
            let mut source =
                RasterArchive::open(Cursor::new(archive), descriptor, context.memory(), context)?;
            let mut pixels = vec![Pixel::TRANSPARENT; 19 * 17];
            source.read(
                descriptor.asset(),
                descriptor.layout().extent().region(),
                &mut pixels,
                context,
            )?;
            let expected = if index == 0 { [128, 128, 128] } else { [128, 64, 32] };
            for pixel in pixels {
                let channels = pixel.channels();
                for channel in 0..3 {
                    let expected = Transfer::Srgb.linear(f64::from(expected[channel]) / 255.0);
                    assert!(
                        (channels[channel] - expected).abs() < 0.012,
                        "fixture={index}, channel={channel}, actual={}",
                        channels[channel]
                    );
                }
                assert_eq!(channels[3], 1.0);
            }
        }
        Ok(())
    })
}

#[test]
fn jpeg_preflight_rejects_unsupported_metadata_precision_sampling_and_scans() -> harness::TestResult
{
    harness::run(1024 * 1024, |context| {
        let source = FIXTURES[3];
        let mut rejected = vec![include_bytes!("jpeg/fixtures/progressive.jpg").to_vec()];
        for app in [0xe1, 0xe2, 0xed, 0xee, 0xfe] {
            let mut bytes = source.to_vec();
            bytes.splice(2..2, [255, app, 0, 2]);
            rejected.push(bytes);
        }
        let sof = marker(source, 0xc0)?;
        let sos = marker(source, 0xda)?;
        for (offset, value) in [
            (sof + 4, 12),
            (sof + 6, 0),
            (sof + 11, 0x31),
            (sof + 14, 0x21),
            (sof + 10, 9),
            (sos + 4, 1),
            (sos + 5, 9),
            (sos + 6, 0x33),
            (sos + 11, 1),
        ] {
            let mut bytes = source.to_vec();
            bytes[offset] = value;
            rejected.push(bytes);
        }
        // Replace both height bytes together to exercise the zero-extent rejection.
        let mut zero = source.to_vec();
        zero[sof + 5..sof + 7].fill(0);
        rejected.push(zero);
        for bytes in rejected {
            assert!(
                JpegImport::open(Cursor::new(&bytes), Sha256::digest(&bytes).into(), context)
                    .is_err()
            );
        }
        Ok(())
    })
}

#[test]
fn jpeg_every_truncated_prefix_trailing_bytes_and_second_scan_reject_before_decode()
-> harness::TestResult {
    harness::run(1024 * 1024, |context| {
        let source = FIXTURES[3];
        for length in 0..source.len() {
            assert!(
                JpegImport::open(Cursor::new(&source[..length]), [0; 32], context).is_err(),
                "prefix={length}"
            );
        }
        let mut trailing = source.to_vec();
        trailing.push(0);
        assert!(JpegImport::open(Cursor::new(trailing), [0; 32], context).is_err());
        let mut scan = source.to_vec();
        let end = scan.len() - 2;
        scan.splice(end..end, [255, 218, 0, 2]);
        assert!(JpegImport::open(Cursor::new(scan), [0; 32], context).is_err());
        Ok(())
    })
}

#[test]
fn jpeg_invalid_tables_reject_before_decoder_allocation() -> harness::TestResult {
    harness::run(1024 * 1024, |context| {
        let source = FIXTURES[3];
        let dqt = marker(source, 0xdb)?;
        let dht = marker(source, 0xc4)?;
        for (offset, value) in [(dqt + 4, 0x10), (dqt + 5, 0), (dht + 4, 0x20), (dht + 5, 255)] {
            let mut bytes = source.to_vec();
            bytes[offset] = value;
            assert!(JpegImport::open(Cursor::new(bytes), [0; 32], context).is_err());
        }
        Ok(())
    })
}
