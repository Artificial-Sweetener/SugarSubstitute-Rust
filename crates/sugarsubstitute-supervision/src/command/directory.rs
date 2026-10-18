// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Give child path libraries an ordinary cwd without changing extended-path meaning.
use crate::{LaunchCommandError, arguments};

const BACKSLASH: u16 = b'\\' as u16;
const DOT: u16 = b'.' as u16;
const SPACE: u16 = b' ' as u16;
const LEGACY_PATH_UNITS: usize = 260;

/// Prepare the same operational spelling for explicit child environment paths.
/// Lexical validation precedes normalization; this establishes no filesystem identity.
pub(crate) fn operational(units: &[u16]) -> Result<Vec<u16>, LaunchCommandError> {
    if units.len() >= super::MAX_UNITS || units.contains(&0) || !super::paths::valid(units, false) {
        return Err(LaunchCommandError::InvalidWorkingDirectory);
    }
    let mut result = terminated(units)?;
    result.truncate(result.len().saturating_sub(1));
    Ok(result)
}

/// Encode an already shape-validated cwd, including its terminating NUL.
///
/// Python's Windows root-relative resolution misinterprets an extended cwd as a UNC
/// root. Its filesystem identity remains with the launch proof; this only chooses
/// an equivalent child-facing spelling. Extended-only components and long paths
/// must retain their original namespace because removing it can name another file.
pub(super) fn terminated(units: &[u16]) -> Result<Vec<u16>, LaunchCommandError> {
    let extended = [BACKSLASH, BACKSLASH, b'?' as u16, BACKSLASH];
    if !units.starts_with(&extended) {
        return arguments::terminated(units);
    }
    let unc = units[4..].starts_with(&[b'U' as u16, b'N' as u16, b'C' as u16, BACKSLASH]);
    let (tail, prefix, body) =
        if unc { (&units[8..], 2, &units[8..]) } else { (&units[4..], 0, &units[7..]) };
    let length = prefix + tail.len();
    if length >= LEGACY_PATH_UNITS || !ordinary_components(body) {
        return arguments::terminated(units);
    }
    let mut directory = arguments::reserved(length + 1)?;
    if unc {
        directory.extend_from_slice(&[BACKSLASH, BACKSLASH]);
    }
    directory.extend_from_slice(tail);
    directory.push(0);
    Ok(directory)
}

/// Require ordinary Win32 component semantics, including reserved DOS device names.
fn ordinary_components(body: &[u16]) -> bool {
    if body.is_empty() {
        return true;
    }
    let mut components = body.split(|unit| *unit == BACKSLASH).peekable();
    while let Some(component) = components.next() {
        if component.is_empty() {
            return components.peek().is_none();
        }
        if matches!(component.last(), Some(&DOT | &SPACE)) || device(component) {
            return false;
        }
    }
    true
}

/// Preserve components that ordinary Win32 would interpret as a DOS device.
fn device(component: &[u16]) -> bool {
    let stem = component.split(|unit| *unit == DOT).next().unwrap_or(component);
    let end = stem.iter().rposition(|unit| *unit != SPACE).map_or(0, |index| index + 1);
    let stem = &stem[..end];
    ["CON", "PRN", "AUX", "NUL", "CLOCK$", "CONIN$", "CONOUT$"]
        .iter()
        .any(|name| equal_ascii(stem, name.as_bytes()))
        || stem.len() == 4
            && (equal_ascii(&stem[..3], b"COM") || equal_ascii(&stem[..3], b"LPT"))
            && matches!(stem[3], 49..=57 | 0xb9 | 0xb2 | 0xb3)
}

/// Compare an uppercase ASCII device name without changing UTF-16 filename units.
fn equal_ascii(units: &[u16], ascii: &[u8]) -> bool {
    units.len() == ascii.len()
        && units.iter().zip(ascii).all(|(unit, byte)| {
            *unit == u16::from(*byte) || *unit == u16::from(byte.to_ascii_lowercase())
        })
}
