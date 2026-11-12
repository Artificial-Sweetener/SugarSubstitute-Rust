// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Start a generation's launch engine and observe whether it takes over startup.
//!
//! The stable entry and a redirecting engine start a pinned engine with
//! `--launch-engine-v1 <installation> <event> <hops>`. That spelling, its one inherited manual-
//! reset event and the rule that the engine signals the event as soon as it owns startup, before
//! any verification work, are permanent cross-generation surface. The creator waits only for the
//! event or the engine's exit, bounded in seconds; a silent engine that never took over is
//! terminated by its creator.
use super::*;
use crate::setup_storage::{PinnedEngine, PointerFailure};
use std::{
    ffi::OsString,
    io,
    os::windows::io::{AsHandle, AsRawHandle, FromRawHandle, OwnedHandle},
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::{application_launch::ENGINE_LAUNCH_ARGUMENT, execution::Moment};
use sugarsubstitute_execution::handle_wait::{HandleWaitError, wait_any};
use windows::Win32::{
    Foundation::{HANDLE, HANDLE_FLAG_INHERIT, SetHandleInformation},
    System::Threading::{CreateEventW, SetEvent, TerminateProcess},
};

/// Largest hop count: the root starts at 0; a redirected or fallback engine runs at 1 and never
/// redirects again.
pub const MAX_ENGINE_HOPS: u8 = 1;

/// How a started engine ended the creator's wait.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Takeover {
    /// The engine signalled that it owns startup.
    TookOver,
    /// The engine exited without taking over.
    Exited,
    /// The engine neither signalled nor exited before the bound; it has been terminated.
    Silent,
}

/// A created engine process and the event it signals once it owns startup.
pub struct StartedEngine {
    process: OwnedHandle,
    event: OwnedHandle,
}

/// Create the pinned engine with exactly one inherited event. Keep `engine` pinned until return.
/// Run on an admitted blocking infrastructure job.
/// # Errors
/// Native event, command or creation failures; no process exists after an error.
pub fn start_engine(
    engine: &PinnedEngine,
    installation: &Path,
    hops: u8,
    context: &JobContext,
) -> Result<StartedEngine, ClaimError> {
    start_engine_image(engine.executable(), engine.directory(), installation, hops, context)
}

/// Start an already verified, pinned engine image, such as a newer accepted updater, with the
/// engine argument. The caller keeps the image's pins until this returns.
/// # Errors
/// Native event, command or creation failures; no process exists after an error.
pub fn start_engine_image(
    executable: &Path,
    directory: &Path,
    installation: &Path,
    hops: u8,
    context: &JobContext,
) -> Result<StartedEngine, ClaimError> {
    if hops > MAX_ENGINE_HOPS {
        return Err(ClaimError::invalid("launch_engine_hops"));
    }
    spawn(executable, directory, context, |event| {
        vec![
            ENGINE_LAUNCH_ARGUMENT.into(),
            installation.as_os_str().to_owned(),
            event.to_string().into(),
            hops.to_string().into(),
        ]
    })
}

/// Create `executable` inheriting only a fresh takeover event whose value `arguments` receives.
pub(super) fn spawn(
    executable: &Path,
    directory: &Path,
    context: &JobContext,
    arguments: impl FnOnce(usize) -> Vec<OsString>,
) -> Result<StartedEngine, ClaimError> {
    if context.cancellation_requested() {
        return Err(ClaimError::invalid("launch_engine_admission"));
    }
    // SAFETY: a private unnamed manual-reset event; OwnedHandle adopts it immediately.
    let event = unsafe { CreateEventW(None, true, false, None) }
        .map_err(|cause| ClaimError::new("launch_engine_event", cause))?;
    // SAFETY: the event was just created and has no other owner.
    let event = unsafe { OwnedHandle::from_raw_handle(event.0) };
    inheritable(&event, true)?;
    let arguments = arguments(event.as_raw_handle() as usize);
    let command = WindowsLaunchCommand::from_os(executable, directory, &arguments)
        .map_err(|cause| ClaimError::new("launch_engine_command", cause))?;
    let mut buffers = command.into_native_parts();
    let mut process = PROCESS_INFORMATION::default();
    let created = creation::engine(&mut buffers, event.as_raw_handle() as usize, &mut process);
    // Adopt every output before checking the status that produced them.
    let thread = (!process.hThread.is_invalid())
        // SAFETY: CreateProcessW returned this thread handle and nothing else owns it.
        .then(|| unsafe { OwnedHandle::from_raw_handle(process.hThread.0) });
    let child = (!process.hProcess.is_invalid())
        // SAFETY: CreateProcessW returned this process handle and nothing else owns it.
        .then(|| unsafe { OwnedHandle::from_raw_handle(process.hProcess.0) });
    drop(thread);
    created?;
    inheritable(&event, false)?;
    let pid = process.dwProcessId;
    let process = child.ok_or_else(|| ClaimError::invalid("launch_engine_process"))?;
    tracing::info!(
        operation = "launch_engine_created",
        pid,
        "Started a generation's launch engine"
    );
    Ok(StartedEngine { process, event })
}

impl StartedEngine {
    /// Wait for the engine to take over or exit, no later than `until` on the job's clock.
    /// An engine still silent at `until` is terminated and its exit observed before returning.
    /// Run on an admitted blocking infrastructure job.
    /// # Errors
    /// Cancellation and native wait or termination failures.
    pub fn await_takeover(
        &self,
        context: &JobContext,
        until: Moment,
    ) -> Result<Takeover, ClaimError> {
        match wait_any(context, &[self.event.as_handle(), self.process.as_handle()], until) {
            Ok(0) => Ok(Takeover::TookOver),
            Ok(_) => Ok(Takeover::Exited),
            Err(HandleWaitError::Timeout) => {
                // SAFETY: this creator owns the live process handle; the engine never took over.
                unsafe { TerminateProcess(HANDLE(self.process.as_raw_handle()), 1) }
                    .map_err(|cause| ClaimError::new("launch_engine_terminate", cause))?;
                let settled = until
                    .checked_add(ENGINE_TAKEOVER)
                    .map_err(|cause| ClaimError::new("launch_engine_termination_bound", cause))?;
                wait_any(context, &[self.process.as_handle()], settled)
                    .map_err(|cause| ClaimError::new("launch_engine_terminated", cause))?;
                Ok(Takeover::Silent)
            }
            Err(cause) => Err(ClaimError::new("launch_engine_wait", cause)),
        }
    }
}

fn inheritable(event: &OwnedHandle, inherit: bool) -> Result<(), ClaimError> {
    // SAFETY: the owned event handle is live for this call.
    unsafe {
        SetHandleInformation(
            HANDLE(event.as_raw_handle()),
            HANDLE_FLAG_INHERIT.0,
            if inherit { HANDLE_FLAG_INHERIT } else { Default::default() },
        )
    }
    .map_err(|cause| ClaimError::new("launch_engine_inheritance", cause))
}

/// Bound on one engine's takeover; engines signal before any verification work.
pub const ENGINE_TAKEOVER: std::time::Duration = std::time::Duration::from_secs(10);

/// Why the stable entry could not hand startup to any engine.
#[derive(Debug)]
pub enum StartupRefused {
    /// The launch pointer is missing, damaged or incompatible.
    Pointer(PointerFailure),
    /// Every pointed engine was refused; each generation is listed with its cause, in order.
    NoEngine(Vec<(String, ClaimError)>),
}
impl std::fmt::Display for StartupRefused {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pointer(failure) => write!(formatter, "{failure}"),
            Self::NoEngine(refused) => {
                formatter.write_str("No launch engine took over startup")?;
                for (generation, cause) in refused {
                    write!(formatter, "; {generation}: {cause}")?;
                }
                Ok(())
            }
        }
    }
}
impl std::error::Error for StartupRefused {}

/// Start the pointer's primary engine, then its fallback, returning once one takes over.
/// The primary runs with hop 0; the fallback runs with hop 1 so it never redirects. Each engine
/// gets `ENGINE_TAKEOVER`; a silent one is terminated before the next starts. Run on one admitted
/// blocking infrastructure job reserving `setup_storage::POINTER_RESERVATION_BYTES`.
/// # Errors
/// The pointer's failure, or every engine's refusal.
pub fn start_pointed(installation: &Path, context: &JobContext) -> Result<(), StartupRefused> {
    start_pointed_with(installation, context, ENGINE_TAKEOVER, |engine, hops| {
        start_engine(engine, installation, hops, context)
    })
}

/// Share the real pointer, pinning and takeover policy with tests that substitute the child.
pub(super) fn start_pointed_with(
    installation: &Path,
    context: &JobContext,
    takeover: std::time::Duration,
    start: impl Fn(&PinnedEngine, u8) -> Result<StartedEngine, ClaimError>,
) -> Result<(), StartupRefused> {
    let pointer = crate::setup_storage::read_launch_pointer(installation, context)
        .map_err(StartupRefused::Pointer)?;
    let mut refused = Vec::new();
    let targets = std::iter::once((pointer.primary(), 0))
        .chain(pointer.fallback().map(|target| (target, MAX_ENGINE_HOPS)));
    for (target, hops) in targets {
        let attempt = (|| {
            let engine = crate::setup_storage::pin_engine(installation, target, context)
                .map_err(|cause| ClaimError::new("launch_engine_pin", cause))?;
            let started = start(&engine, hops)?;
            drop(engine);
            let until = context
                .now()
                .checked_add(takeover)
                .map_err(|cause| ClaimError::new("launch_engine_takeover_bound", cause))?;
            match started.await_takeover(context, until)? {
                Takeover::TookOver => Ok(()),
                Takeover::Exited => Err(ClaimError::invalid("launch_engine_exited")),
                Takeover::Silent => Err(ClaimError::invalid("launch_engine_silent")),
            }
        })();
        match attempt {
            Ok(()) => return Ok(()),
            Err(cause) => {
                tracing::warn!(
                    operation = "stable_entry_engine_refused",
                    generation = target.generation().as_str(),
                    hops,
                    cause = %cause,
                    "Launch engine did not take over"
                );
                refused.push((target.generation().as_str().to_owned(), cause));
            }
        }
    }
    Err(StartupRefused::NoEngine(refused))
}

/// Startup handed to this process by a stable entry or a redirecting engine.
#[derive(Debug, Eq, PartialEq)]
pub struct EngineLaunch {
    installation: PathBuf,
    hops: u8,
}
impl EngineLaunch {
    /// Installation the creator named; the engine verifies it contains its own generation.
    pub fn installation(&self) -> &Path {
        &self.installation
    }
    /// 0 when started by the stable entry's primary start; 1 when redirected or a fallback.
    pub fn hops(&self) -> u8 {
        self.hops
    }
}

/// Capture the engine entry, signal "took over" at once and close the event.
/// Other entries return `None` untouched; an unknown engine spelling is refused.
/// # Safety
/// Call once at executable entry before any local handle owner exists. The event slot must be
/// uniquely unowned and protected from concurrent closure or reuse.
/// # Errors
/// Malformed or unsupported arguments and an event that cannot be signalled.
pub unsafe fn capture_engine_launch(
    arguments: impl IntoIterator<Item = OsString>,
) -> io::Result<Option<EngineLaunch>> {
    let Some((launch, raw)) = parse(arguments)? else { return Ok(None) };
    // SAFETY: the initial-entry caller guarantees unique ownership of this inherited slot.
    let event = unsafe { OwnedHandle::from_raw_handle(raw as *mut std::ffi::c_void) };
    inheritable(&event, false).map_err(io::Error::other)?;
    // SAFETY: the adopted event handle is live for this call.
    unsafe { SetEvent(HANDLE(event.as_raw_handle())) }.map_err(io::Error::other)?;
    Ok(Some(launch))
}

fn parse(
    arguments: impl IntoIterator<Item = OsString>,
) -> io::Result<Option<(EngineLaunch, usize)>> {
    let mut arguments = arguments.into_iter();
    let Some(prefix) = arguments.next() else { return Ok(None) };
    if prefix != ENGINE_LAUNCH_ARGUMENT {
        if prefix.to_str().is_some_and(|value| value.starts_with("--launch-engine-")) {
            return Err(io::Error::other("Unsupported launch engine protocol"));
        }
        return Ok(None);
    }
    let installation = PathBuf::from(
        arguments.next().ok_or_else(|| io::Error::other("Launch engine installation missing"))?,
    );
    if !installation.is_absolute() || installation.parent().is_none() {
        return Err(io::Error::other("Launch engine installation is not absolute"));
    }
    let mut number = || -> io::Result<String> {
        arguments
            .next()
            .and_then(|value| value.into_string().ok())
            .ok_or_else(|| io::Error::other("Launch engine argument missing"))
    };
    let event = number()?;
    let raw =
        event.parse::<usize>().map_err(|_| io::Error::other("Launch engine event invalid"))?;
    let hops = number()?;
    let parsed = hops.parse::<u8>().map_err(|_| io::Error::other("Launch engine hops invalid"))?;
    if raw == 0
        || raw == usize::MAX
        || raw.to_string() != event
        || parsed > MAX_ENGINE_HOPS
        || parsed.to_string() != hops
        || arguments.next().is_some()
    {
        return Err(io::Error::other("Launch engine arguments are invalid"));
    }
    Ok(Some((EngineLaunch { installation, hops: parsed }, raw)))
}

#[cfg(test)]
mod tests;
