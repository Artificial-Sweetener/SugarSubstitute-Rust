// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise peer matching and inert admission before native output effects.

use super::*;
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
fn transfer_rejects_a_limit_smaller_than_a_png_before_any_native_effect()
-> Result<(), Box<dyn Error>> {
    let reference =
        CanvasFileReference::decode(br#"{"filename":"image.png","subfolder":"","type":"output"}"#)?;
    let target = ComfyViewTarget::new(&reference)?;
    let origin = LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?;
    assert!(ComfyOutputTransfer::new(origin, target.clone(), peer()?, 32).is_err());
    let transfer = ComfyOutputTransfer::new(origin, target, peer()?, 33)?;
    assert!(!transfer.settled());
    Ok(())
}

#[test]
fn output_stream_rejects_truncation_excess_and_non_png_bytes() -> Result<(), Box<dyn Error>> {
    use std::io::Read;
    let mut valid = b"\x89PNG\r\n\x1a\n".to_vec();
    valid.extend([0x42; 32]);
    let mut output = Vec::new();
    let mut input = std::io::Cursor::new(&valid);
    native::copy_png(valid.len() as u64, &mut output, |buffer| input.read(buffer))?;
    assert_eq!(output, valid);
    for (length, bytes) in [
        (valid.len() as u64 + 1, valid.clone()),
        (valid.len() as u64 - 1, valid.clone()),
        (valid.len() as u64, vec![0x42; valid.len()]),
    ] {
        let mut input = std::io::Cursor::new(bytes);
        let mut output = Vec::new();
        assert!(native::copy_png(length, &mut output, |buffer| input.read(buffer)).is_err());
    }
    let mut output = Vec::new();
    assert!(native::copy_png(u64::MAX, &mut output, |buffer| Ok(buffer.len() + 1)).is_err());
    Ok(())
}
