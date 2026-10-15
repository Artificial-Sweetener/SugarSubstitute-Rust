// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Shape-only retained cleanup actors; no native family, claim or settlement proof.
use super::*;

/// Retain two actual cleanup states under the same original charged backend owner.
pub(crate) fn retained_cleanup(
    scopes: [Scope; 2],
    budgets: [ShutdownBudget; 2],
) -> Result<RetainedBackend> {
    let mut held = tests::unadmitted_policy()?;
    for (at, (budget, scope)) in budgets.into_iter().zip(scopes).enumerate() {
        let key = ResourceKey {
            owner: held.backend.owner,
            id: sugarsubstitute_contracts::lifecycle::ResourceId::new(at as u64 + 1)?,
            generation: sugarsubstitute_contracts::lifecycle::ResourceGeneration::new(1)?,
        };
        let mut record = cutoff_tests::creator(
            key,
            BackendTarget::Attached(LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?),
            ConnectionBudget::new(ConnectionBudgetKind::Attached, budget.started)?,
            scope,
        )?;
        record.shutdown = Some((ShutdownId::new(at as u64 + 1)?, budget));
        held.backend.records[at] = Some(record);
    }
    Ok(held)
}
