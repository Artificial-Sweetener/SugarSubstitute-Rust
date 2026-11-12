// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise bounded routing decisions with inert transport facts, not native success proofs.
use super::*;
use std::{cell::Cell, collections::VecDeque};
use sugarsubstitute_contracts::{
    coordinator_entry::{EntryConnectionId, EntryRejection, EntryRequestId},
    execution::TerminalKind,
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
};

fn run() -> Result<RunKey, Box<dyn Error>> {
    Ok(RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(3)?,
        generation: RunGeneration::new(4)?,
    })
}

#[test]
fn terminal_repoll_adopts_late_actual_outcome_at_end_of_full_bounded_batch()
-> Result<(), Box<dyn Error>> {
    let run = run()?;
    let request = EntryRequestId::new(17)?;
    let connection = EntryConnectionId::new(9)?;
    let mut observations = VecDeque::new();
    assert!(observations.pop_front().is_none());
    // The independent terminal can become observable after the initial intake
    // poll. The late batch includes the exact closed outcome at its final slot.
    for _ in 0..31 {
        observations.push_back(EntryTransportObservation::Prepared);
    }
    observations.push_back(EntryTransportObservation::OutcomeReceived {
        connection,
        request,
        outcome: EntryOutcome::Accepted(run),
    });
    let polls = Cell::new(0);
    let completed = reconcile_initial(Some(TerminalKind::Success), false, || {
        polls.set(polls.get() + 1);
        let mut adopted = false;
        for _ in 0..32 {
            let Some(fact) = observations.pop_front() else { break };
            if let EntryTransportObservation::OutcomeReceived {
                connection: actual_connection,
                request: actual_request,
                outcome,
            } = fact
            {
                assert_eq!(actual_connection, connection);
                assert_eq!(actual_request, request);
                assert_eq!(outcome, EntryOutcome::Accepted(run));
                adopted = initial_outcome(outcome, Some(run.owner), Some(run.owner))?;
            }
        }
        Ok(adopted)
    })?;
    assert!(completed);
    assert_eq!(polls.get(), 1);
    assert!(observations.is_empty());
    assert!(!premature_transport(1, Some(TerminalKind::Success), completed));
    let without_outcome = reconcile_initial(Some(TerminalKind::Success), false, || Ok(false))?;
    assert!(!without_outcome);
    assert!(premature_transport(1, Some(TerminalKind::Success), without_outcome));
    Ok(())
}

#[test]
fn accepted_fact_never_hides_failure_server_terminal_or_foreign_owner() -> Result<(), Box<dyn Error>>
{
    let run = run()?;
    let adopted = initial_outcome(EntryOutcome::Accepted(run), Some(run.owner), Some(run.owner))?;
    let polls = Cell::new(0);
    let completed = reconcile_initial(Some(TerminalKind::Failure), adopted, || {
        polls.set(polls.get() + 1);
        Ok(true)
    })?;
    assert_eq!(polls.get(), 0);
    assert!(premature_transport(1, Some(TerminalKind::Failure), completed));
    assert!(premature_transport(0, Some(TerminalKind::Success), completed));
    assert!(premature_transport(2, Some(TerminalKind::Success), completed));
    let foreign = CoordinatorKey { epoch: CoordinatorEpoch::new([8; 16])?, ..run.owner };
    let cause = initial_outcome(EntryOutcome::Accepted(run), Some(foreign), Some(foreign))
        .err()
        .ok_or("Foreign full owner was adopted")?;
    assert!(matches!(
        cause.downcast_ref::<EntryTransportError>(),
        Some(EntryTransportError::Authentication)
    ));
    let cause = initial_outcome(
        EntryOutcome::Rejected(EntryRejection::InvalidInput),
        Some(run.owner),
        Some(run.owner),
    )
    .err()
    .ok_or("Rejected request was adopted")?;
    assert!(matches!(
        cause.downcast_ref::<Rejected>(),
        Some(Rejected(EntryRejection::InvalidInput))
    ));
    Ok(())
}
