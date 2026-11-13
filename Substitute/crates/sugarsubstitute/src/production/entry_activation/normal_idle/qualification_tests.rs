// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual idle-result preservation, veto classification and change suppression.
use super::*;
use sugarsubstitute_contracts::lifecycle::{CoordinatorEpoch, CoordinatorKey, NamespaceId};

fn activation() -> Result<Box<ProductionActivation>> {
    ProductionActivation::new(
        r"E:\application\SugarSubstitute.exe".into(),
        r"E:\application".into(),
        r"D:\captures".into(),
    )
}
#[test]
fn normal_idle_gate_preserves_result_and_first_global_veto() -> Result<()> {
    let mut owner = activation()?;
    assert!(owner.normal_close_budget().is_none());
    assert_eq!(owner.idle_gate.get().map(|gate| gate.reason), Some("MissingBackend"));
    owner.closing = true;
    owner.failure_run = Some(run()?);
    assert!(owner.normal_close_budget().is_none());
    assert_eq!(owner.idle_gate.get().map(|gate| gate.reason), Some("Closing"));
    owner.closing = false;
    assert!(owner.normal_close_budget().is_none());
    assert_eq!(owner.idle_gate.get().map(|gate| gate.reason), Some("FailureRun"));
    owner.failure_run = None;
    owner.preserve(Box::new(std::io::Error::other("Retained original failure")));
    assert!(owner.normal_close_budget().is_none());
    assert_eq!(owner.idle_gate.get().map(|gate| gate.reason), Some("FailureSource"));
    Ok(())
}
#[test]
fn normal_idle_gate_identical_snapshots_do_not_change_and_new_state_does() -> Result<()> {
    let mut owner = activation()?;
    let initial = owner.qualification_idle_gate(&owner.normal_close_decision());
    assert_ne!(owner.idle_gate.replace(Some(initial)), Some(initial));
    assert_eq!(owner.idle_gate.replace(Some(initial)), Some(initial));
    owner.closing = true;
    let closing = owner.qualification_idle_gate(&owner.normal_close_decision());
    assert_ne!(closing, initial);
    assert_ne!(owner.idle_gate.replace(Some(closing)), Some(closing));
    assert_eq!(owner.idle_gate.replace(Some(closing)), Some(closing));
    assert_eq!(
        owner
            .qualification_idle_gate(&Ok(ShutdownBudget::new(
                sugarsubstitute_contracts::execution::Moment::from_nanos(1)
            )?))
            .reason,
        "Eligible"
    );
    Ok(())
}
#[test]
fn normal_idle_gate_role_classifies_reply_then_parent_without_granting_close() -> Result<()> {
    let run = run()?;
    let mut record = Record::new(
        sugarsubstitute_contracts::startup::MainIdentity {
            run,
            launch: LaunchId::new(1)?,
            navigation: Generation::new(1)?,
        },
        ConnectionId::new(1)?,
        Default::default(),
        false,
        false,
    );
    assert_eq!(role_decision(&record).err(), Some("EntryReply"));
    record.replied = true;
    assert_eq!(role_decision(&record).err(), Some("MissingParent"));
    Ok(())
}
fn run() -> Result<RunKey> {
    Ok(RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    })
}
