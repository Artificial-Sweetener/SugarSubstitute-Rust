// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate fully-qualified Windows lexical shape without accessing or trusting the filesystem.
const BACKSLASH: u16 = b'\\' as u16;
const SLASH: u16 = b'/' as u16;
const COLON: u16 = b':' as u16;
const DOT: u16 = b'.' as u16;
const FORBIDDEN: [u16; 7] =
    [b'"' as u16, COLON, b'<' as u16, b'>' as u16, b'?' as u16, b'|' as u16, b'*' as u16];
fn drive(units: &[u16], extended: bool) -> bool {
    units.len() >= 3
        && matches!(units[0], 65..=90 | 97..=122)
        && units[1] == COLON
        && separator(units[2], extended)
}
fn separator(unit: u16, extended: bool) -> bool {
    unit == BACKSLASH || !extended && unit == SLASH
}
pub(super) fn valid(units: &[u16], executable: bool) -> bool {
    let extended = units.starts_with(&[BACKSLASH, BACKSLASH, b'?' as u16, BACKSLASH]);
    let path = if extended { &units[4..] } else { units };
    let (body, minimum) = if drive(path, extended) {
        (&path[3..], usize::from(executable))
    } else if extended && path.starts_with(&[b'U' as u16, b'N' as u16, b'C' as u16, BACKSLASH]) {
        (&path[4..], 2 + usize::from(executable))
    } else if !extended && path.starts_with(&[BACKSLASH, BACKSLASH]) {
        (&path[2..], 2 + usize::from(executable))
    } else {
        return false;
    };
    if body.iter().any(|unit| *unit <= 31 || FORBIDDEN.contains(unit) || extended && *unit == SLASH)
    {
        return false;
    }
    if executable && body.last().is_none_or(|unit| separator(*unit, extended)) {
        return false;
    }
    if minimum >= 2 {
        let mut root = body.split(|unit| separator(*unit, extended));
        if root.next().is_none_or(|part| part.is_empty())
            || root.next().is_none_or(|part| part.is_empty())
        {
            return false;
        }
    }
    let mut count = 0;
    for component in body.split(|unit| separator(*unit, extended)) {
        if component.is_empty() {
            continue;
        }
        if component == [DOT] || component == [DOT, DOT] {
            return false;
        }
        count += 1;
    }
    count >= minimum
}
