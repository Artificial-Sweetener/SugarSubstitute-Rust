// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain actual scopes and immutable IDs/cutoff; central permits/jobs retain exact admitted specs.
use super::*;
pub(super) struct ExportJobs {
    pub(super) source: Scope,
    pub(super) receiving: Scope,
    pub(super) source_cleanup: Scope,
    pub(super) receiving_cleanup: Scope,
    ids: Option<ExportIds>,
    until: Moment,
}
/// IDs belong only to one actual admission attempt; accepted permits retain their specs.
struct ExportIds {
    source: JobId,
    receiving: JobId,
    source_cleanup: JobId,
    receiving_cleanup: JobId,
}
impl ExportJobs {
    /// Retain four scopes and the original cutoff without reserving future job IDs.
    pub(super) fn new(
        client: &ExecutionClient,
        scopes: &crate::scope_sequence::ScopeSequence,
        until: Moment,
    ) -> Result<Self> {
        let scope = || -> Result<Scope> {
            Ok(client.create_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, None)?)
        };
        Ok(Self {
            source: scope()?,
            receiving: scope()?,
            source_cleanup: scope()?,
            receiving_cleanup: scope()?,
            ids: None,
            until,
        })
    }
    /// Allocate IDs immediately before this attempt's first central admission.
    /// Repository/view jobs may finish before this call. Repeats keep the same attempt.
    pub(super) fn prepare(&mut self, jobs: &crate::job_sequence::JobSequence) -> Result<()> {
        if self.ids.is_none() {
            self.ids = Some(ExportIds {
                source_cleanup: jobs.allocate()?,
                receiving_cleanup: jobs.allocate()?,
                source: jobs.allocate()?,
                receiving: jobs.allocate()?,
            });
        }
        Ok(())
    }
    /// Reject an attempt whose real admission identities have not yet been allocated.
    fn ids(&self) -> Result<&ExportIds> {
        self.ids.as_ref().ok_or_else(|| "Original export admission attempt missing".into())
    }
    /// The same original action cutoff remains through actual native result delivery.
    pub(super) const fn until(&self) -> Moment {
        self.until
    }
    fn spec(
        id: JobId,
        class: JobClass,
        reservation_bytes: u64,
        deadline: Option<Moment>,
    ) -> JobSpec {
        JobSpec {
            id,
            operation: Operation::Infrastructure,
            class,
            deadline,
            reservation_bytes,
            replacement: None,
        }
    }
    /// Same original source Main8,159,232/260 admission; deadline never recomputed from now.
    pub(super) fn source_spec(&self) -> Result<JobSpec> {
        Ok(Self::spec(
            self.ids()?.source,
            JobClass::InfrastructureOrdinary,
            8_159_232,
            Some(self.until),
        ))
    }
    /// Same separate original receiving Main128KiB/68 admission.
    pub(super) fn receiving_spec(&self) -> Result<JobSpec> {
        Ok(Self::spec(
            self.ids()?.receiving,
            JobClass::InfrastructureOrdinary,
            131_072,
            Some(self.until),
        ))
    }
    /// Same independently preheld source cleanup32KiB spec; original cutoff freezes centrally.
    pub(super) fn source_cleanup_spec(&self) -> Result<JobSpec> {
        Ok(Self::spec(self.ids()?.source_cleanup, JobClass::InfrastructureLifecycle, 32_768, None))
    }
    /// Same independently preheld receiving cleanup32KiB spec.
    pub(super) fn receiving_cleanup_spec(&self) -> Result<JobSpec> {
        Ok(Self::spec(
            self.ids()?.receiving_cleanup,
            JobClass::InfrastructureLifecycle,
            32_768,
            None,
        ))
    }
}

#[cfg(test)]
#[path = "scopes/tests.rs"]
mod tests;
