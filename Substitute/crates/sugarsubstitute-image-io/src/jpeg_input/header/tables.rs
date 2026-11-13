// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate fixed JPEG quantization and Huffman definitions before decoder allocation.
use super::{byte, invalid};
use crate::ImportError;
use std::io::Read;
/// Validate exact 8-bit quantization tables without proportional allocation.
pub(super) fn quantization(
    input: &mut impl Read,
    mut remaining: usize,
    tables: &mut [u8; 4],
) -> Result<(), ImportError> {
    while remaining >= 65 {
        let index = byte(input)?;
        if index > 3 || tables[index as usize] != 0 {
            return invalid("JPEG quantization definition is unsupported");
        }
        let mut values = [0; 64];
        input.read_exact(&mut values)?;
        if values.contains(&0) {
            return invalid("JPEG quantization values must be positive");
        }
        tables[index as usize] = values[0];
        remaining -= 65;
    }
    if remaining != 0 {
        return invalid("JPEG quantization length is invalid");
    }
    Ok(())
}
/// Bound table definitions and require canonical, nonoversubscribed baseline Huffman symbols.
pub(super) fn huffman(
    input: &mut impl Read,
    mut remaining: usize,
    tables: &mut u8,
) -> Result<(), ImportError> {
    while remaining >= 17 {
        let selector = byte(input)?;
        let (class, index) = (selector >> 4, selector & 15);
        if class > 1 || index > 3 {
            return invalid("JPEG Huffman selector is invalid");
        }
        let flag = 1 << (class * 4 + index);
        if *tables & flag != 0 {
            return invalid("JPEG Huffman table is repeated");
        }
        let mut counts = [0; 16];
        input.read_exact(&mut counts)?;
        remaining -= 17;
        let count = counts.iter().map(|value| usize::from(*value)).sum::<usize>();
        if count == 0 || count > 256 || count > remaining {
            return invalid("JPEG Huffman symbol count is invalid");
        }
        let mut available = 1_i32;
        for count in counts {
            available = available * 2 - i32::from(count);
            if available < 0 {
                return invalid("JPEG Huffman codes are oversubscribed");
            }
        }
        if available == 0 {
            return invalid("JPEG Huffman codes consume the padding code");
        }
        let mut symbols = [0; 256];
        input.read_exact(&mut symbols[..count])?;
        for symbol in &symbols[..count] {
            if (class == 0 && *symbol > 11)
                || (class == 1
                    && ((*symbol & 15) > 10
                        || ((*symbol & 15) == 0 && !matches!(*symbol, 0 | 0xf0))))
            {
                return invalid("JPEG Huffman symbol exceeds baseline precision");
            }
        }
        *tables |= flag;
        remaining -= count;
    }
    if remaining != 0 {
        return invalid("JPEG Huffman length is invalid");
    }
    Ok(())
}
