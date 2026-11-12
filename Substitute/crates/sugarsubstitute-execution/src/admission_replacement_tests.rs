// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Exercise ready-queue replacement against bounded deferred cleanup credits.

use super::*;

#[test]
fn full_ready_queue_replacement_reserves_credit_before_retiring_old_factory() -> TestResult {
    let rig = Rig::with_workers(1)?;
    let release = Arc::new(Gate::default());
    let mut io = Vec::new();
    for id in 1..=12 {
        let started = Arc::new(Gate::default());
        io.push(controlled_io(&rig, id, JobClass::IoLifecycle, release.clone(), started.clone())?);
        started.wait()?;
    }
    let cpu_release = Arc::new(Gate::default());
    let started = Arc::new(Gate::default());
    let worker_started = started.clone();
    let worker_release = cpu_release.clone();
    let mut cpu = vec![rig.client.submit_cpu(
        &rig.scope,
        spec(13, JobClass::CpuInteractive, 0)?,
        move |_| HoldingCpu { started: worker_started, release: worker_release },
    )?];
    started.wait()?;
    for id in 14..=40 {
        let worker_release = cpu_release.clone();
        cpu.push(rig.client.submit_cpu(
            &rig.scope,
            spec(id, JobClass::CpuDecode, 0)?,
            move |_| HoldingCpu { started: Arc::new(Gate::default()), release: worker_release },
        )?);
    }
    let worker_release = cpu_release.clone();
    cpu.push(rig.client.submit_cpu(
        &rig.scope,
        spec(41, JobClass::CpuInteractive, 0)?,
        move |_| HoldingCpu { started: Arc::new(Gate::default()), release: worker_release },
    )?);
    for id in 42..=45 {
        io.push(controlled_io(
            &rig,
            id,
            JobClass::IoLifecycle,
            release.clone(),
            Arc::new(Gate::default()),
        )?);
    }
    assert_eq!(rig.scope.snapshot().queued, 32);
    for id in 46..=48 {
        let worker_release = cpu_release.clone();
        cpu.push(rig.client.submit_cpu(
            &rig.scope,
            spec(id, JobClass::CpuInteractive, 0)?,
            move |_| HoldingCpu { started: Arc::new(Gate::default()), release: worker_release },
        )?);
    }
    assert_eq!(rig.scope.snapshot().queued, 35);
    let called = Arc::new(AtomicBool::new(false));
    let invoked = called.clone();
    let worker_release = cpu_release.clone();
    assert!(matches!(
        rig.client.submit_cpu(&rig.scope, spec(49, JobClass::CpuInteractive, 0)?, move |_| {
            invoked.store(true, Ordering::Release);
            HoldingCpu { started: Arc::new(Gate::default()), release: worker_release }
        }),
        Err(AdmissionError::CompletionSaturated)
    ));
    assert!(!called.load(Ordering::Acquire));
    cpu_release.open();
    release.open();
    for job in &mut io {
        assert!(matches!(rig.completion(job)?.outcome, JobOutcome::Success(())));
    }
    for job in &mut cpu {
        let completion = rig.completion(job)?;
        if matches!(job.spec().id.get(), 41 | 46 | 47) {
            assert!(
                matches!(completion.outcome, JobOutcome::Coalesced { replacement } if replacement.get() == match job.spec().id.get() {41 => 46, 46 => 47, _ => 48})
            );
        } else {
            assert!(matches!(completion.outcome, JobOutcome::Success(())));
        }
    }
    Ok(())
}
