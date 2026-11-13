// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise peer matching and inert admission before native output effects.

use super::*;
use crate::output_media::ComfyOutputFormat;
use crate::output_media::encoded_copy::copy_media;
use sugarsubstitute_contracts::canvas_surface::CanvasFileReference;

mod native_http;
mod retained;

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
    copy_media(format, length, None, &mut output, |buffer| input.read(buffer))?;
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
        copy_media(ComfyOutputFormat::Png, u64::MAX, None, &mut output, |buffer| Ok(
            buffer.len() + 1
        ))
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

#[test]
fn verified_output_stream_rejects_changed_bytes_before_publication() -> Result<(), Box<dyn Error>> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut valid = b"\x89PNG\r\n\x1a\n".to_vec();
    valid.extend([0x42; 32]);
    let hash = format!("{:x}", Sha256::digest(&valid));
    let expected = OutputArtifactIdentity::new(&hash, valid.len() as u64)?;
    let mut output = Vec::new();
    let mut position = 0;
    copy_media(ComfyOutputFormat::Png, expected.bytes(), Some(expected), &mut output, |buffer| {
        let count = (valid.len() - position).min(3).min(buffer.len());
        buffer[..count].copy_from_slice(&valid[position..position + count]);
        position += count;
        Ok(count)
    })?;
    assert_eq!(output, valid);
    valid[20] ^= 1;
    let mut changed = std::io::Cursor::new(valid);
    let mut unpublished = Vec::new();
    assert!(
        copy_media(
            ComfyOutputFormat::Png,
            expected.bytes(),
            Some(expected),
            &mut unpublished,
            |buffer| changed.read(buffer)
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn verified_transfer_rejects_receipt_lengths_outside_native_source_admission()
-> Result<(), Box<dyn Error>> {
    let origin = LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?;
    let reference =
        CanvasFileReference::decode(br#"{"filename":"image.png","subfolder":"","type":"output"}"#)?;
    let target = ComfyViewTarget::new(&reference)?;
    for bytes in [1, 1025] {
        let expected = OutputArtifactIdentity::new(&"0".repeat(64), bytes)?;
        assert!(
            ComfyOutputTransfer::verified(origin, target.clone(), peer()?, 1024, expected).is_err()
        );
    }
    let expected = OutputArtifactIdentity::new(&"0".repeat(64), 40)?;
    let transfer = ComfyOutputTransfer::verified(origin, target, peer()?, 1024, expected)?;
    assert!(!transfer.settled());
    Ok(())
}
#[test]
fn generic_verified_bodies_preserve_encoded_browser_media_and_reject_changed_signatures()
-> Result<(), Box<dyn Error>> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    for (format, prefix) in [
        (ComfyOutputFormat::Gif, b"GIF89a1234567".as_slice()),
        (ComfyOutputFormat::Webp, b"RIFF1234WEBP".as_slice()),
        (ComfyOutputFormat::Mp4, b"\0\0\0\x18ftypisom".as_slice()),
        (ComfyOutputFormat::Webm, b"\x1a\x45\xdf\xa3abcdefgh".as_slice()),
    ] {
        let mut encoded = prefix.to_vec();
        encoded.extend([0x42; 32]);
        let expected = OutputArtifactIdentity::new(
            &format!("{:x}", Sha256::digest(&encoded)),
            encoded.len() as u64,
        )?;
        let mut output = Vec::new();
        let mut input = std::io::Cursor::new(encoded.clone());
        copy_media(format, expected.bytes(), Some(expected), &mut output, |bytes| {
            input.read(bytes)
        })?;
        assert_eq!(output, encoded);
        encoded[0] ^= 1;
        let mut input = std::io::Cursor::new(encoded);
        assert!(
            copy_media(format, expected.bytes(), Some(expected), &mut Vec::new(), |bytes| input
                .read(bytes))
            .is_err()
        );
    }
    Ok(())
}
