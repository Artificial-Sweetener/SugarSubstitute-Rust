// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate a fixed sequential JPEG subset without allocating from untrusted segment lengths.
mod tables;
use crate::ImportError;
use std::io::{Read, Seek, SeekFrom};
use sugarsubstitute_image_ops::Extent;

/// Preflight facts sufficient to bound every image-dependent decoder allocation.
pub(super) struct Header {
    pub(super) extent: Extent,
    components: usize,
    horizontal: u64,
    vertical: u64,
    ids: [u8; 3],
    quantization: [u8; 3],
}
impl Header {
    /// Check bounded original length and complete marker framing on the same admitted stream.
    pub(super) fn read(input: &mut (impl Read + Seek)) -> Result<Self, ImportError> {
        let length = input.seek(SeekFrom::End(0))?;
        if length > crate::MAX_ENCODED_SOURCE_BYTES {
            return invalid("JPEG original exceeds the encoded source limit");
        }
        input.rewind()?;
        if word(input)? != 0xffd8 {
            return invalid("JPEG SOI is absent");
        }
        let mut frame = None;
        let (mut quantization, mut huffman, mut jfif) = ([0_u8; 4], 0_u8, false);
        loop {
            if byte(input)? != 255 {
                return invalid("JPEG marker prefix is absent");
            }
            let marker = byte(input)?;
            let remaining = usize::from(
                word(input)?
                    .checked_sub(2)
                    .ok_or(ImportError::Invalid("JPEG segment length is invalid"))?,
            );
            match marker {
                0xe0 if !jfif => {
                    if remaining != 14 {
                        return invalid("JPEG requires fixed JFIF without thumbnail metadata");
                    }
                    let mut data = [0; 14];
                    input.read_exact(&mut data)?;
                    if &data[..5] != b"JFIF\0"
                        || data[5] != 1
                        || data[6] > 2
                        || data[7] > 2
                        || data[12..] != [0, 0]
                    {
                        return invalid("JPEG JFIF metadata is unsupported");
                    }
                    jfif = true;
                }
                0xdb => tables::quantization(input, remaining, &mut quantization)?,
                0xc4 => tables::huffman(input, remaining, &mut huffman)?,
                0xc0 if frame.is_none() => frame = Some(Self::frame(input, remaining)?),
                0xda => {
                    let header =
                        frame.ok_or(ImportError::Invalid("JPEG SOS precedes its frame"))?;
                    header.arithmetic(&quantization)?;
                    header.scan(input, remaining, huffman)?;
                    entropy(input)?;
                    return Ok(header);
                }
                _ => return invalid("JPEG requires one sequential scan and JFIF-only metadata"),
            }
        }
    }
    /// Require standard component identities and exact sampling combinations, in SOF order.
    fn frame(input: &mut impl Read, remaining: usize) -> Result<Self, ImportError> {
        if !matches!(remaining, 9 | 15) {
            return invalid("JPEG frame length is unsupported");
        }
        let mut data = [0; 15];
        input.read_exact(&mut data[..remaining])?;
        let components = usize::from(data[5]);
        if data[0] != 8 || !matches!(components, 1 | 3) || remaining != 6 + 3 * components {
            return invalid("JPEG requires one or three 8-bit components");
        }
        let extent = Extent::new(
            u32::from(u16::from_be_bytes([data[3], data[4]])),
            u32::from(u16::from_be_bytes([data[1], data[2]])),
        )?;
        let mut ids = [0; 3];
        let mut quantization = [0; 3];
        for (index, id) in ids.iter_mut().enumerate().take(components) {
            *id = data[6 + 3 * index];
            let table = data[8 + 3 * index];
            if table > 3 {
                return invalid("JPEG quantization table index is invalid");
            }
            quantization[index] = table;
        }
        let direct = (components == 1 && ids[0] == 1 && data[7] == 0x11)
            || (components == 3
                && ids == [b'R', b'G', b'B']
                && [data[7], data[10], data[13]] == [0x11; 3]);
        let sampling = if direct {
            (1, 1)
        } else if components == 3 && ids == [1, 2, 3] && data[10] == 0x11 && data[13] == 0x11 {
            match data[7] {
                0x11 => (1, 1),
                0x21 => (2, 1),
                0x22 => (2, 2),
                _ => return invalid("JPEG sampling factors are unsupported"),
            }
        } else {
            return invalid("JPEG component interpretation is unsupported");
        };
        Ok(Self {
            extent,
            components,
            horizontal: sampling.0,
            vertical: sampling.1,
            ids,
            quantization,
        })
    }
    /// All components must appear once in the sole complete sequential scan.
    fn scan(&self, input: &mut impl Read, remaining: usize, tables: u8) -> Result<(), ImportError> {
        if remaining != 4 + 2 * self.components {
            return invalid("JPEG scan length is invalid");
        }
        let mut data = [0; 10];
        input.read_exact(&mut data[..remaining])?;
        if usize::from(data[0]) != self.components || data[remaining - 3..remaining] != [0, 63, 0] {
            return invalid("JPEG requires one complete sequential scan");
        }
        for index in 0..self.components {
            let selectors = data[2 + 2 * index];
            let (dc, ac) = (selectors >> 4, selectors & 15);
            if data[1 + 2 * index] != self.ids[index]
                || dc > 3
                || ac > 3
                || tables & (1 << dc) == 0
                || tables & (1 << (4 + ac)) == 0
            {
                return invalid(
                    "JPEG scan references unsupported components or absent Huffman tables",
                );
            }
        }
        Ok(())
    }
    /// Bound the dependency's ordinary DC dequantization multiply even for malformed entropy.
    /// Each baseline block can change its DC predictor by at most 2047. Require every possible
    /// predictor times its positive DC quantizer to fit i32; scalar IDCT then uses wrapping math.
    fn arithmetic(&self, tables: &[u8; 4]) -> Result<(), ImportError> {
        let mcus = u64::from(self.extent.width())
            .div_ceil(8 * self.horizontal)
            .checked_mul(u64::from(self.extent.height()).div_ceil(8 * self.vertical))
            .ok_or(ImportError::Invalid("JPEG MCU count overflow"))?;
        for index in 0..self.components {
            let quantizer = u64::from(tables[self.quantization[index] as usize]);
            let blocks = if index == 0 { mcus * self.horizontal * self.vertical } else { mcus };
            if quantizer == 0
                || blocks
                    .checked_mul(2047)
                    .and_then(|bound| bound.checked_mul(quantizer))
                    .is_none_or(|bound| bound > i32::MAX as u64)
            {
                return invalid(
                    "JPEG dimensions and DC quantizer exceed the safe sequential arithmetic subset",
                );
            }
        }
        Ok(())
    }
    /// Exact RGB output size, independently checked before decoder invocation.
    pub(super) fn rgb_bytes(&self) -> Result<usize, ImportError> {
        usize::try_from(
            self.extent
                .pixels()
                .checked_mul(3)
                .ok_or(ImportError::Invalid("JPEG RGB size overflow"))?,
        )
        .map_err(|_| ImportError::Invalid("JPEG RGB size exceeds the platform"))
    }
    /// Bound pinned scalar decoder allocations, including fixed components and error construction.
    ///
    /// P is the full MCU-padded width. Raw/auxiliary/scratch i16 payloads sum to 16P for
    /// grayscale, 48P for 4:4:4, 110P for 4:2:2, and 168P for 4:2:0. The standard 4:2:2
    /// horizontal resize equals its initial allocation. Each private Components is below 1024
    /// bytes on supported 32/64-bit targets; 4096 covers bounded diagnostic construction.
    /// Every allocation also carries the owner's 128-byte accounting allowance.
    pub(super) fn decoder_bytes(&self) -> Result<u64, ImportError> {
        let padded =
            u64::from(self.extent.width()).div_ceil(8 * self.horizontal) * 8 * self.horizontal;
        let sampled = self.horizontal != 1 || self.vertical != 1;
        let factor = match (self.components, self.horizontal, self.vertical) {
            (1, 1, 1) => 16,
            (3, 1, 1) => 48,
            (3, 2, 1) => 110,
            (3, 2, 2) => 168,
            _ => return invalid("JPEG allocation facts are unsupported"),
        };
        let components = self.components as u64;
        let allocations = 1 + components + if sampled { 4 * components + 1 } else { 0 };
        padded
            .checked_mul(factor)
            .and_then(|bytes| bytes.checked_add(1024 * components + 4096 + 128 * allocations))
            .ok_or(ImportError::Invalid("JPEG decoder reservation overflow"))
    }
}

/// Require one entropy stream with byte stuffing, terminal EOI and no trailing image.
fn entropy(input: &mut impl Read) -> Result<(), ImportError> {
    let mut data = false;
    loop {
        if byte(input)? != 255 {
            data = true;
            continue;
        }
        match byte(input)? {
            0 => data = true,
            0xd9 if data => {
                let mut trailing = [0];
                if input.read(&mut trailing)? != 0 {
                    return invalid("JPEG contains trailing bytes");
                }
                return Ok(());
            }
            _ => return invalid("JPEG entropy contains an unsupported marker or another scan"),
        }
    }
}
/// Read one exact marker/sample byte without allocating.
fn byte(input: &mut impl Read) -> Result<u8, ImportError> {
    let mut value = [0];
    input.read_exact(&mut value)?;
    Ok(value[0])
}
/// Read one exact big-endian JPEG word without allocating.
fn word(input: &mut impl Read) -> Result<u16, ImportError> {
    let mut value = [0; 2];
    input.read_exact(&mut value)?;
    Ok(u16::from_be_bytes(value))
}
/// Reject unsupported interpretation while retaining a bounded static diagnostic.
fn invalid<T>(message: &'static str) -> Result<T, ImportError> {
    Err(ImportError::Invalid(message))
}
