// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe off-screen artifact completion without requiring a display server on portable hosts.
use super::*;
use std::sync::{Condvar, Mutex};
#[derive(Default)]
struct Wake(Mutex<u64>, Condvar);
impl CompletionWake for Wake {
    fn wake(&self) -> std::result::Result<(), ExecutionError> {
        let mut count = self.0.lock().map_err(|cause| {
            ExecutionError::new(
                Operation::Infrastructure,
                Box::new(std::io::Error::other(cause.to_string())),
            )
        })?;
        *count += 1;
        self.1.notify_all();
        Ok(())
    }
}
pub(super) fn run<T: Send + 'static>(lifetime: &mut Lifetime<T>) -> Result {
    let wake = Arc::new(Wake::default());
    let started = lifetime.start(wake.clone());
    let mut failure = started.err();
    if failure.is_some() {
        lifetime.request_shutdown()?;
    }
    loop {
        let observed = *wake.0.lock().map_err(|_| "Artifact wake poisoned")?;
        match lifetime.activity(NonZeroUsize::MIN.saturating_add(31)) {
            Ok(HeadlessStep::Shutdown) => lifetime.request_shutdown()?,
            Err(cause) => {
                if failure.is_none() {
                    failure = Some(cause);
                }
                lifetime.request_shutdown()?;
            }
            Ok(HeadlessStep::Active) => {}
        }
        if lifetime.closing {
            match lifetime.poll_shutdown() {
                Ok(true) => break,
                Ok(false) => {}
                Err(cause) => {
                    if failure.is_none() {
                        failure = Some(cause);
                    }
                }
            }
        }
        let wait = if lifetime.closing {
            Duration::from_millis(16)
        } else {
            lifetime.deadline.saturating_duration_since(Instant::now())
        };
        let (_guard, _) = wake
            .1
            .wait_timeout_while(
                wake.0.lock().map_err(|_| "Artifact wake poisoned")?,
                wait,
                |count| *count == observed,
            )
            .map_err(|_| "Artifact wake poisoned")?;
    }
    lifetime.finish_shutdown()?;
    match failure {
        Some(cause) => Err(cause),
        None => Ok(()),
    }
}
