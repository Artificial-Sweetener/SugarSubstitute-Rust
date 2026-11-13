// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify transient animation routing on exact-reader header facts rather than filename guesses.

use super::*;
use std::io::{self, Cursor, Seek};

fn png_chunk(target: &mut Vec<u8>, kind: &[u8; 4], payload: &[u8]) {
    target.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    target.extend_from_slice(kind);
    target.extend_from_slice(payload);
    target.extend_from_slice(&[0; 4]);
}

#[test]
fn animation_probes_distinguish_webp_headers_and_preserve_reader_position()
-> Result<(), Box<dyn Error>> {
    for (format, bytes, animated) in [
        (ComfyOutputFormat::Gif, b"GIF89a12345678".to_vec(), true),
        (ComfyOutputFormat::Jpeg, vec![0xff, 0xd8, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0], false),
        (ComfyOutputFormat::Mp4, b"\0\0\0\x18ftypisom".to_vec(), false),
        (ComfyOutputFormat::Webm, b"\x1a\x45\xdf\xa3abcdefgh".to_vec(), false),
        (ComfyOutputFormat::Webp, b"RIFF1234WEBPVP8X\x0a\0\0\0\x02".to_vec(), true),
        (ComfyOutputFormat::Webp, b"RIFF1234WEBPVP8X\x0a\0\0\0\x00".to_vec(), false),
        (ComfyOutputFormat::Webp, b"RIFF1234WEBPVP8L\x01\0\0\0\x02".to_vec(), false),
    ] {
        let mut reader = Cursor::new(bytes);
        reader.set_position(2);
        assert_eq!(probe_output_animation(&mut reader, format, || Ok(()))?, animated);
        assert_eq!(reader.stream_position()?, 2);
    }
    Ok(())
}

#[test]
fn png_animation_uses_actl_after_metadata_and_stops_before_large_pixel_payload()
-> Result<(), Box<dyn Error>> {
    let mut header = b"\x89PNG\r\n\x1a\n".to_vec();
    png_chunk(&mut header, b"IHDR", &[0; 13]);
    png_chunk(&mut header, b"tEXt", &[b'x'; 128]);
    png_chunk(&mut header, b"acTL", &[0, 0, 0, 2, 0, 0, 0, 0]);
    let mut reader = Cursor::new(header);
    assert!(probe_output_animation(&mut reader, ComfyOutputFormat::Png, || Ok(()))?);
    let mut still = b"\x89PNG\r\n\x1a\n".to_vec();
    png_chunk(&mut still, b"IHDR", &[0; 13]);
    still.extend_from_slice(&100_000_000u32.to_be_bytes());
    still.extend_from_slice(b"IDAT");
    let mut reader = Cursor::new(still);
    assert!(!probe_output_animation(&mut reader, ComfyOutputFormat::Png, || Ok(()))?);
    Ok(())
}

#[test]
fn animation_probe_rejects_bad_metadata_and_observes_cancellation() -> Result<(), Box<dyn Error>> {
    let mut oversized = b"\x89PNG\r\n\x1a\n".to_vec();
    oversized.extend_from_slice(&u32::MAX.to_be_bytes());
    oversized.extend_from_slice(b"tEXt");
    assert!(
        probe_output_animation(&mut Cursor::new(oversized), ComfyOutputFormat::Png, || Ok(()))
            .is_err()
    );
    let mut reader = Cursor::new(b"GIF89a12345678");
    let error = probe_output_animation(&mut reader, ComfyOutputFormat::Gif, || {
        Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"))
    });
    assert!(error.is_err());
    assert_eq!(reader.stream_position()?, 0);
    Ok(())
}
