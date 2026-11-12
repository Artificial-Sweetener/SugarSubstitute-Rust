// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Allocate actual watch-before-creation specifications without renewing deadlines.
use super::Result;
use sugarsubstitute_contracts::execution::{JobClass, JobId, JobSpec, Moment, Operation};
pub(super) fn next(
    identity: &mut u64,
    class: JobClass,
    bytes: u64,
    deadline: Option<Moment>,
) -> Result<JobSpec> {
    let id = JobId::new(*identity)?;
    *identity = identity.checked_add(1).ok_or("Claim campaign job identity exhausted")?;
    Ok(JobSpec {
        id,
        operation: Operation::NativeLifecycle,
        class,
        deadline,
        reservation_bytes: bytes,
        replacement: None,
    })
}
/// The registry actually admits this root watch before its finite creation job.
pub(super) fn launch(
    identity: &mut u64,
    bytes: u64,
    original: Moment,
) -> Result<(JobSpec, JobSpec)> {
    let watch = next(identity, JobClass::NativeObservation, 4096, None)?;
    let creation = next(identity, JobClass::InfrastructureLifecycle, bytes, Some(original))?;
    Ok((watch, creation))
}
/// Map an existing admitted Moment after sampling Instant before the local clock.
/// Checked subtraction rejects expiry rather than starting another budget.
pub(super) fn native_deadline(
    sampled: std::time::Instant,
    now: Moment,
    admitted: Moment,
) -> Result<std::time::Instant> {
    let nanos = admitted
        .as_nanos()
        .checked_sub(now.as_nanos())
        .ok_or("Claim child original deadline expired")?;
    sampled
        .checked_add(std::time::Duration::from_nanos(nanos))
        .ok_or_else(|| "Claim child native deadline overflow".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_launch_pair_matches_registry_admission_and_preserves_deadline() -> Result<()> {
        let mut identity = 17;
        let original = Moment::from_nanos(456);
        let (watch, creation) = launch(&mut identity, 12345, original)?;
        assert!(watch.id < creation.id);
        assert_eq!(watch.id.get(), 17);
        assert_eq!(creation.id.get(), 18);
        assert_eq!(identity, 19);
        assert_eq!(watch.class, JobClass::NativeObservation);
        assert_eq!(watch.deadline, None);
        assert_eq!(creation.class, JobClass::InfrastructureLifecycle);
        assert_eq!(creation.deadline, Some(original));
        assert_eq!(creation.reservation_bytes, 12345);
        assert_eq!(creation.operation, Operation::NativeLifecycle);
        assert_eq!(watch.operation, creation.operation);
        Ok(())
    }
    #[test]
    fn headless_expiry_mapping_uses_original_remaining_budget_and_rejects_expiry() -> Result<()> {
        let sampled = std::time::Instant::now();
        let mapped = native_deadline(sampled, Moment::from_nanos(900), Moment::from_nanos(1000))?;
        assert_eq!(mapped.duration_since(sampled), std::time::Duration::from_nanos(100));
        assert!(
            native_deadline(sampled, Moment::from_nanos(1001), Moment::from_nanos(1000)).is_err()
        );
        assert_eq!(
            native_deadline(sampled, Moment::from_nanos(1000), Moment::from_nanos(1000))?,
            sampled
        );
        Ok(())
    }
}
