// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual artifact worker retention through cancellation and original deadline observation.
use super::*;
use std::sync::{Condvar, Mutex};

#[derive(Default)]
struct Wake(Mutex<u64>, Condvar);
impl CompletionWake for Wake {
    fn wake(&self) -> std::result::Result<(), ExecutionError> {
        let mut revision = self.0.lock().map_err(|cause| {
            ExecutionError::new(
                Operation::Infrastructure,
                Box::new(std::io::Error::other(cause.to_string())),
            )
        })?;
        *revision += 1;
        self.1.notify_all();
        Ok(())
    }
}

/// A real blocking worker must remain owned after a shutdown request until it actually returns.
#[test]
fn artifact_cancellation_preserves_unreturned_work_and_its_original_failure() -> Result {
    retained_failure(false)
}

/// Deadline observation must not replace the original producer cause or invent worker retirement.
#[test]
fn artifact_expiry_preserves_unreturned_work_and_its_original_failure() -> Result {
    retained_failure(true)
}

fn retained_failure(expire: bool) -> Result {
    let gate = Arc::new((Mutex::new((false, false)), Condvar::new()));
    let worker_gate = gate.clone();
    let mut lifetime = Lifetime::<()>::new(Kind::Record, move |context| {
        let _original_memory = context.memory().claim(64 * 1024).map_err(WorkError::new)?;
        let mut state = worker_gate
            .0
            .lock()
            .map_err(|_| WorkError::new(std::io::Error::other("Gate poisoned")))?;
        state.0 = true;
        worker_gate.1.notify_all();
        let (state, timeout) = worker_gate
            .1
            .wait_timeout_while(state, Duration::from_secs(20), |state| !state.1)
            .map_err(|_| WorkError::new(std::io::Error::other("Gate poisoned")))?;
        if timeout.timed_out() || !state.1 {
            return Err(WorkError::new(std::io::Error::other("Fixture release was not received")));
        }
        Err(WorkError::new(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "original artifact failure",
        )))
    })?;
    let wake = Arc::new(Wake::default());
    lifetime.start(wake.clone())?;
    let (state, timeout) = gate
        .1
        .wait_timeout_while(
            gate.0.lock().map_err(|_| "Gate poisoned")?,
            Duration::from_secs(20),
            |state| !state.0,
        )
        .map_err(|_| "Gate poisoned")?;
    let entered = state.0 && !timeout.timed_out();
    drop(state);
    let original_until = lifetime.until;
    let expired = if expire {
        lifetime.deadline = Instant::now();
        lifetime.activity(NonZeroUsize::MIN).is_err()
    } else {
        true
    };
    lifetime.request_shutdown()?;
    let retained = lifetime.job.is_some() && !lifetime.poll_shutdown()?;
    gate.0.lock().map_err(|_| "Gate poisoned")?.1 = true;
    gate.1.notify_all();
    let until = Instant::now() + Duration::from_secs(20);
    while lifetime.result.is_none() {
        let revision = *wake.0.lock().map_err(|_| "Wake poisoned")?;
        lifetime.activity(NonZeroUsize::MIN.saturating_add(31))?;
        if lifetime.result.is_some() {
            break;
        }
        let remaining = until
            .checked_duration_since(Instant::now())
            .ok_or("Artifact terminal not delivered")?;
        let (_guard, timeout) = wake
            .1
            .wait_timeout_while(wake.0.lock().map_err(|_| "Wake poisoned")?, remaining, |value| {
                *value == revision
            })
            .map_err(|_| "Wake poisoned")?;
        if timeout.timed_out() {
            return Err("Artifact terminal not delivered".into());
        }
    }
    assert!(entered && expired && retained);
    assert_eq!(lifetime.until, original_until);
    let result = lifetime.result.take().ok_or("Artifact result missing")?;
    let cause = result.err().ok_or("Artifact failure was replaced by success")?;
    assert!(cause.to_string().contains("original artifact failure"), "{cause}");
    assert!(lifetime.job.is_none());
    // Test execution owns this off-UI join; production retains the native pump until settlement.
    drop(lifetime);
    Ok(())
}
