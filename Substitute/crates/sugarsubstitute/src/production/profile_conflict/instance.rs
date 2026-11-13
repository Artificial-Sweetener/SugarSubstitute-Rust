// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve a running copy that did not take this launch, by its kernel-verified identity.
//!
//! A closing copy is waited for on its process handle; if it stops responding meanwhile the
//! user is asked. An unresponsive copy is ended only after the user chooses to close it and
//! start; its kill-on-close job ends the desktop and backend it started. Every operation runs
//! in an admitted blocking job and re-verifies the identity first.
use super::{super::entry_runner::PreviousCopy, PreviousCopyState};
use std::{
    error::Error,
    fmt,
    sync::{Arc, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::{
    execution::JobContext,
    instance_conflict::{ConflictBackend, ConflictObservation, ConflictOperation},
};
use sugarsubstitute_desktop::{ProcessError, ProcessResponsiveness};
use sugarsubstitute_supervision::{ClaimError, profile_conflict::Relaunch};

/// The longest single wait on the running copy's exit; the controller observes again after it.
/// The wait returns the moment the copy exits; the bound only limits how long a choice the
/// user makes meanwhile, such as "Close it and start", waits behind it.
const EXIT_WAIT_SLICE: Duration = Duration::from_secs(1);

/// The native operations on a verified running copy; tests substitute their own.
#[derive(Clone, Copy)]
pub(super) struct ProcessPort {
    pub(super) probe: fn(u32, u64) -> Result<ProcessResponsiveness, ProcessError>,
    pub(super) end: fn(u32, u64) -> Result<bool, ProcessError>,
    pub(super) wait: fn(u32, u64, Duration) -> Result<bool, ProcessError>,
}
const NATIVE: ProcessPort = ProcessPort {
    probe: sugarsubstitute_desktop::probe_process_responsiveness,
    end: sugarsubstitute_desktop::end_process,
    wait: sugarsubstitute_desktop::wait_for_process_exit,
};

/// Starts this application afresh once the running copy has gone; tests count instead.
pub(super) trait FreshLaunch: Send + Sync {
    /// Start the fresh launch inside an admitted blocking job.
    fn launch(&self, context: &JobContext) -> Result<(), ClaimError>;
    /// Retire the fresh launch's custody, if one was started.
    fn retire(&self) -> Result<(), ClaimError>;
}
impl FreshLaunch for Relaunch {
    fn launch(&self, context: &JobContext) -> Result<(), ClaimError> {
        Relaunch::launch(self, context)
    }
    fn retire(&self) -> Result<(), ClaimError> {
        Relaunch::retire(self)
    }
}

/// A failed operation on the running copy or the fresh launch, with its original cause.
#[derive(Debug)]
pub(super) enum InstanceConflictError {
    /// A native process operation failed.
    Process(ProcessError),
    /// Starting or retiring the fresh launch failed.
    Launch(ClaimError),
    /// The operation's job was cancelled or reached its deadline first.
    Expired,
    /// An operation this target never offers was requested.
    Unsupported,
}
impl fmt::Display for InstanceConflictError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Process(_) => "Running copy operation failed",
            Self::Launch(_) => "Fresh launch failed",
            Self::Expired => "Running copy operation expired",
            Self::Unsupported => "Running copy operation is not offered",
        })
    }
}
impl Error for InstanceConflictError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Process(cause) => Some(cause),
            Self::Launch(cause) => Some(cause),
            Self::Expired | Self::Unsupported => None,
        }
    }
}

#[derive(Default)]
struct State {
    ended: bool,
}

/// Shared custody for one running copy; clones operate on the same state.
#[derive(Clone)]
pub(super) struct InstanceSession {
    copy: PreviousCopy,
    reported: PreviousCopyState,
    port: ProcessPort,
    fresh: Arc<dyn FreshLaunch>,
    state: Arc<Mutex<State>>,
}
impl InstanceSession {
    /// Retain the verified copy and what it reported; nothing runs until a job executes.
    pub(super) fn new(copy: PreviousCopy, reported: PreviousCopyState) -> Self {
        Self::with_port(copy, reported, NATIVE, Arc::new(Relaunch::default()))
    }
    /// Retain the copy with substituted native operations and fresh launch.
    pub(super) fn with_port(
        copy: PreviousCopy,
        reported: PreviousCopyState,
        port: ProcessPort,
        fresh: Arc<dyn FreshLaunch>,
    ) -> Self {
        Self { copy, reported, port, fresh, state: Arc::default() }
    }
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    fn probe(&self) -> Result<ProcessResponsiveness, InstanceConflictError> {
        (self.port.probe)(self.copy.pid, self.copy.birth).map_err(InstanceConflictError::Process)
    }
    fn discover(&self) -> Result<ConflictObservation, InstanceConflictError> {
        // The coordinator answered for its desktop, which is another process; probing the
        // coordinator itself would not see that window.
        Ok(match (self.reported, self.probe()?) {
            (_, ProcessResponsiveness::Exited) => ConflictObservation::Released,
            (PreviousCopyState::Unresponsive, _) | (_, ProcessResponsiveness::Unresponsive) => {
                ConflictObservation::Unresponsive
            }
            (PreviousCopyState::Closing, ProcessResponsiveness::Responsive) => {
                ConflictObservation::Closing
            }
        })
    }
    fn observe(&self, context: &JobContext) -> Result<ConflictObservation, InstanceConflictError> {
        let remaining = context
            .deadline()
            .and_then(|until| until.checked_duration_since(context.now()).ok())
            .ok_or(InstanceConflictError::Expired)?;
        let slice = EXIT_WAIT_SLICE.min(remaining / 2);
        let (pid, birth) = (self.copy.pid, self.copy.birth);
        if (self.port.wait)(pid, birth, slice).map_err(InstanceConflictError::Process)? {
            return Ok(ConflictObservation::Released);
        }
        let ended = self.state().ended;
        if !ended && self.probe()? == ProcessResponsiveness::Unresponsive {
            return Ok(ConflictObservation::Unresponsive);
        }
        Ok(ConflictObservation::Waiting)
    }
}
impl ConflictBackend for InstanceSession {
    type Error = InstanceConflictError;
    fn execute(
        &self,
        operation: ConflictOperation,
        context: &JobContext,
    ) -> Result<ConflictObservation, InstanceConflictError> {
        if context.cancellation_requested()
            || context.deadline().is_none_or(|until| context.now() >= until)
        {
            return Err(InstanceConflictError::Expired);
        }
        match operation {
            ConflictOperation::Discover => self.discover(),
            ConflictOperation::Observe => self.observe(context),
            ConflictOperation::Force => {
                (self.port.end)(self.copy.pid, self.copy.birth)
                    .map_err(InstanceConflictError::Process)?;
                self.state().ended = true;
                Ok(ConflictObservation::Waiting)
            }
            ConflictOperation::Retire => Ok(ConflictObservation::Retired),
            ConflictOperation::Close => Err(InstanceConflictError::Unsupported),
        }
    }
    fn relaunch(&self, context: &JobContext) -> Result<(), InstanceConflictError> {
        self.fresh.launch(context).map_err(InstanceConflictError::Launch)
    }
    fn retire_relaunch(&self) -> Result<(), InstanceConflictError> {
        self.fresh.retire().map_err(InstanceConflictError::Launch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion_wake::Wake;
    use sugarsubstitute_contracts::instance_conflict::{
        ConflictAction, ConflictController, ConflictPhase,
    };

    const COPY: PreviousCopy = PreviousCopy { pid: 5, birth: 9 };
    fn responsive(_: u32, _: u64) -> Result<ProcessResponsiveness, ProcessError> {
        Ok(ProcessResponsiveness::Responsive)
    }
    fn hung(_: u32, _: u64) -> Result<ProcessResponsiveness, ProcessError> {
        Ok(ProcessResponsiveness::Unresponsive)
    }
    fn exited(_: u32, _: u64) -> Result<ProcessResponsiveness, ProcessError> {
        Ok(ProcessResponsiveness::Exited)
    }
    fn never_ends(_: u32, _: u64) -> Result<bool, ProcessError> {
        Ok(false)
    }
    fn still_running(_: u32, _: u64, _: Duration) -> Result<bool, ProcessError> {
        Ok(false)
    }
    fn port(probe: fn(u32, u64) -> Result<ProcessResponsiveness, ProcessError>) -> ProcessPort {
        ProcessPort { probe, end: never_ends, wait: still_running }
    }
    /// Counts fresh launches instead of starting a process.
    #[derive(Default)]
    struct Counted(std::sync::atomic::AtomicUsize);
    impl FreshLaunch for Counted {
        fn launch(&self, _: &JobContext) -> Result<(), ClaimError> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            Ok(())
        }
        fn retire(&self) -> Result<(), ClaimError> {
            Ok(())
        }
    }
    fn session(reported: PreviousCopyState, port: ProcessPort) -> (InstanceSession, Arc<Counted>) {
        let fresh = Arc::new(Counted::default());
        (InstanceSession::with_port(COPY, reported, port, fresh.clone()), fresh)
    }

    /// The copy's own report and a probe of it decide what is shown; an exited copy releases.
    #[test]
    fn discovery_waits_for_a_closing_copy_and_asks_about_an_unresponsive_one()
    -> Result<(), InstanceConflictError> {
        use PreviousCopyState::*;
        let cases = [
            (Closing, responsive as fn(u32, u64) -> _, ConflictObservation::Closing),
            (Closing, hung, ConflictObservation::Unresponsive),
            (Closing, exited, ConflictObservation::Released),
            (Unresponsive, responsive, ConflictObservation::Unresponsive),
            (Unresponsive, exited, ConflictObservation::Released),
        ];
        for (reported, probe, expected) in cases {
            let (session, _) = session(reported, port(probe));
            assert_eq!(session.discover()?, expected, "{reported:?}");
        }
        Ok(())
    }

    /// A closing copy is waited for with only Cancel offered, nothing is ended, and Cancel
    /// retires without a fresh launch.
    #[test]
    fn a_closing_copy_is_waited_for_and_cancel_never_ends_or_launches() {
        let (session, fresh) = session(PreviousCopyState::Closing, port(responsive));
        let observed = session.clone();
        let mut service = sugarsubstitute::instance_conflict::Service::new(session);
        let wake = Arc::new(Wake::default());
        service.start(wake.clone());
        let waiting = wake.until(|| {
            let view = service.poll();
            (view.phase != ConflictPhase::Checking).then_some(view)
        });
        assert_eq!(waiting.phase, ConflictPhase::AwaitingExit);
        assert!(!waiting.close_enabled() && !waiting.force_enabled());
        service.choose(ConflictAction::Close);
        service.choose(ConflictAction::Cancel);
        wake.until(|| service.poll().done().then_some(()));
        assert!(!service.continued());
        assert!(!observed.state().ended, "Waiting never ends the previous copy");
        assert_eq!(fresh.0.load(std::sync::atomic::Ordering::Acquire), 0);
    }
}
