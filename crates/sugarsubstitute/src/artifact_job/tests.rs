// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual artifact worker retention through cancellation and original deadline observation.
use super::*;
use crate::completion_wake::{Gate, Wake};
use std::sync::atomic::{AtomicU64, Ordering};

/// A clock the test moves by hand; its timers never fire.
#[derive(Default)]
struct Manual(AtomicU64);
impl Clock for Manual {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::SeqCst))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}

/// Release the held worker when the test unwinds, so a failed assertion cannot strand its join.
struct Release(Arc<Gate>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.release();
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
    let gate = Arc::new(Gate::default());
    let worker_gate = gate.clone();
    let clock = Arc::new(Manual::default());
    let mut lifetime = Lifetime::<()>::new(Kind::Record, clock.clone(), move |context| {
        let _original_memory = context.memory().claim(64 * 1024).map_err(WorkError::new)?;
        worker_gate.hold();
        Err(WorkError::new(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "original artifact failure",
        )))
    })?;
    // Declared after the lifetime so it drops first and frees the worker before the join.
    let _release = Release(gate.clone());
    let wake = Arc::new(Wake::default());
    lifetime.start(wake.clone())?;
    gate.wait_entered();
    let original_until = lifetime.until;
    let expired = if expire {
        clock.0.store(original_until.as_nanos(), Ordering::SeqCst);
        lifetime.activity(NonZeroUsize::MIN).is_err()
    } else {
        true
    };
    lifetime.request_shutdown()?;
    let retained = lifetime.job.is_some() && !lifetime.poll_shutdown()?;
    gate.release();
    wake.until(|| {
        let delivered = lifetime
            .activity(NonZeroUsize::MIN.saturating_add(31))
            .map(|_| lifetime.result.is_some().then_some(()));
        delivered.transpose()
    })?;
    assert!(expired && retained);
    assert_eq!(lifetime.until, original_until);
    let result = lifetime.result.take().ok_or("Artifact result missing")?;
    let cause = result.err().ok_or("Artifact failure was replaced by success")?;
    assert!(cause.to_string().contains("original artifact failure"), "{cause}");
    assert!(lifetime.job.is_none());
    // Test execution owns this off-UI join; production retains the native pump until settlement.
    drop(lifetime);
    Ok(())
}
