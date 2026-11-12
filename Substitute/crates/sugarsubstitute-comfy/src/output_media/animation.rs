// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Inspect animation routing facts on the already verified original reader without decoding pixels.

use super::ComfyOutputFormat;
use std::{
    error::Error,
    fmt,
    io::{self, Read, Seek, SeekFrom},
};

const MAX_HEADER_BYTES: u64 = 64 * 1024 * 1024;
const MAX_HEADER_CHUNKS: usize = 10000;

/// Probe animation routing while retaining the caller's verified reader and restoring its position.
/// GIF follows the existing animated presenter even for one frame; video uses its separate presenter.
/// WebP reads the `VP8X` animation bit; PNG scans bounded pre-IDAT metadata for `acTL`.
/// `check` must observe the owning storage job's cancellation/deadline before each bounded I/O unit.
/// The caller authenticates full original length/digest and keeps file custody through this operation.
/// # Errors
/// Rejects malformed headers, metadata beyond 64 MiB/10,000 chunks, cancellation and I/O failures.
/// Reader-position restoration failures retain the primary error when both operations fail.
pub fn probe_output_animation(
    reader: &mut (impl Read + Seek),
    format: ComfyOutputFormat,
    mut check: impl FnMut() -> io::Result<()>,
) -> Result<bool, OutputAnimationError> {
    let position = reader.stream_position().map_err(OutputAnimationError::from)?;
    let result = probe(reader, format, &mut check);
    let restoration = reader.seek(SeekFrom::Start(position));
    match (result, restoration) {
        (Ok(animated), Ok(_)) => Ok(animated),
        (Err(cause), Ok(_)) | (Ok(_), Err(cause)) => Err(cause.into()),
        (Err(cause), Err(restoration)) => {
            Err(OutputAnimationError { cause, restoration: Some(restoration) })
        }
    }
}

fn probe(
    reader: &mut (impl Read + Seek),
    format: ComfyOutputFormat,
    check: &mut impl FnMut() -> io::Result<()>,
) -> io::Result<bool> {
    check()?;
    reader.seek(SeekFrom::Start(0))?;
    let mut prefix = [0u8; 21];
    let needed = if format == ComfyOutputFormat::Webp { 21 } else { 12 };
    reader.read_exact(&mut prefix[..needed])?;
    if !format.matches_prefix(&prefix[..needed]) {
        return Err(invalid("Output media header does not match its format"));
    }
    match format {
        ComfyOutputFormat::Gif => Ok(true),
        ComfyOutputFormat::Webp => {
            Ok(prefix.get(12..16) == Some(b"VP8X") && prefix[20] & 0x02 != 0)
        }
        ComfyOutputFormat::Png | ComfyOutputFormat::Apng => png_animation(reader, check),
        ComfyOutputFormat::Jpeg | ComfyOutputFormat::Mp4 | ComfyOutputFormat::Webm => Ok(false),
    }
}

fn png_animation(
    reader: &mut (impl Read + Seek),
    check: &mut impl FnMut() -> io::Result<()>,
) -> io::Result<bool> {
    reader.seek(SeekFrom::Start(8))?;
    let mut position = 8u64;
    for _ in 0..MAX_HEADER_CHUNKS {
        check()?;
        let mut header = [0u8; 8];
        reader.read_exact(&mut header)?;
        if &header[4..8] == b"IDAT" {
            return Ok(false);
        }
        let bytes = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
        position = position
            .checked_add(bytes)
            .and_then(|value| value.checked_add(12))
            .filter(|value| *value <= MAX_HEADER_BYTES)
            .ok_or_else(|| invalid("Output animation metadata exceeds its byte limit"))?;
        match &header[4..8] {
            b"acTL" if bytes == 8 => return Ok(true),
            b"acTL" => return Err(invalid("Output animation control header is invalid")),
            b"IDAT" => return Ok(false),
            b"IEND" => return Err(invalid("Output PNG has no image data")),
            _ => {
                reader.seek(SeekFrom::Current((bytes + 4) as i64))?;
            }
        }
    }
    Err(invalid("Output animation metadata exceeds its chunk limit"))
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// Preserve the original probe and any reader-restoration failure without exposing filesystem paths.
#[derive(Debug)]
pub struct OutputAnimationError {
    cause: io::Error,
    restoration: Option<io::Error>,
}
impl From<io::Error> for OutputAnimationError {
    fn from(cause: io::Error) -> Self {
        Self { cause, restoration: None }
    }
}
impl fmt::Display for OutputAnimationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Output animation probe failed: {}", self.cause)?;
        if let Some(cause) = &self.restoration {
            write!(formatter, "; reader restoration also failed: {cause}")?;
        }
        Ok(())
    }
}
impl Error for OutputAnimationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}
