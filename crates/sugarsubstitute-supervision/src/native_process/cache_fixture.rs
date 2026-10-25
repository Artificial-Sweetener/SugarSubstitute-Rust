// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise private named containment through the actual native adapter on admitted test workers.
use super::*;
use crate::setup_storage::cache_custody::CacheFamilyId;
use std::io;
use sugarsubstitute_contracts::execution::JobContext;

/// An actual empty containment object is still live until its retained native handle closes.
pub(crate) struct CacheFamily {
    family: Option<FamilyStorage>,
    identity: CacheFamilyId,
}
impl CacheFamily {
    pub(crate) fn new(context: &JobContext) -> io::Result<Self> {
        Self::named(CacheFamilyId::new(context)?, context)
    }
    /// Retain the exact containment identity already persisted by an admitted cache producer.
    pub(crate) fn named(identity: CacheFamilyId, context: &JobContext) -> io::Result<Self> {
        let mut result = Self {
            family: Some(Family::allocate_owned(context.memory()).map_err(io::Error::other)?),
            identity,
        };
        let family =
            result.family.as_mut().ok_or_else(|| io::Error::other("Fixture family absent"))?;
        family.cache_family = Some(identity);
        WindowsPort.prepare_job_native(family).map_err(io::Error::other)?;
        Ok(result)
    }
    pub(crate) fn identity(&self) -> CacheFamilyId {
        self.identity
    }
    pub(crate) fn close(&mut self) -> io::Result<()> {
        if let Some(family) = &mut self.family {
            WindowsPort.close(family).map_err(io::Error::other)?;
        }
        self.family = None;
        Ok(())
    }
}
impl Drop for CacheFamily {
    fn drop(&mut self) {
        if let Some(mut family) = self.family.take()
            && let Err(cause) = WindowsPort.close(&mut family)
        {
            tracing::error!(operation="cache_native_fixture_retirement", cause=%cause,
                "Retaining the failed native output and its original allocation");
            std::mem::forget(family);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup_storage::test_worker::{self as worker, TestResult};
    use windows::Win32::System::JobObjects::{
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        QueryInformationJobObject,
    };

    /// Read actual limits, independently of the adapter's cached seal flag.
    fn limits(family: &Family) -> io::Result<JOBOBJECT_EXTENDED_LIMIT_INFORMATION> {
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        // SAFETY: the fixture retains this valid native job and the initialized ABI output.
        unsafe {
            QueryInformationJobObject(
                Some(handle(family.job, "cache_fixture_job").map_err(io::Error::other)?),
                JobObjectExtendedLimitInformation,
                std::ptr::from_mut(&mut limits).cast(),
                size_of_val(&limits) as u32,
                None,
            )
        }
        .map_err(io::Error::other)?;
        Ok(limits)
    }
    #[test]
    fn named_cache_job_collision_retains_only_a_borrowed_reference_without_changing_limits()
    -> TestResult {
        worker::run_reserved(4 * 1024 * 1024, |context, _| {
            let mut original = CacheFamily::new(context)?;
            let family = original.family.as_mut().ok_or("Original family absent")?;
            WindowsPort.seal(family)?;
            let before = limits(family)?;
            let mut rejected = CacheFamily {
                identity: original.identity,
                family: Some(Family::allocate_owned(context.memory())?),
            };
            let rejected_family = rejected.family.as_mut().ok_or("Rejected family absent")?;
            rejected_family.cache_family = Some(original.identity);
            assert!(WindowsPort.prepare_job_native(rejected_family).is_err());
            assert!(rejected_family.job.is_none());
            assert!(rejected_family.rejected_job.is_some());
            let after = limits(family)?;
            assert_eq!(
                before.BasicLimitInformation.LimitFlags,
                after.BasicLimitInformation.LimitFlags
            );
            assert_eq!(
                before.BasicLimitInformation.ActiveProcessLimit,
                after.BasicLimitInformation.ActiveProcessLimit
            );
            rejected.close()?;
            assert!(!original.identity.retired(context)?);
            original.close()?;
            assert!(original.identity.retired(context)?);
            Ok(())
        })
    }
}
