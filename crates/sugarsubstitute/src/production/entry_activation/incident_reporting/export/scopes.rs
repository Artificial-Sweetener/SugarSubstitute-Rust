// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain actual scopes and immutable IDs/cutoff; central permits/jobs retain exact admitted specs.
use super::*;
pub(super) struct ExportJobs {
    pub(super) source: Scope,
    pub(super) receiving: Scope,
    pub(super) source_cleanup: Scope,
    pub(super) receiving_cleanup: Scope,
    source_id: JobId,
    receiving_id: JobId,
    source_cleanup_id: JobId,
    receiving_cleanup_id: JobId,
    until: Moment,
}
impl ExportJobs {
    /// Allocate four independent owned scopes and consume IDs before original admission.
    pub(super) fn new(resources: &EntryResources<'_>, until: Moment) -> Result<Self> {
        let scope = || -> Result<Scope> {
            Ok(resources.client.create_scope(
                ScopeSpec { id: resources.scopes.allocate()?, deadline: None },
                None,
            )?)
        };
        Ok(Self {
            source: scope()?,
            receiving: scope()?,
            source_cleanup: scope()?,
            receiving_cleanup: scope()?,
            source_cleanup_id: resources.jobs.allocate()?,
            receiving_cleanup_id: resources.jobs.allocate()?,
            source_id: resources.jobs.allocate()?,
            receiving_id: resources.jobs.allocate()?,
            until,
        })
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
    pub(super) fn source_spec(&self) -> JobSpec {
        Self::spec(self.source_id, JobClass::InfrastructureOrdinary, 8_159_232, Some(self.until))
    }
    /// Same separate original receiving Main128KiB/68 admission.
    pub(super) fn receiving_spec(&self) -> JobSpec {
        Self::spec(self.receiving_id, JobClass::InfrastructureOrdinary, 131_072, Some(self.until))
    }
    /// Same independently preheld source cleanup32KiB spec; original cutoff freezes centrally.
    pub(super) fn source_cleanup_spec(&self) -> JobSpec {
        Self::spec(self.source_cleanup_id, JobClass::InfrastructureLifecycle, 32_768, None)
    }
    /// Same independently preheld receiving cleanup32KiB spec.
    pub(super) fn receiving_cleanup_spec(&self) -> JobSpec {
        Self::spec(self.receiving_cleanup_id, JobClass::InfrastructureLifecycle, 32_768, None)
    }
}
