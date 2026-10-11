// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Supervise discarded results and nested panic-payload destruction.

use super::*;

struct ReadyWithDrop {
    value: Option<DropPanic>,
}
impl Future for ReadyWithDrop {
    type Output = Result<DropPanic, ()>;
    fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(self.value.take().ok_or(()))
    }
}
impl Drop for ReadyWithDrop {
    fn drop(&mut self) {
        std::panic::resume_unwind(Box::new("Future cleanup failed"));
    }
}

#[test]
fn produced_result_is_supervised_when_future_destruction_panics() -> TestResult {
    let rig = Rig::new()?;
    let destroyed = Arc::new(AtomicBool::new(false));
    let value = DropPanic(destroyed.clone());
    let mut job =
        rig.client.submit_async(&rig.scope, spec(1, JobClass::IoOrdinary, 0)?, move |_| {
            ReadyWithDrop { value: Some(value) }
        })?;
    let completion = rig.completion(&mut job)?;
    assert!(destroyed.load(Ordering::Acquire));
    assert!(
        matches!(completion.outcome, JobOutcome::Panicked(cause) if cause.as_str().contains("Future cleanup failed") && cause.as_str().contains("Owned destructor failed"))
    );
    Ok(())
}

#[test]
fn custom_panic_payload_destructor_cannot_escape_owned_panic_observation() -> TestResult {
    let destroyed = Arc::new(AtomicBool::new(false));
    let cause = crate::task::panic_cause(Box::new(DropPanic(destroyed.clone())));
    assert!(destroyed.load(Ordering::Acquire));
    assert!(cause.as_str().contains("Non-text panic payload"));
    assert!(cause.as_str().contains("destruction also panicked"));
    Ok(())
}

#[test]
fn abandoned_success_and_error_payload_destructor_failures_are_observed() -> TestResult {
    let rig = Rig::new()?;
    let success = Arc::new(AtomicBool::new(false));
    let error = Arc::new(AtomicBool::new(false));
    let value = DropPanic(success.clone());
    let failure = DropPanic(error.clone());
    let stamp = rig.wake.count();
    let first = rig.client.submit_async(
        &rig.scope,
        spec(1, JobClass::IoOrdinary, 1)?,
        move |_| async move { Ok::<_, ()>(value) },
    )?;
    rig.wake.wait_after(stamp)?;
    let _batch = rig.client.drain_completions(std::num::NonZeroUsize::MIN)?;
    let stamp = rig.wake.count();
    let second = rig.client.submit_async(
        &rig.scope,
        spec(2, JobClass::IoOrdinary, 1)?,
        move |_| async move { Err::<(), _>(failure) },
    )?;
    rig.wake.wait_after(stamp)?;
    drop(first);
    drop(second);
    drop(rig.owner);
    assert!(success.load(Ordering::Acquire) && error.load(Ordering::Acquire));
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    let cause = rig
        .client
        .drain_completions(std::num::NonZeroUsize::MIN)
        .err()
        .ok_or_else(|| std::io::Error::other("Disposal failures were lost"))?;
    assert!(cause.to_string().contains("2 owned execution teardown failures"));
    Ok(())
}
