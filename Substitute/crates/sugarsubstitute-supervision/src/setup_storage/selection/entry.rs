// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Dispatch versioned installed entry before any local native handle owners are initialized.
use super::{InheritedSelection, InheritedTrial, capture_launch_selection, capture_trial_custody};
use std::{ffi::OsString, io};
use sugarsubstitute_contracts::application_launch::TRIAL_LAUNCH_ARGUMENT;

/// Actual once-captured native entry custody, validated by the elected coordinator's worker.
pub enum InheritedLaunch {
    /// Protected selection reference for accepted startup or forwarding to a live trial owner.
    /// It never grants permission to admit an unaccepted generation as an elected coordinator.
    Selection(InheritedSelection),
    /// Candidate startup with live installation exclusion and its protected trial snapshot.
    Trial(InheritedTrial),
}

/// Capture either supported installed entry protocol; unrelated roles remain untouched.
/// # Safety
/// Call once at executable entry before resources or workers can own any supplied handle.
/// The caller must guarantee unique adoption and prevent concurrent closure/reuse of all slots.
/// # Errors
/// Rejects unsupported versions, malformed arguments and invalid inherited native capabilities.
pub unsafe fn capture_launch(
    arguments: impl IntoIterator<Item = OsString>,
) -> io::Result<Option<InheritedLaunch>> {
    let mut arguments = arguments.into_iter();
    let Some(prefix) = arguments.next() else {
        return Ok(None);
    };
    if prefix == TRIAL_LAUNCH_ARGUMENT {
        let handles = trial_handles(arguments)?;
        // SAFETY: the caller guarantees the same initial-entry ownership for every slot.
        return unsafe { capture_trial_custody(handles) }
            .map(|trial| Some(InheritedLaunch::Trial(trial)));
    }
    // SAFETY: forwarding untouched v1 arguments preserves the caller's initial-entry guarantee.
    unsafe { capture_launch_selection(std::iter::once(prefix).chain(arguments)) }
        .map(|selection| selection.map(InheritedLaunch::Selection))
}

/// Decode exactly the three canonical handle slots (run witness, snapshot, slot-held event)
/// before adopting any native object.
fn trial_handles(arguments: impl IntoIterator<Item = OsString>) -> io::Result<[usize; 3]> {
    let mut arguments = arguments.into_iter();
    let mut handles = [0; 3];
    for handle in &mut handles {
        let value = arguments
            .next()
            .and_then(|value| value.into_string().ok())
            .ok_or_else(|| super::super::invalid("Trial launch handle is absent"))?;
        *handle = value
            .parse::<usize>()
            .map_err(|_| super::super::invalid("Trial launch handle is invalid"))?;
        if *handle == 0 || *handle == usize::MAX || handle.to_string() != value {
            return Err(super::super::invalid("Trial launch handle is not canonical"));
        }
    }
    if handles[0] == handles[1]
        || handles[0] == handles[2]
        || handles[1] == handles[2]
        || arguments.next().is_some()
    {
        return Err(super::super::invalid("Trial launch handle roles are ambiguous"));
    }
    Ok(handles)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A launcher from before selection schema 2 spells trial v2 and accepted v1. Neither is
    /// captured as any entry, so no handle is adopted and the old snapshot is never decoded.
    #[test]
    fn entries_from_earlier_launch_protocols_are_refused_before_any_handle_is_adopted() {
        for prefix in ["--installed-launch-v1", "--installed-launch-v2", "--installed-launch-v5"] {
            let arguments = [prefix, "4", "8"].map(OsString::from);
            // SAFETY: each spelling is refused while parsing, before any slot is adopted.
            let captured = unsafe { capture_launch(arguments) };
            assert!(captured.is_err(), "{prefix} was captured");
        }
    }
    #[test]
    fn trial_protocol_requires_three_distinct_canonical_handles() -> io::Result<()> {
        assert_eq!(trial_handles(["4", "8", "12"].map(OsString::from))?, [4, 8, 12]);
        for values in [
            vec![],
            vec!["4"],
            vec!["4", "8"],
            vec!["4", "4", "12"],
            vec!["4", "8", "4"],
            vec!["4", "8", "8"],
            vec!["0", "8", "12"],
            vec!["04", "8", "12"],
            vec!["4", "+8", "12"],
            vec!["4", "8", "12", "extra"],
        ] {
            assert!(trial_handles(values.into_iter().map(OsString::from)).is_err());
        }
        assert!(
            trial_handles(["4".into(), "8".into(), OsString::from(usize::MAX.to_string())])
                .is_err()
        );
        Ok(())
    }
}
