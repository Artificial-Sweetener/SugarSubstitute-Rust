// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Shared real-executor observation and deterministic clock for updater lifetimes.
use std::{
    error::Error,
    io,
    sync::{
        Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionWake, ExecutionError, Moment, Operation, WaitFuture,
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Default)]
pub(crate) struct Wake(Mutex<u64>, Condvar);
impl CompletionWake for Wake {
    fn wake(&self) -> std::result::Result<(), ExecutionError> {
        let mut revision = self.0.lock().map_err(|_| {
            ExecutionError::new(
                Operation::Infrastructure,
                Box::new(io::Error::other("Update test wake poisoned")),
            )
        })?;
        *revision += 1;
        self.1.notify_all();
        Ok(())
    }
}
#[derive(Default)]
pub(crate) struct Manual(pub(crate) AtomicU64);
impl Clock for Manual {
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::SeqCst))
    }
}
/// Wait only on actual adapter notifications, with a bounded external observer cutoff.
pub(crate) fn observe(wake: &Wake, mut step: impl FnMut() -> Result<bool>) -> Result<()> {
    observe_until(wake, &mut step, true)
}

/// Active work must produce its own notification. Unlike closing-thread observation, a final
/// deadman poll must not conceal a missing wake that would strand the native event loop.
pub(crate) fn observe_notifications(
    wake: &Wake,
    mut step: impl FnMut() -> Result<bool>,
) -> Result<()> {
    observe_until(wake, &mut step, false)
}

/// Preserve wake revisions across each poll; only shutdown observation permits a final deadman poll.
fn observe_until(
    wake: &Wake,
    step: &mut impl FnMut() -> Result<bool>,
    final_poll: bool,
) -> Result<()> {
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        let revision = *wake.0.lock().map_err(|_| "Update wake poisoned")?;
        if step()? {
            return Ok(());
        }
        let remaining =
            until.checked_duration_since(Instant::now()).ok_or("Update observer expired")?;
        let (guard, timeout) = wake
            .1
            .wait_timeout_while(
                wake.0.lock().map_err(|_| "Update wake poisoned")?,
                remaining,
                |current| *current == revision,
            )
            .map_err(|_| "Update wake poisoned")?;
        drop(guard);
        if timeout.timed_out() {
            if final_poll && step()? {
                return Ok(());
            }
            return Err("Update observer expired".into());
        }
    }
}

/// Lexically valid input with no verified preparation role; native preparation admission rejects it.
pub(crate) fn untrusted_input() -> Result<sugarsubstitute_supervision::WindowsLaunchInput> {
    use sugarsubstitute_supervision::{
        LaunchEnvironment, WindowsLaunchCommand, WindowsLaunchInput,
    };
    let executable = std::env::current_exe()?;
    let command = WindowsLaunchCommand::from_os(
        &executable,
        executable.parent().ok_or("Test executable parent absent")?,
        &[],
    )?;
    Ok(WindowsLaunchInput::new(command, LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?)?)
}

/// Retain an actual producer's reservation after terminal delivery to deterministically exercise
/// capacity deferral. This reserves accounting only; it does not allocate 240 MiB of RAM.
pub(crate) fn retain_capacity(
    client: &sugarsubstitute_execution::ExecutionClient,
    scope: &sugarsubstitute_contracts::execution::Scope,
    id: sugarsubstitute_contracts::execution::JobId,
    wake: &Wake,
) -> Result<sugarsubstitute_contracts::execution::MemoryLease> {
    use sugarsubstitute_contracts::execution::*;
    let mut job = client.submit_blocking(
        scope,
        JobSpec {
            id,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureLifecycle,
            deadline: None,
            reservation_bytes: 240 * 1024 * 1024,
            replacement: None,
        },
        |context| Ok::<_, io::Error>(context.memory().clone()),
    )?;
    let mut retained = None;
    observe_notifications(wake, || {
        client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        if let Some(done) = job.try_take_completion() {
            let JobOutcome::Success(memory) = done.outcome else {
                return Err("Capacity fixture did not complete".into());
            };
            retained = Some(memory);
            return Ok(true);
        }
        Ok(false)
    })?;
    retained.ok_or_else(|| "Capacity fixture lost its reservation".into())
}
