// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise peer matching and inert admission before native output effects.

use super::*;
use crate::output_media::ComfyOutputFormat;
use sugarsubstitute_contracts::canvas_surface::CanvasFileReference;

mod native_http;

fn peer() -> Result<BackendPeer, Box<dyn Error>> {
    Ok(BackendPeer {
        process_id: std::num::NonZeroU32::new(47).ok_or("zero PID")?,
        creation_time_100ns: std::num::NonZeroU64::new(53).ok_or("zero process birth")?,
    })
}

#[test]
fn connected_peer_must_match_both_parent_granted_process_coordinates() -> Result<(), Box<dyn Error>>
{
    let expected = peer()?;
    assert!(native::matches_peer(47, 53, expected));
    assert!(!native::matches_peer(48, 53, expected));
    assert!(!native::matches_peer(47, 54, expected));
    assert!(!native::matches_peer(0, 0, expected));
    Ok(())
}

#[test]
fn transfer_rejects_a_limit_smaller_than_its_format_before_any_native_effect()
-> Result<(), Box<dyn Error>> {
    let origin = LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?;
    for (filename, minimum) in [("image.png", 33), ("image.jpg", 27), ("image.jpeg", 27)] {
        let reference = CanvasFileReference::decode(
            format!(r#"{{"filename":"{filename}","subfolder":"","type":"output"}}"#).as_bytes(),
        )?;
        let target = ComfyViewTarget::new(&reference)?;
        assert!(ComfyOutputTransfer::new(origin, target.clone(), peer()?, minimum - 1).is_err());
        let transfer = ComfyOutputTransfer::new(origin, target, peer()?, minimum)?;
        assert!(!transfer.settled());
    }
    Ok(())
}

/// Copy `bytes` as a declared `length` body of `format` through the bounded media copier.
fn copy(format: ComfyOutputFormat, length: u64, bytes: &[u8]) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut input = std::io::Cursor::new(bytes);
    let mut output = Vec::new();
    native::copy_media(format, length, &mut output, |buffer| input.read(buffer))?;
    Ok(output)
}

#[test]
fn output_stream_rejects_truncation_excess_and_non_png_bytes() -> Result<(), Box<dyn Error>> {
    let mut valid = b"\x89PNG\r\n\x1a\n".to_vec();
    valid.extend([0x42; 32]);
    assert_eq!(copy(ComfyOutputFormat::Png, valid.len() as u64, &valid)?, valid);
    let mut jpeg = vec![0xff, 0xd8, 0xff, 0xe0];
    jpeg.extend([0x42; 36]);
    for (length, bytes) in [
        (valid.len() as u64 + 1, valid.clone()),
        (valid.len() as u64 - 1, valid.clone()),
        (valid.len() as u64, vec![0x42; valid.len()]),
        (jpeg.len() as u64, jpeg),
    ] {
        assert!(copy(ComfyOutputFormat::Png, length, &bytes).is_err());
    }
    let mut output = Vec::new();
    assert!(
        native::copy_media(ComfyOutputFormat::Png, u64::MAX, &mut output, |buffer| Ok(buffer
            .len()
            + 1))
        .is_err()
    );
    Ok(())
}

#[test]
fn output_stream_requires_the_jpeg_signature_and_exact_declared_length()
-> Result<(), Box<dyn Error>> {
    let valid = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../sugarsubstitute-image-io/src/tests/jpeg/fixtures/444.jpg"
    ));
    assert_eq!(copy(ComfyOutputFormat::Jpeg, valid.len() as u64, valid)?, valid);
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend([0x42; 32]);
    let mut marker_only = valid.to_vec();
    marker_only[2] = 0x00;
    for (length, bytes) in [
        (valid.len() as u64 + 1, valid.to_vec()),
        (valid.len() as u64 - 1, valid.to_vec()),
        (valid.len() as u64, marker_only),
        (png.len() as u64, png),
        (2, vec![0xff, 0xd8]),
    ] {
        assert!(copy(ComfyOutputFormat::Jpeg, length, &bytes).is_err(), "length={length}");
    }
    Ok(())
}
