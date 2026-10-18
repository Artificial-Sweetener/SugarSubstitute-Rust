// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve original publication controls while retaining an unsubmitted source.
use super::*;

fn staged() -> TestResult<Fixture> {
    let mut fixture = Fixture::new()?;
    fixture.active.work.input.as_mut().ok_or("Input missing")?.recover_unsubmitted = true;
    fixture.active.allocate(&fixture.core)?;
    assert!(fixture.active.work.input.is_some());
    assert!(fixture.active.source_input_claim.is_some());
    Ok(fixture)
}

#[test]
fn returned_unsubmitted_source_keeps_original_bytes_controls_and_memory() -> TestResult {
    let mut fixture = staged()?;
    let bytes = fixture.active.work.input.as_ref().ok_or("Input missing")?.request.units().to_vec();
    fixture.active.source_fence_for_return(&fixture.core)?;
    let recovered = fixture.session.control.take_unsubmitted_request().ok_or("Source lost")?;
    assert_eq!(recovered.input.request.units(), bytes);
    assert_eq!(recovered.input.source_setup, CounterDeadline::new(30_000_001_000, 1_000_000_000)?);
    assert!(
        recovered
            .input
            .request
            .original_owner()
            .is_some_and(|owner| owner.same_owner(&fixture.prepared.owner))
    );
    assert!(fixture.active.source_input_claim.is_none());
    assert!(fixture.active.work.input.is_none());
    fixture.core.close_scope(fixture.original.id());
    assert!(matches!(
        recovered.validate(
            &recovered.input.facts,
            None,
            recovered.input.source_setup,
            &crate::ExecutionClient { core: fixture.core.clone() },
            sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget::new(
                Moment::from_nanos(0)
            )?,
        ),
        Err(EntryTransportError::Preparation(_))
    ));
    Ok(())
}

#[test]
fn original_scope_revocation_vetoes_unsubmitted_source_publication() -> TestResult {
    let mut fixture = staged()?;
    fixture.core.close_scope(fixture.original.id());
    assert!(matches!(
        fixture.active.source_fence_for_return(&fixture.core),
        Err(EntryTransportError::Preparation(_))
    ));
    assert!(fixture.session.control.take_unsubmitted_request().is_none());
    assert!(fixture.active.source_input_claim.is_some());
    assert!(fixture.active.work.input.is_some());
    Ok(())
}
