// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! One rejected notice must not strand another genuine delivered preparation in the same batch.
use super::*;
#[test]
fn recycle_same_batch_identity_rejection_still_captures_actual_second_preparation() -> Result<()> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let first = spec()?;
    let second = JobSpec { id: JobId::new(2)?, ..first };
    let mut receiving = IncidentReporting::new();
    let preparation = receiving.receiving_session.preparing_mut()?;
    preparation.metadata.job = Some(client.submit_blocking(&scope, first, |context| {
        Err(delivery::Failure::test_cause(&context, "Original first preparation cause"))
    })?);
    preparation.launch.job = Some(client.submit_blocking(&scope, second, |context| {
        Err(delivery::Failure::test_cause(&context, "Original second preparation cause"))
    })?);
    let mut actual = CompletionBatch { notifications: Vec::new(), remaining: 0 };
    while !actual.notifications.iter().any(|notice| notice.job_id == first.id)
        || !actual.notifications.iter().any(|notice| notice.job_id == second.id)
    {
        let observed = batch(&client, &wake)?;
        actual.notifications.extend(observed.notifications);
        actual.remaining = observed.remaining;
    }
    let foreign = CompletionBatch {
        notifications: actual
            .notifications
            .iter()
            .map(|notice| {
                if notice.job_id == first.id {
                    CompletionNotification { kind: TerminalKind::Success, ..*notice }
                } else {
                    *notice
                }
            })
            .collect(),
        remaining: actual.remaining,
    };
    assert!(receiving.capture(&foreign).is_err());
    let preparation = receiving.receiving_session.preparing()?;
    assert!(preparation.metadata.value().is_err());
    assert!(preparation.metadata.completion.is_some());
    assert!(preparation.metadata.recyclable().is_err());
    assert!(preparation.launch.job.is_none());
    assert_eq!(preparation.launch.completion.as_ref().map(|done| done.spec), Some(second));
    let cause = preparation
        .launch
        .failure()
        .and_then(Error::source)
        .and_then(|cause| cause.downcast_ref::<std::io::Error>())
        .ok_or("Actual second preparation cause lost after first rejection")?;
    assert_eq!(cause.to_string(), "Original second preparation cause");
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 32768);
    drop(receiving);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
