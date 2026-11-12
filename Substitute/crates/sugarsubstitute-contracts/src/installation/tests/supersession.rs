// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Supersession changes pending selection without granting health or payload cleanup authority.
use super::*;

#[test]
fn pending_replacement_is_one_revision_and_preserves_accepted_history() -> Result {
    let accepted = install(&install(&empty()?, &generation(1)?)?, &generation(2)?)?;
    let old = generation(3)?;
    let next = generation(4)?;
    let staging = accepted.prepare(old.clone(), PreparationIntent::Automatic, NOW)?;
    for before in [staging.clone(), staging.prepared(&old)?] {
        let after = reload(&before.supersede_preparation(
            &old,
            next.clone(),
            PreparationIntent::Automatic,
            NOW,
        )?)?;
        assert_eq!(after.revision(), before.revision() + 1);
        assert_eq!(after.accepted(), before.accepted());
        assert_eq!(after.retained(), before.retained());
        assert_eq!(after.strikes(), before.strikes());
        assert_eq!(after.candidate(), Some(&Candidate::Staging { generation: next.clone() }));
        assert_eq!(after.startup_selection(), before.startup_selection());
    }
    Ok(())
}

#[test]
fn supersession_rejects_live_trials_stale_generations_and_reused_directories() -> Result {
    let old = generation(1)?;
    let next = generation(2)?;
    let prepared = prepare(&empty()?, &old)?;
    assert!(
        empty()?
            .supersede_preparation(&old, next.clone(), PreparationIntent::Automatic, NOW)
            .is_err()
    );
    assert!(
        prepared
            .begin_trial()?
            .supersede_preparation(&old, next.clone(), PreparationIntent::Automatic, NOW)
            .is_err()
    );
    assert!(
        prepared
            .supersede_preparation(&next, generation(3)?, PreparationIntent::Automatic, NOW)
            .is_err()
    );
    let reused = Generation {
        id: old.id.clone(),
        package: next.package.clone(),
        release: next.release.clone(),
    };
    assert!(
        prepared.supersede_preparation(&old, reused, PreparationIntent::Automatic, NOW).is_err()
    );
    let replacement =
        prepared.supersede_preparation(&old, next.clone(), PreparationIntent::Automatic, NOW)?;
    assert!(
        replacement
            .supersede_preparation(&old, generation(3)?, PreparationIntent::Automatic, NOW)
            .is_err()
    );
    Ok(())
}

#[test]
fn replacement_cannot_bypass_failed_package_suppression() -> Result {
    let failed = generation(1)?;
    let rejected = strike(&strike(&empty()?, &failed, NOW)?, &failed, NOW)?;
    let old = generation(2)?;
    let pending = rejected.prepare(old.clone(), PreparationIntent::Automatic, NOW)?;
    let retry =
        Generation { id: generation(3)?.id, package: failed.package, release: failed.release };
    assert_eq!(
        pending.supersede_preparation(&old, retry.clone(), PreparationIntent::Automatic, NOW),
        Err(Rejection::Quarantined)
    );
    let repaired = pending.supersede_preparation(&old, retry, PreparationIntent::Repair, NOW)?;
    assert_eq!(repaired.strikes(), pending.strikes());
    Ok(())
}
