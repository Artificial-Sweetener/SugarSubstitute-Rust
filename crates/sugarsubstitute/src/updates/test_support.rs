// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Shared real-executor observation and deterministic clock for updater lifetimes.
use std::{
    error::Error,
    io,
    sync::atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_contracts::execution::{Clock, Moment, WaitFuture};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub(crate) use crate::completion_wake::Wake;
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
/// Re-examine `step` after each actual adapter notification until it reports completion.
pub(crate) fn observe(wake: &Wake, mut step: impl FnMut() -> Result<bool>) -> Result<()> {
    wake.until(|| step().map(|done| done.then_some(())).transpose())
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
    observe(wake, || {
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
