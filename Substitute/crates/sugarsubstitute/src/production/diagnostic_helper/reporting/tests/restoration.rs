// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check passive restoration denies a real begun repository and retains its actual worker credit.
use super::*;
#[test]
fn reporting_passive_restore_rejects_actual_repository_and_missing_input_without_retirement()
-> TestResult {
    let (owner, wake, scope) = execution()?;
    let client = owner.client();
    let mut controller = client.submit_blocking(&scope, spec(1, 2 * 1024 * 1024)?, |context| {
        ReportingService::new(&context)
    })?;
    let service = success(completed(&client, &wake, &mut controller)?)?;
    let source = service.source.alias();
    assert!(source.restore_passive().is_err());
    assert!(!source.passive_replacement_ready());
    let mut replacement = source.alias();
    assert_eq!(replacement.replace_passive_generation(), Err(ReportingIssue::Busy));
    drop(replacement);
    assert!(service.passive_restoration().is_none());
    let active = source.alias();
    let mut work =
        client.submit_blocking(&scope, spec(2, DIAGNOSTIC_STORAGE_BYTES)?, move |context| {
            let fixture = DiagnosticStorageFixture::new()?;
            let mut loan = active.loan()?;
            *loan.operation()? = prepared(&context, &fixture)?;
            assert!(active.restore_passive().is_err());
            Ok::<_, TestError>(())
        })?;
    let done = completed(&client, &wake, &mut work)?;
    assert!(matches!(done.outcome, JobOutcome::Success(())));
    drop(done);
    assert!(source.restore_passive().is_err());
    assert_eq!(source.inspect(ReportingOperation::files_retired), Some(false));
    assert!(!source.passive_replacement_ready());
    assert!(service.passive_restoration().is_none());
    let retiring = source.alias();
    let mut cleanup = client.submit_blocking(&scope, spec(3, 32 * 1024)?, move |context| {
        let mut loan = retiring.loan()?;
        Ok::<_, TestError>(loan.operation()?.retire(&context))
    })?;
    let closed = completed(&client, &wake, &mut cleanup)?;
    assert!(matches!(closed.outcome, JobOutcome::Success(true)));
    assert_eq!(source.inspect(ReportingOperation::files_retired), Some(true));
    assert!(source.restore_passive().is_err());
    drop(owner);
    assert_eq!(
        scope.snapshot().reserved_bytes,
        2 * 1024 * 1024 + DIAGNOSTIC_STORAGE_BYTES + 32 * 1024
    );
    drop(controller);
    drop(work);
    drop(cleanup);
    drop(service);
    assert_eq!(
        scope.snapshot().reserved_bytes,
        2 * 1024 * 1024 + DIAGNOSTIC_STORAGE_BYTES + 32 * 1024
    );
    drop(source);
    assert_eq!(scope.snapshot().reserved_bytes, 32 * 1024);
    drop(closed);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
