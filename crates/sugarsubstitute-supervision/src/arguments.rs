// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode later argv entries using Microsoft C startup quoting, independently of argv0.
use crate::LaunchCommandError;
const QUOTE: u16 = b'"' as u16;
const SLASH: u16 = b'\\' as u16;

pub(crate) fn quoted_length(argument: impl Iterator<Item = u16>) -> usize {
    let mut length = 2;
    let mut slashes = 0;
    for unit in argument {
        length += 1;
        match unit {
            SLASH => slashes += 1,
            QUOTE => {
                length += slashes + 1;
                slashes = 0;
            }
            _ => slashes = 0,
        }
    }
    length + slashes
}
pub(crate) fn append_quoted(output: &mut Vec<u16>, argument: &[u16]) {
    output.push(QUOTE);
    let mut slashes = 0;
    for unit in argument {
        if *unit == SLASH {
            slashes += 1;
        } else {
            let count = if *unit == QUOTE { 2 * slashes + 1 } else { slashes };
            output.extend(std::iter::repeat_n(SLASH, count));
            output.push(*unit);
            slashes = 0;
        }
    }
    output.extend(std::iter::repeat_n(SLASH, 2 * slashes));
    output.push(QUOTE);
}
pub(crate) fn reserved(units: usize) -> Result<Vec<u16>, LaunchCommandError> {
    let mut buffer = Vec::new();
    buffer.try_reserve_exact(units).map_err(LaunchCommandError::Allocation)?;
    Ok(buffer)
}
pub(crate) fn terminated(source: &[u16]) -> Result<Vec<u16>, LaunchCommandError> {
    let mut buffer = reserved(source.len() + 1)?;
    buffer.extend_from_slice(source);
    buffer.push(0);
    Ok(buffer)
}
