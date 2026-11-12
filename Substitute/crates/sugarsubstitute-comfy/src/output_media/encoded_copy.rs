// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify exact bounded original streams before either network or retained-source cache publication.

use super::{ComfyOutputFormat, OutputArtifactIdentity};
use sha2::{Digest, Sha256};
use std::io::{self, Write};
/// Authenticate exact original bytes before the containing entry can become visible.
/// Partial writes remain unpublished and are retired by the cache entry owner on error.
pub(super) fn copy_media(
    format: ComfyOutputFormat,
    length: u64,
    expected: Option<OutputArtifactIdentity>,
    output: &mut dyn Write,
    mut read: impl FnMut(&mut [u8]) -> io::Result<usize>,
) -> io::Result<()> {
    let mut digest = Sha256::new();
    let mut scratch = [0u8; 32 * 1024];
    let mut signature = [0u8; 16];
    let mut total = 0u64;
    loop {
        let count = read(&mut scratch)?;
        if count == 0 {
            break;
        }
        if count > scratch.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Comfy output reader returned an invalid byte count",
            ));
        }
        let next =
            total.checked_add(count as u64).filter(|value| *value <= length).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Comfy output exceeded its declared length",
                )
            })?;
        if total < signature.len() as u64 {
            let start = total as usize;
            let copied = count.min(signature.len() - start);
            signature[start..start + copied].copy_from_slice(&scratch[..copied]);
        }
        digest.update(&scratch[..count]);
        output.write_all(&scratch[..count])?;
        total = next;
    }
    if total != length
        || !format.matches_prefix(&signature[..(total as usize).min(signature.len())])
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Comfy output is truncated or does not match its format",
        ));
    }
    let actual: [u8; 32] = digest.finalize().into();
    if expected.is_some_and(|expected| expected.bytes() != total || expected.digest() != &actual) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Comfy output differs from its recorded identity",
        ));
    }
    Ok(())
}
