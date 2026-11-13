// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the original report, registered startup and successor generation inseparable.
use super::RecoveryRejection;
use crate::{incident::ReportBinding, lifecycle::RunKey};
use std::num::NonZeroU64;
/// One once-only Root-issued reservation within its unchanged coordinator epoch.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RecoveryGrantId(NonZeroU64);
impl RecoveryGrantId {
    /// Validate a positive matching identity; this creates no admission or native privilege.
    /// # Errors
    /// Zero returns [`RecoveryRejection::Identity`].
    pub const fn new(value: u64) -> Result<Self, RecoveryRejection> {
        match NonZeroU64::new(value) {
            Some(value) => Ok(Self(value)),
            None => Err(RecoveryRejection::Identity),
        }
    }
    /// Return the matching identity without granting restart authority.
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}
/// Bind one delegation to its original registered run and exact acknowledged report view.
/// The successor always uses the same coordinator/instance and exactly the next generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryGrantKey {
    id: RecoveryGrantId,
    previous: RunKey,
    run: RunKey,
    report: ReportBinding,
}
impl RecoveryGrantKey {
    /// Derive exactly one successor without permitting a coordinator-epoch transfer.
    /// The report may be a recovered historical incident in the same installation namespace.
    /// # Errors
    /// Rejects a foreign report namespace or an exhausted source generation.
    pub fn new(
        id: RecoveryGrantId,
        previous: RunKey,
        report: ReportBinding,
    ) -> Result<Self, RecoveryRejection> {
        if report.incident().subject().coordinator().namespace != previous.owner.namespace {
            return Err(RecoveryRejection::Binding);
        }
        let generation = previous.generation.next().map_err(|_| RecoveryRejection::Binding)?;
        Ok(Self { id, previous, run: RunKey { generation, ..previous }, report })
    }
    /// Return the Root-issued reservation correlation.
    pub const fn id(self) -> RecoveryGrantId {
        self.id
    }
    /// Return the exact old run whose family and profile must retire independently.
    pub const fn previous(self) -> RunKey {
        self.previous
    }
    /// Return the fresh Desktop's same-owner successor run.
    pub const fn run(self) -> RunKey {
        self.run
    }
    /// Return the exact report session/revision that authorizes this request.
    pub const fn report(self) -> ReportBinding {
        self.report
    }
}
/// Actual process observations; constructing these facts cannot acquire or authenticate a handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryProcessIdentity {
    pid: u32,
    birth: u64,
}
impl RecoveryProcessIdentity {
    /// Validate nonzero PID and creation time in Windows100-ns units.
    /// # Errors
    /// Zero fields return [`RecoveryRejection::Identity`].
    pub const fn new(pid: u32, birth: u64) -> Result<Self, RecoveryRejection> {
        if pid == 0 || birth == 0 {
            return Err(RecoveryRejection::Identity);
        }
        Ok(Self { pid, birth })
    }
    /// Return the observed PID; actual retained handles prevent reuse races.
    pub const fn pid(self) -> u32 {
        self.pid
    }
    /// Return actual process creation time in Windows100-ns units.
    pub const fn birth(self) -> u64 {
        self.birth
    }
}
