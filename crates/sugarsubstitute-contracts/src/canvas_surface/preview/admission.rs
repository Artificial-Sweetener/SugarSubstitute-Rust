// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Enforce shared preview attribution and canonical bounded byte admission.

use super::{MAX_SURFACE_PREVIEW_CHUNK_BYTES, wire};

/// Check the shared client's attribution grammar without asserting actual workflow authority.
pub(super) fn valid_identity(value: &wire::CanvasSurfacePreviewIdentity) -> bool {
    valid_attribution(&value.prompt_id, "prompt:")
        && valid_attribution(&value.source_id, "source:")
        && value.token == format!("preview:{}", value.sequence)
}

/// Require a nonempty suffix and preserve the shared client's control-character rejection.
fn valid_attribution(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|tail| !tail.is_empty())
        && !value.chars().any(char::is_control)
}

/// Bound decoded allocation before decoding a chunk and reject noncanonical pad bits.
pub(super) fn decode_chunk(value: &str) -> Option<Vec<u8>> {
    let encoded = value.as_bytes();
    if encoded.is_empty() || !encoded.len().is_multiple_of(4) {
        return None;
    }
    let padding = if value.ends_with("==") { 2 } else { usize::from(value.ends_with('=')) };
    let length = encoded.len().checked_div(4)?.checked_mul(3)?.checked_sub(padding)?;
    if length == 0 || length > MAX_SURFACE_PREVIEW_CHUNK_BYTES {
        return None;
    }
    let mut decoded = Vec::with_capacity(length);
    for (index, chunk) in encoded.chunks_exact(4).enumerate() {
        let a = sextet(chunk[0])?;
        let b = sextet(chunk[1])?;
        let last = index + 1 == encoded.len() / 4;
        decoded.push((a << 2) | (b >> 4));
        if last && padding == 2 {
            if b & 15 != 0 || chunk[2] != b'=' || chunk[3] != b'=' {
                return None;
            }
            continue;
        }
        let c = sextet(chunk[2])?;
        decoded.push((b << 4) | (c >> 2));
        if last && padding == 1 {
            if c & 3 != 0 || chunk[3] != b'=' {
                return None;
            }
            continue;
        }
        decoded.push((c << 6) | sextet(chunk[3])?);
    }
    Some(decoded)
}

/// Translate the standard RFC 4648 alphabet without accepting URL-safe alternatives.
fn sextet(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}
