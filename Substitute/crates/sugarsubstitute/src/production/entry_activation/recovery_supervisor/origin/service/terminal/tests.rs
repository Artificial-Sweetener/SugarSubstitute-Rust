// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject changed terminal identity using an actual execution-owned result and charge.
use super::super::super::test_support::*;
use super::*;
use crate::job_sequence::JobSequence;
#[test]
fn only_actual_scope_spec_kind_and_reservation_match_the_original_terminal() -> Result<(), TestError>
{
    let (_owner, client, wake, scope) = owner()?;
    let jobs = JobSequence::new();
    let original = spec(jobs.allocate()?, 32 * 1024);
    let mut job = client.submit_blocking(&scope, original, |_| Ok::<_, TestError>(()))?;
    let done = observe(&client, &wake, &mut job)?;
    assert!(matches(&done, scope.id(), original, Some(TerminalKind::Success)));
    assert!(!matches(&done, ScopeId::new(2)?, original, Some(TerminalKind::Success)));
    assert!(!matches(&done, scope.id(), original, None));
    let mut altered = original;
    altered.reservation_bytes += 1;
    assert!(!matches(&done, scope.id(), altered, Some(TerminalKind::Success)));
    let mut altered = original;
    altered.deadline = Some(Moment::from_nanos(101));
    assert!(!matches(&done, scope.id(), altered, Some(TerminalKind::Success)));
    Ok(())
}
