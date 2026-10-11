// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve original inspection faults and typed fatal identity restoration failures.
use super::*;
#[test]
fn restored_identity_and_ordinary_rejection_remain_distinct_from_fatal_restore() {
    let identity = Identity::fixture(1, 2, 3);
    assert!(restored(Ok(Ok(identity)), Ok(())).is_ok());
    let rejected = restored(Ok(Err(failure("original_query"))), Ok(())).err();
    assert!(rejected.is_some_and(
        |e| !e.requires_process_shutdown() && e.native_cause().stage() == "original_query"
    ));
    let fatal = restored(Ok(Ok(identity)), Err(failure("actual_revert"))).err();
    assert!(fatal.is_some_and(
        |e| e.requires_process_shutdown() && e.native_cause().stage() == "actual_revert"
    ));
}
#[test]
fn query_failure_remains_first_when_identity_restore_also_fails()
-> Result<(), Box<dyn std::error::Error>> {
    let error = restored(Ok(Err(failure("original_query"))), Err(failure("actual_revert")))
        .err()
        .ok_or("Missing restore fault")?;
    assert!(error.requires_process_shutdown());
    assert!(!error.contains_owner_panic());
    assert_eq!(error.native_cause().stage(), "original_query");
    assert!(error.native_cause().to_string().contains("actual_revert"));
    Ok(())
}
#[test]
fn panic_and_failed_restore_preserve_fatal_facts_without_secret_debug()
-> Result<(), Box<dyn std::error::Error>> {
    let error = restored(
        Err(Box::new("Secret inspection panic".to_owned())),
        Err(failure("actual_revert")),
    )
    .err()
    .ok_or("Missing fatal panic")?;
    assert!(error.requires_process_shutdown());
    assert!(error.contains_owner_panic());
    assert!(!format!("{error:?}").contains("Secret"));
    assert!(!format!("{error}").contains("Secret"));
    Ok(())
}
