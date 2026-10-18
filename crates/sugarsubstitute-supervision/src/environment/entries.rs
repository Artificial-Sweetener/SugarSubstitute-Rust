// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow bounded environment entries and merge through fallible native ordering.
use super::{
    EnvironmentOverride, LaunchEnvironmentError, MAX_OVERRIDES, MAX_UNITS, MAX_VARIABLES, ordinal,
};
use std::cmp::Ordering;

pub(super) struct Entry<'a> {
    pub(super) name: &'a [u16],
    pub(super) value: Option<&'a [u16]>,
}
pub(super) fn preflight(
    snapshot: &[u16],
    overrides: &[EnvironmentOverride<'_>],
) -> Result<(), LaunchEnvironmentError> {
    if snapshot.len() > MAX_UNITS {
        return Err(LaunchEnvironmentError::SourceTooLong);
    }
    if overrides.len() > MAX_OVERRIDES {
        return Err(LaunchEnvironmentError::TooManyOverrides);
    }
    let mut total = 0_usize;
    for change in overrides {
        let entry = override_entry(change);
        total = total
            .checked_add(entry.name.len())
            .and_then(|size| size.checked_add(entry.value.map_or(0, <[u16]>::len)))
            .filter(|size| *size <= MAX_UNITS)
            .ok_or(LaunchEnvironmentError::SourceTooLong)?;
    }
    Ok(())
}
fn override_entry<'a>(change: &EnvironmentOverride<'a>) -> Entry<'a> {
    match change {
        EnvironmentOverride::Set { name, value } => Entry { name, value: Some(value) },
        EnvironmentOverride::Remove { name } => Entry { name, value: None },
    }
}
fn ordinary_name(name: &[u16]) -> bool {
    !name.is_empty() && !name.iter().any(|unit| *unit == 0 || *unit == b'=' as u16)
}
fn snapshot_entry(line: &[u16]) -> Result<Entry<'_>, LaunchEnvironmentError> {
    let hidden = line.first() == Some(&(b'=' as u16));
    let separator = line
        .iter()
        .enumerate()
        .skip(usize::from(hidden))
        .find(|(_, unit)| **unit == b'=' as u16)
        .map(|(index, _)| index)
        .ok_or(LaunchEnvironmentError::InvalidName)?;
    let name = &line[..separator];
    let valid = if hidden {
        name.len() == 3 && matches!(name[1], 65..=90 | 97..=122) && name[2] == b':' as u16
    } else {
        ordinary_name(name)
    };
    if !valid {
        return Err(LaunchEnvironmentError::InvalidName);
    }
    Ok(Entry { name, value: Some(&line[separator + 1..]) })
}
pub(super) fn snapshot_count(snapshot: &[u16]) -> Result<usize, LaunchEnvironmentError> {
    if !snapshot.ends_with(&[0, 0]) {
        return Err(LaunchEnvironmentError::InvalidSnapshot);
    }
    if snapshot == [0, 0] {
        return Ok(0);
    }
    let mut count = 0;
    let mut start = 0;
    for (end, unit) in snapshot[..snapshot.len() - 1].iter().enumerate() {
        if *unit == 0 {
            if start == end {
                return Err(LaunchEnvironmentError::InvalidSnapshot);
            }
            count += 1;
            if count > MAX_VARIABLES {
                return Err(LaunchEnvironmentError::TooManyVariables);
            }
            start = end + 1;
        }
    }
    Ok(count)
}
pub(super) fn position(
    entries: &[Entry<'_>],
    name: &[u16],
) -> Result<Result<usize, usize>, LaunchEnvironmentError> {
    let mut low = 0;
    let mut high = entries.len();
    while low < high {
        let middle = low + (high - low) / 2;
        match ordinal::compare(entries[middle].name, name)? {
            Ordering::Less => low = middle + 1,
            Ordering::Greater => high = middle,
            Ordering::Equal => return Ok(Ok(middle)),
        }
    }
    Ok(Err(low))
}
pub(super) fn merge<'a>(
    snapshot: &'a [u16],
    overrides: &[EnvironmentOverride<'a>],
) -> Result<Vec<Entry<'a>>, LaunchEnvironmentError> {
    let count = snapshot_count(snapshot)?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(count + overrides.len())
        .map_err(LaunchEnvironmentError::Allocation)?;
    if count > 0 {
        for line in snapshot[..snapshot.len() - 1].split(|unit| *unit == 0).take(count) {
            let entry = snapshot_entry(line)?;
            match position(&entries, entry.name)? {
                Ok(_) => return Err(LaunchEnvironmentError::DuplicateSnapshot),
                Err(index) => entries.insert(index, entry),
            }
        }
    }
    let mut changes = Vec::new();
    changes.try_reserve_exact(overrides.len()).map_err(LaunchEnvironmentError::Allocation)?;
    for change in overrides {
        let entry = override_entry(change);
        if !ordinary_name(entry.name) {
            return Err(LaunchEnvironmentError::InvalidName);
        }
        if entry.value.is_some_and(|value| value.contains(&0)) {
            return Err(LaunchEnvironmentError::InteriorNul);
        }
        if entry.value.is_some() && ordinal::private_name(entry.name)? {
            return Err(LaunchEnvironmentError::PrivateVariable);
        }
        match position(&changes, entry.name)? {
            Ok(_) => return Err(LaunchEnvironmentError::DuplicateOverride),
            Err(index) => changes.insert(index, entry),
        }
    }
    let mut index = 0;
    while index < entries.len() {
        if ordinal::private_name(entries[index].name)? {
            entries.remove(index);
        } else {
            index += 1;
        }
    }
    for change in changes {
        match (position(&entries, change.name)?, change.value.is_some()) {
            (Ok(index), true) => entries[index] = change,
            (Ok(index), false) => {
                entries.remove(index);
            }
            (Err(index), true) => entries.insert(index, change),
            (Err(_), false) => {}
        }
    }
    if entries.len() > MAX_VARIABLES {
        return Err(LaunchEnvironmentError::TooManyVariables);
    }
    Ok(entries)
}
