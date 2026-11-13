// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire successful native role capsules after their independent domain settlement.
use super::*;
use sugarsubstitute_contracts::startup::MainIdentity;

/// Preserve the exact accepted close after its complete native owners have retired.
/// Construction requires the original Parent, native/profile/output checks and the
/// backend's actual run settlement. The existing Record keeps its sealed native receipt.
pub(super) struct CompletedNormal {
    identity: MainIdentity,
    connection: ConnectionId,
    original: ShutdownBudget,
}

impl CompletedNormal {
    /// Revalidate retained matching data without manufacturing native or policy authority.
    pub(super) fn budget(
        &self,
        record: &Record,
    ) -> std::result::Result<ShutdownBudget, &'static str> {
        if self.identity.run != record.run
            || self.identity.launch != record.launch
            || self.identity.navigation != record.navigation
            || self.connection != record.connection
            || !record.closed
            || record
                .retired
                .as_ref()
                .is_none_or(|retired| retired.run() != record.run || !retired.registered())
        {
            return Err("CompletedNormalIdentity");
        }
        if record.start.is_some()
            || record.parent.is_some()
            || record.preparation.is_some()
            || record.attached_memory.is_some()
            || record.projected.is_some()
            || record.attach.is_some()
            || record.attach_failure.is_some()
            || record.released_attach.is_some()
            || record.attach_retirement.is_some()
            || record.forwarded.is_some()
            || !record.replied
            || record.failed.is_some()
            || record.failed_cause.is_some()
            || record.failed_shutdown.is_some()
            || record.failed_family_adopted
            || record.pending_desktop_failure.is_some()
            || record.collection.pending()
            || !record.collection.delivered()
            || record.seed.failure().is_some()
            || !record.seed.delivered()
        {
            return Err("CompletedNormalDuty");
        }
        Ok(self.original)
    }
}

/// Drop complete obsolete owners only after both actual settlement producers agree.
/// Failed/recovery roles retain their original archival and restart custody.
pub(super) fn retire(record: &mut Record) -> Result<()> {
    let Some(original) = super::normal_idle::completed_normal_budget(record) else {
        return Ok(());
    };
    if record.retained_causes.is_some() {
        return Err("Completed normal role duplicated its historical cause owner".into());
    }
    if let Some(parent) = &mut record.parent {
        record.retained_causes = parent.take_retired_backend_causes()?;
    }
    let completed = CompletedNormal {
        identity: MainIdentity {
            run: record.run,
            launch: record.launch,
            navigation: record.navigation,
        },
        connection: record.connection,
        original,
    };
    let output = record.start.as_mut().and_then(|start| start.output.take());
    complete(record, completed, output);
    Ok(())
}

/// Dispose complete native values after validation; never return a naked admission claim.
fn complete(
    record: &mut Record,
    completed: CompletedNormal,
    output: Option<super::desktop_output::DesktopOutput>,
) {
    // The Parent holds borrowed transport/profile facts; retire it before those
    // complete native owners. Data always dies before its original admission guard.
    drop(record.parent.take());
    drop(output);
    drop(record.start.take());
    drop(record.preparation.take());
    drop(record.profile_predecessor.take());
    drop(record.attached_memory.take());
    record.seed = incident_seed::SeedPreparation::new();
    record.completed_normal = Some(completed);
}

#[cfg(test)]
#[path = "normal_retirement/tests.rs"]
mod tests;
