// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise production teardown admission with actual boundary facts, without native effects.

use super::*;

fn settled() -> ShutdownFacts {
    ShutdownFacts {
        closing: true,
        splash_retired: true,
        main_retired: true,
        execution_settled: true,
    }
}

#[test]
fn early_return_before_native_retirement_retains_loop_and_sta_custody() {
    let mut custody = LoopCustody::new();
    for facts in [
        ShutdownFacts { closing: false, ..settled() },
        ShutdownFacts { splash_retired: false, ..settled() },
        ShutdownFacts { main_retired: false, ..settled() },
        ShutdownFacts { splash_retired: false, main_retired: false, ..settled() },
    ] {
        assert!(!custody.may_exit(facts));
        let returned = custody.native_return(facts, 0);
        assert!(returned.failure.is_some());
        assert!(returned.teardown.is_none());
    }
    assert!(custody.may_exit(settled()));
}

#[test]
fn early_return_after_native_retirement_keeps_unsettled_workers_and_wake_owned() {
    let facts = ShutdownFacts { execution_settled: false, ..settled() };
    let mut custody = LoopCustody::new();
    assert!(!custody.may_exit(facts));
    for status in [0, 7, 0] {
        let returned = custody.native_return(facts, status);
        assert!(
            returned
                .failure
                .is_some_and(|cause| { cause.to_string().contains(&format!("status {status}")) })
        );
        assert!(returned.teardown.is_none());
    }
    assert!(custody.may_exit(settled()));
}

#[test]
fn actual_complete_settlement_permits_final_release_exactly_once()
-> Result<(), Box<dyn std::error::Error>> {
    let mut custody = LoopCustody::new();
    let returned = custody.native_return(settled(), 0);
    assert!(returned.failure.is_none());
    let mut releases = 0;
    returned.teardown.ok_or("actual settlement did not admit teardown")?.finish(|| releases += 1);
    assert_eq!(releases, 1);
    assert!(!custody.may_exit(settled()));
    let duplicate = custody.native_return(settled(), 0);
    assert!(duplicate.failure.is_some());
    assert!(duplicate.teardown.is_none());
    assert_eq!(releases, 1);
    Ok(())
}

#[test]
fn failed_native_status_preserves_cause_while_actual_settlement_permits_teardown()
-> Result<(), Box<dyn std::error::Error>> {
    let mut custody = LoopCustody::new();
    let mut original = Some(DesktopError::message("original_owner", "initial failure"));
    let returned = custody.native_return(settled(), 9);
    DesktopError::retain_first(
        &mut original,
        returned.failure.ok_or("native status cause missing")?,
    );
    assert_eq!(
        original.ok_or("original owner cause missing")?.to_string(),
        "original_owner: initial failure"
    );
    let mut releases = 0;
    returned.teardown.ok_or("actual retirement proof missing")?.finish(|| releases += 1);
    assert_eq!(releases, 1);
    Ok(())
}
