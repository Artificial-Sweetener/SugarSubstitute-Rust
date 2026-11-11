// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate actual native observations before fixed descendant creation/resume.
use crate::RegistryError;
use sugarsubstitute_execution::windows_fixture::FixtureDeadline;
use windows::Win32::System::JobObjects::{
    JOB_OBJECT_LIMIT_BREAKAWAY_OK, JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK,
};
pub(super) fn parent(contained: bool, flags: u32) -> Result<(), RegistryError> {
    if !contained {
        return Err(RegistryError::invalid("fixture_root_uncontained"));
    }
    if flags & (JOB_OBJECT_LIMIT_BREAKAWAY_OK | JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK).0 != 0 {
        return Err(RegistryError::invalid("fixture_root_breakaway_limits"));
    }
    Ok(())
}
pub(super) fn resume(previous: u32) -> Result<(), RegistryError> {
    if previous != 1 {
        return Err(RegistryError::invalid("fixture_descendant_suspend_count"));
    }
    Ok(())
}
pub(super) fn deadline(
    deadline: FixtureDeadline,
    ticks: i64,
    frequency: i64,
) -> Result<(), RegistryError> {
    if ticks <= 0
        || frequency <= 0
        || frequency as u64 != deadline.frequency()
        || ticks as u64 >= deadline.ticks()
    {
        return Err(RegistryError::invalid("fixture_descendant_deadline"));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn descendant_native_checks_reject_uncontained_and_breakaway_roots() -> Result<(), RegistryError>
    {
        assert!(parent(false, 0).is_err());
        for flags in [
            JOB_OBJECT_LIMIT_BREAKAWAY_OK.0,
            JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK.0,
            (JOB_OBJECT_LIMIT_BREAKAWAY_OK | JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK).0,
        ] {
            assert!(parent(true, flags).is_err());
        }
        parent(true, 0)
    }
    #[test]
    fn descendant_native_checks_preserve_original_clock_and_suspended_thread_contract()
    -> Result<(), Box<dyn std::error::Error>> {
        let original = FixtureDeadline::new(1000, 1000)?;
        for (ticks, frequency) in [(0, 1000), (1, 0), (1000, 1000), (1001, 1000), (999, 999)] {
            assert!(deadline(original, ticks, frequency).is_err());
        }
        deadline(original, 999, 1000)?;
        for previous in [0, 2, u32::MAX] {
            assert!(resume(previous).is_err());
        }
        resume(1)?;
        Ok(())
    }
}
