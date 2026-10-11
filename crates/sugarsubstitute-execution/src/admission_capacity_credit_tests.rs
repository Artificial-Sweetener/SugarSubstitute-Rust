// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Preserve lifecycle reserves and coalesce credit-release activity with terminal wakes.

use super::*;

#[test]
fn lifecycle_credit_release_does_not_wake_an_ordinary_credit_waiter() -> TestResult {
    let rig = Rig::new()?;
    let mut jobs = Vec::new();
    for id in 1..=48 {
        let class = if id <= 44 { JobClass::IoOrdinary } else { JobClass::IoLifecycle };
        jobs.push(
            rig.client
                .submit_async(&rig.scope, spec(id, class, 0)?, |_| async { Ok::<_, ()>(()) })?,
        );
        loop {
            let observed = rig.wake.count();
            let batch = rig.client.drain_completions(NonZeroUsize::MIN)?;
            if !batch.notifications.is_empty() {
                break;
            }
            rig.wake.wait_after(observed)?;
        }
    }
    assert!(matches!(
        rig.client.submit_async(&rig.scope, spec(49, JobClass::IoOrdinary, 0)?, |_| async {
            Ok::<_, ()>(())
        }),
        Err(AdmissionError::CompletionSaturated)
    ));
    let before = rig.wake.count();
    drop(jobs[47].try_take_completion().ok_or("Lifecycle terminal missing")?);
    assert_eq!(rig.wake.count(), before);
    drop(jobs[0].try_take_completion().ok_or("Ordinary terminal missing")?);
    assert_eq!(rig.wake.count(), before + 1);
    drain(&rig)?;
    let mut accepted =
        rig.client.submit_async(&rig.scope, spec(49, JobClass::IoOrdinary, 0)?, |_| async {
            Ok::<_, ()>(())
        })?;
    let _completion = rig.completion(&mut accepted)?;
    Ok(())
}

#[test]
fn pending_terminal_wake_also_covers_the_later_memory_capacity_edge() -> TestResult {
    let rig = Rig::new()?;
    let context = retained_context(&rig, 1, LIMIT)?;
    drain(&rig)?;
    assert!(matches!(
        rig.client.submit_async(&rig.scope, spec(2, JobClass::IoLifecycle, 1)?, |_| async {
            Ok::<_, ()>(())
        }),
        Err(AdmissionError::MemorySaturated { .. })
    ));
    let before = rig.wake.count();
    let mut terminal =
        rig.client.submit_async(&rig.scope, spec(2, JobClass::IoLifecycle, 0)?, |_| async {
            Ok::<_, ()>(())
        })?;
    rig.wake.wait_after(before)?;
    let signalled = rig.wake.count();
    drop(context);
    wait_bytes(&rig, 0)?;
    assert_eq!(rig.wake.count(), signalled);
    let batch = rig.client.drain_completions(NonZeroUsize::MIN)?;
    assert_eq!(batch.notifications.len(), 1);
    assert_eq!(batch.notifications[0].job_id, JobId::new(2)?);
    let _completion = rig.completion(&mut terminal)?;
    Ok(())
}
