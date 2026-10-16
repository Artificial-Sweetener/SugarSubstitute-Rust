// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Inspect private final-control ordering only with the genuine fixture's admitted grant and lease.
use super::RegisteredRecoveryGrant;
use sugarsubstitute_contracts::execution::{MemoryLease, Scope};
/// Keep private payload extraction inside its owner; no payload, raw alias or authority escapes.
/// The existing actual Entry/projection fixture remains responsible for creation and final shutdown.
pub(in crate::coordinator_entry) fn assert_final_payload_custody(
    mut grant: RegisteredRecoveryGrant,
    memory: &MemoryLease,
    scope: &Scope,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = 4 * 1024 * 1024;
    assert_eq!(memory.bytes(), bytes);
    let mut alias = grant.clone();
    assert!(grant.same_grant(&alias));
    assert_eq!(grant.key(), alias.key());
    assert_eq!(grant.startup_bytes(), alias.state()?.startup_body().len());
    let mut custody = alias.clone().into_custody();
    let mut final_custody = custody.clone();
    assert_eq!(size_of::<super::RecoveryGrantCustody>(), size_of::<usize>());
    assert!(
        matches!((&alias.owner, &custody.0), (Some(a), Some(b)) if std::sync::Arc::ptr_eq(a, b))
    );
    let before = scope.snapshot().reserved_bytes;
    assert!(grant.take_owner().is_none());
    assert!(grant.check().is_err());
    assert!(!grant.same_grant(&alias));
    alias.check()?;
    assert!(alias.take_owner().is_none());
    assert!(alias.check().is_err());
    assert!(custody.take_owner().is_none());
    let payload = final_custody.take_owner().ok_or("Final genuine grant payload missing")?;
    // The final control is freed while its extracted payload still owns the original4096 claim.
    let free = memory.claim(bytes - 4096)?;
    assert!(memory.claim(1).is_err());
    drop(free);
    assert_eq!(scope.snapshot().reserved_bytes, before);
    drop(payload);
    let all = memory.claim(bytes)?;
    assert!(memory.claim(1).is_err());
    drop(all);
    drop(grant);
    drop(alias);
    drop(custody);
    drop(final_custody);
    Ok(())
}
