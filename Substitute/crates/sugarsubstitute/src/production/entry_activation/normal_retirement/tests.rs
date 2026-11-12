// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Substitute native receipt facts for classification; execution and output retirement remain real.
use super::*;
use sugarsubstitute_contracts::{
    execution::{Generation, Moment},
    lifecycle::*,
};
#[path = "test_support.rs"]
mod support;

fn record() -> Result<Record> {
    Ok(Record::new(
        MainIdentity {
            run: RunKey {
                owner: CoordinatorKey {
                    namespace: NamespaceId::new([3; 32]),
                    epoch: CoordinatorEpoch::new([4; 16])?,
                },
                instance: InstanceId::new(1)?,
                generation: RunGeneration::new(1)?,
            },
            launch: LaunchId::new(1)?,
            navigation: Generation::new(1)?,
        },
        ConnectionId::new(1)?,
        Default::default(),
        true,
        false,
    ))
}

/// This supplies an external native-boundary result, never production retirement authority.
fn boundary_receipt(record: &mut Record) -> Result<CompletedNormal> {
    let mut exit = sugarsubstitute_application::supervision::DesktopExitTransaction::new(
        record.run,
        ProcessKey { owner: record.run.owner, id: ProcessId::new(1)? },
    )?;
    exit.begin(
        record.run,
        ShutdownId::new(1)?,
        CompletionIntent::Normal,
        0,
        Moment::from_nanos(10),
    )?;
    let original = exit.normal_close_request().ok_or("Accepted normal request absent")?.budget();
    record.closed = true;
    record.retired = Some(retirement::RetiredDesktop::boundary_fixture(record.run, true));
    Ok(CompletedNormal {
        identity: MainIdentity {
            run: record.run,
            launch: record.launch,
            navigation: record.navigation,
        },
        connection: record.connection,
        original,
    })
}

#[test]
fn closed_normal_capsule_preserves_accepted_budget_without_native_owners() -> Result<()> {
    let mut record = record()?;
    let completed = boundary_receipt(&mut record)?;
    let original = completed.original;
    complete(&mut record, completed, None);
    assert!(record.start.is_none());
    assert!(record.parent.is_none());
    assert_eq!(ProductionActivation::normal_role_budget(&record), Some(original));
    assert!(
        ProductionActivation::overdue_role_budget(&record, Moment::from_nanos(u64::MAX)).is_none()
    );
    retire(&mut record)?;
    assert_eq!(ProductionActivation::normal_role_budget(&record), Some(original));
    Ok(())
}

#[test]
fn closed_normal_capsule_rejects_foreign_or_missing_retirement_and_new_duties() -> Result<()> {
    for change in 0..10 {
        let mut record = record()?;
        let completed = boundary_receipt(&mut record)?;
        complete(&mut record, completed, None);
        match change {
            0 => record.run.generation = RunGeneration::new(2)?,
            1 => record.launch = LaunchId::new(2)?,
            2 => record.navigation = Generation::new(2)?,
            3 => record.connection = ConnectionId::new(2)?,
            4 => record.closed = false,
            5 => record.retired = None,
            6 => {
                record.retired =
                    Some(retirement::RetiredDesktop::boundary_fixture(record.run, false))
            }
            7 => record.replied = false,
            8 => {
                record.failed_cause = Some(Box::new(std::io::Error::other("Original role failure")))
            }
            9 => record.failed_shutdown = Some(ShutdownBudget::new(Moment::from_nanos(11))?),
            _ => return Err("Unknown closed-role test mutation".into()),
        }
        assert!(ProductionActivation::normal_role_budget(&record).is_none(), "mutation {change}");
        assert!(super::super::normal_idle::completed_normal_budget(&record).is_none());
    }
    Ok(())
}

#[test]
fn closed_flag_or_boundary_receipt_alone_never_disposes_live_admitted_credit() -> Result<()> {
    let mut record = record()?;
    let mut rig = support::Rig::new()?;
    let mut attached = rig.attachment()?;
    rig.until(|| {
        rig.batch()?;
        let Some(done) = attached.try_take_completion() else { return Ok(false) };
        record.attached_memory = Some(match done.outcome {
            JobOutcome::Success(memory) => memory,
            _ => return Err("Actual funding did not complete".into()),
        });
        Ok(true)
    })?;
    drop(attached);
    rig.join();
    // No original live Parent/DesktopStart has passed the actual producer guard.
    let _boundary = boundary_receipt(&mut record)?;
    retire(&mut record)?;
    assert!(record.completed_normal.is_none());
    assert!(record.attached_memory.is_some());
    assert_eq!(rig.finite.snapshot().reserved_bytes, 16384);
    drop(record);
    assert_eq!(rig.finite.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn completed_payload_releases_real_seed_attachment_and_output_after_actual_worker_join()
-> Result<()> {
    let mut record = record()?;
    let mut rig = support::Rig::new()?;
    let mut output = rig.output(record.run)?;
    record.seed = rig.seed(record.run)?;
    let mut attached = Some(rig.attachment()?);
    rig.until(|| {
        let batch = rig.batch()?;
        record.seed.capture(&batch)?;
        if let Some(job) = &mut attached
            && let Some(done) = job.try_take_completion()
        {
            record.attached_memory = Some(match done.outcome {
                JobOutcome::Success(memory) => memory,
                _ => return Err("Actual attachment funding did not complete".into()),
            });
            attached = None;
        }
        output.observe(
            #[cfg(feature = "k1-native-qualification")]
            None,
        )?;
        Ok(attached.is_none() && record.seed.delivered() && output.settled())
    })?;
    assert!(matches!(record.seed.material(), Ok(Some(_))));
    assert_eq!(rig.finite.snapshot().reserved_bytes, 32768 + 16384);
    assert_eq!(rig.native.snapshot().reserved_bytes, support::OUTPUT_BYTES);
    assert_eq!(rig.external.closes(), 1);
    rig.join();
    assert_eq!(rig.finite.snapshot().reserved_bytes, 32768 + 16384);
    assert_eq!(rig.native.snapshot().reserved_bytes, support::OUTPUT_BYTES);
    let completed = boundary_receipt(&mut record)?;
    record.seeded = true;
    let original = completed.original;
    // Exercise disposal with native evidence substituted only at the receipt boundary.
    // The output queue, seed job, attachment funding and worker join above are genuine.
    complete(&mut record, completed, Some(output));
    assert_eq!(rig.finite.snapshot().reserved_bytes, 0);
    assert_eq!(rig.native.snapshot().reserved_bytes, 0);
    assert_eq!(ProductionActivation::normal_role_budget(&record), Some(original));
    assert!(record.retired.as_ref().is_some_and(|retired| retired.run() == record.run));
    Ok(())
}
