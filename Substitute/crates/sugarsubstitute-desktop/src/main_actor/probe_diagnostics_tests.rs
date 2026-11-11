// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe actual target transitions while unchanged incomplete results remain coalesced.
use super::*;
use crate::main_readiness_types::DocumentId;
use sugarsubstitute_contracts::{
    execution::Generation,
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    startup::LaunchId,
};

fn target() -> Result<ProbeTarget, Box<dyn std::error::Error>> {
    Ok(ProbeTarget {
        identity: MainIdentity {
            run: RunKey {
                owner: CoordinatorKey {
                    namespace: NamespaceId::new([1; 32]),
                    epoch: CoordinatorEpoch::new([2; 16])?,
                },
                instance: InstanceId::new(1)?,
                generation: RunGeneration::new(1)?,
            },
            launch: LaunchId::new(1)?,
            navigation: Generation::new(2)?,
        },
        operation: 1,
        context: 7,
        frame: DocumentId::new("frame")?,
        loader: DocumentId::new("loader")?,
    })
}

#[test]
fn unchanged_incomplete_callbacks_increment_counters_without_repeated_disposition_logs()
-> Result<(), Box<dyn std::error::Error>> {
    let mut state = ProbeDiagnostics::default();
    let mut target = target()?;
    let blocked = Blocked::new(2048).ok_or("Missing closed bit")?;
    let incomplete = Disposition::Incomplete(blocked);
    state.submitted(target);
    assert!(state.completed(target, incomplete));
    for operation in 2..=100 {
        target.operation = operation;
        state.submitted(target);
        assert!(!state.completed(target, incomplete));
    }
    let snapshot = state.record(target.identity, Stage::Timeout);
    assert_eq!((snapshot.issued, snapshot.completed, snapshot.blocked), (100, 100, 2048));
    assert_eq!((snapshot.launch, snapshot.navigation, snapshot.probe_operation), (1, 2, 100));
    target.operation += 1;
    state.submitted(target);
    let changed = Blocked::new(4096).ok_or("Missing closed bit")?;
    assert!(state.completed(target, Disposition::Incomplete(changed)));
    assert_eq!(state.record(target.identity, Stage::Timeout).blocked, 4096);
    Ok(())
}

#[test]
fn foreign_or_duplicate_completion_cannot_change_actual_diagnostic_custody()
-> Result<(), Box<dyn std::error::Error>> {
    let mut state = ProbeDiagnostics::default();
    let target = target()?;
    state.submitted(target);
    let mut foreign = target;
    foreign.identity.launch = LaunchId::new(3)?;
    assert!(!state.completed(foreign, Disposition::Fault));
    assert_eq!(state.record(target.identity, Stage::Timeout).completed, 0);
    assert!(state.completed(target, Disposition::Receipt));
    assert!(!state.completed(target, Disposition::Fault));
    let record = state.record(target.identity, Stage::Receipt);
    assert_eq!((record.issued, record.completed, record.blocked), (1, 1, 0));
    assert_eq!(state.last, Some(Disposition::Receipt));
    Ok(())
}
