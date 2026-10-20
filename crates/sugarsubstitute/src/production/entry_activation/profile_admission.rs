// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Join native and domain retirement before admitting a known same-profile successor.
use super::*;
use sugarsubstitute_contracts::execution::Moment;

impl Record {
    /// Borrow the original charged projection while capture attachment changes its owner.
    pub(super) fn profile_projection(&self) -> Option<&ProjectedEntry> {
        let completion =
            self.start.as_ref().map(|start| &start.projection).or(self.projected.as_ref())?;
        match &completion.outcome {
            JobOutcome::Success(value) => Some(value),
            _ => None,
        }
    }

    /// Keep the original cutoff observable until both retained predecessor owners release.
    pub(super) fn profile_wait_deadline(&self) -> Option<Moment> {
        self.profile_predecessor.as_ref()?;
        Some(self.profile_projection()?.until)
    }
}
impl ProductionActivation {
    /// Retain only an authenticated retiring exact-path predecessor; unknown owners are unchanged.
    pub(super) fn profile_admission_waiting(
        records: &mut [Option<RetainedRecord>],
        at: usize,
        backend: &CoordinatorBackend,
    ) -> Result<bool> {
        let Some(current) = records[at].as_ref().map(|held| held.record.as_ref()) else {
            return Ok(false);
        };
        if current.start.as_ref().is_some_and(|start| start.acquired) {
            return Ok(false);
        }
        if let Some(predecessor) = &current.profile_predecessor {
            let value =
                current.profile_projection().ok_or("Original profile projection missing")?;
            if predecessor_waiting(predecessor.released(), || {
                Ok(backend.profile_successor_waiting(&value.profile, predecessor.run())?)
            })? {
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
            if previous.run == current.run || previous.closed {
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
        let waiting = match &barrier {
            Some(predecessor) => predecessor_waiting(predecessor.released(), || {
                Ok(backend.profile_successor_waiting(&value.profile, predecessor.run())?)
            })?,
            None => false,
        };
        records[at]
            .as_mut()
            .ok_or("Original profile successor record missing")?
            .record
            .profile_predecessor = if waiting { barrier } else { None };
        Ok(waiting)
    }
}

/// Require both independent owners, retaining native authority while domain cleanup progresses.
fn predecessor_waiting(
    native_released: bool,
    domain_waiting: impl FnOnce() -> Result<bool>,
) -> Result<bool> {
    Ok(!native_released || domain_waiting()?)
}

/// Keep acquisition untouched until both predecessor owners retire, using the original cutoff.
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

    /// Either remaining owner blocks the real dispatch; only their join admits one effect.
    #[test]
    fn profile_dispatch_joins_native_release_with_actual_domain_settlement() -> Result<()> {
        use sugarsubstitute_application::supervision::ResourceCoordinator;
        use sugarsubstitute_contracts::{configuration::ProfileId, lifecycle::*};
        let predecessor = RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        };
        let profile = ProfileId::new("exclusive".into())?;
        let mut policy = ResourceCoordinator::new(predecessor.owner);
        policy.enroll_run(RunEnrollment {
            run: predecessor,
            launch: LaunchId::new(1)?,
            profile: profile.clone(),
        })?;
        let until = Moment::from_nanos(100);
        let calls = Cell::new(0);
        let acquire = || {
            calls.set(calls.get() + 1);
            Ok(())
        };
        for native_released in [false, true] {
            let waiting = predecessor_waiting(native_released, || {
                Ok(policy.profile_successor_waiting(&profile, predecessor)?)
            })?;
            assert!(!admit_profile(waiting, Moment::from_nanos(99), until, acquire)?);
            assert_eq!(calls.get(), 0);
            assert!(admit_profile(waiting, until, until, acquire).is_err());
        }
        // The policy owner accepts settlement of this actually enrolled unlaunched capsule.
        policy.settle_run(predecessor)?;
        assert!(predecessor_waiting(false, || {
            Ok(policy.profile_successor_waiting(&profile, predecessor)?)
        })?);
        let waiting = predecessor_waiting(true, || {
            Ok(policy.profile_successor_waiting(&profile, predecessor)?)
        })?;
        assert!(!waiting);
        assert!(admit_profile(waiting, Moment::from_nanos(99), until, acquire)?);
        assert_eq!(calls.get(), 1);
        Ok(())
    }

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
