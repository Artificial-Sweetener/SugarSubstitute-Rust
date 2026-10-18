// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Order exact same-profile successor admission after the known retiring native owner.
use super::*;
use sugarsubstitute_contracts::execution::Moment;

impl Record {
    /// Borrow the original charged projection while capture attachment changes its owner.
    fn profile_projection(&self) -> Option<&ProjectedEntry> {
        let completion =
            self.start.as_ref().map(|start| &start.projection).or(self.projected.as_ref())?;
        match &completion.outcome {
            JobOutcome::Success(value) => Some(value),
            _ => None,
        }
    }

    /// Keep the original request deadline observable while native retirement owns admission.
    pub(super) fn profile_wait_deadline(&self) -> Option<Moment> {
        self.profile_predecessor.as_ref().filter(|barrier| !barrier.released())?;
        Some(self.profile_projection()?.until)
    }
}
impl ProductionActivation {
    /// Retain only an authenticated retiring exact-path predecessor; unknown owners are unchanged.
    pub(super) fn profile_admission_waiting(
        records: &mut [Option<RetainedRecord>],
        at: usize,
    ) -> Result<bool> {
        let Some(current) = records[at].as_ref().map(|held| held.record.as_ref()) else {
            return Ok(false);
        };
        if current.start.as_ref().is_some_and(|start| start.acquired) {
            return Ok(false);
        }
        if let Some(predecessor) = &current.profile_predecessor {
            if !predecessor.released() {
                return Ok(true);
            }
            records[at]
                .as_mut()
                .ok_or("Original profile successor record missing")?
                .record
                .profile_predecessor = None;
            return Ok(false);
        }
        let Some(value) = current.profile_projection() else {
            return Ok(false);
        };
        let barrier = records.iter().flatten().find_map(|held| {
            let previous = &held.record;
            if previous.run == current.run {
                return None;
            }
            let close = previous.parent.as_deref()?.normal_close_request()?;
            if close.run() != previous.run {
                return None;
            }
            let barrier =
                previous.start.as_ref()?.profile.successor_retirement(&value.requested_profile)?;
            (barrier.run() == previous.run).then_some(barrier)
        });
        let waiting = barrier.is_some();
        records[at]
            .as_mut()
            .ok_or("Original profile successor record missing")?
            .record
            .profile_predecessor = barrier;
        Ok(waiting)
    }
}

/// Keep acquisition untouched until the exact consumed native release, using the original cutoff.
pub(super) fn admit_profile(
    waiting: bool,
    now: Moment,
    until: Moment,
    acquire: impl FnOnce() -> Result<()>,
) -> Result<bool> {
    if now >= until {
        return Err("Original profile acquisition deadline expired".into());
    }
    if waiting {
        return Ok(false);
    }
    acquire()?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// The actual dispatch boundary performs no acquisition while retirement is pending.
    #[test]
    fn retiring_profile_dispatch_defers_acquisition_without_blocking_distinct_profile() -> Result<()>
    {
        let until = Moment::from_nanos(100);
        let calls = Cell::new(0);
        let acquire = || {
            calls.set(calls.get() + 1);
            Ok(())
        };
        for now in [1, 2, 99] {
            assert!(!admit_profile(true, Moment::from_nanos(now), until, acquire)?);
            assert_eq!(calls.get(), 0);
        }
        // A different profile has no predecessor barrier and proceeds independently.
        let distinct = Cell::new(false);
        assert!(admit_profile(false, Moment::from_nanos(99), until, || {
            distinct.set(true);
            Ok(())
        })?);
        assert!(distinct.get());
        assert!(admit_profile(false, Moment::from_nanos(99), until, acquire)?);
        assert_eq!(calls.get(), 1);
        Ok(())
    }

    /// Waiting never extends the original request or admits an effect at its cutoff.
    #[test]
    fn profile_retirement_wait_cannot_renew_or_dispatch_after_original_deadline() {
        let called = Cell::new(false);
        for waiting in [false, true] {
            assert!(
                admit_profile(waiting, Moment::from_nanos(100), Moment::from_nanos(100), || {
                    called.set(true);
                    Ok(())
                })
                .is_err()
            );
        }
        assert!(!called.get());
    }
}
