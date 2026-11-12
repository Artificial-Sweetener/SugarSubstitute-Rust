// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Classify how a trial ended from observed facts, never from elapsed time.
//!
//! A trial is accepted only through authenticated readiness (`InstallationRecord::accept`). Every
//! other ending is classified here: only a failure of the new host itself is a bad verdict; every
//! other ending leaves the candidate prepared for the next launch.

/// Process whose failure was observed while a trial was starting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailedProcess {
    /// The new release's own host: its coordinator, main window or browser.
    Host,
    /// The ComfyUI backend, which fails for reasons that do not belong to the release.
    Backend,
}

/// Why an owner asked the trial host to close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseCause {
    /// The user or another ordinary close request, such as restarting to apply an update.
    Requested,
    /// Windows is ending the session (shutdown, restart or sign-out).
    SessionEnd,
}

/// Settlement outcome for a trial that was not accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrialVerdict {
    /// The new host itself failed; roll back and count a strike.
    Bad(BadTrial),
    /// Nothing proves the release bad; return the candidate to prepared.
    NoVerdict(Undecided),
}

/// Observed failure of the new host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BadTrial {
    /// The host reported its own startup failure before readiness.
    StartupFailed,
    /// The host process ended with an unhandled exception, fail-fast or panic, as its custody
    /// witness observed.
    Crashed,
    /// The host stopped responding and its custody watchdog ended it.
    Unresponsive,
}

/// Observed ending that is not the release's fault.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Undecided {
    /// The app was asked to close before any failure.
    CloseRequested,
    /// Windows ended the session before the trial finished.
    SessionEnd,
    /// The backend failed before the host reached readiness.
    BackendFailed,
    /// The host reached readiness, but its acceptance was never published.
    AcceptanceUnpublished,
    /// The host process was ended from outside, such as by Task Manager or a forced shutdown,
    /// without reporting a failure of its own.
    ProcessEnded,
}

/// Facts a trial's coordinator observes in order. The first ending fact wins: a failure after a
/// close request is a consequence of closing, and a close after a failure is its cleanup.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TrialFacts {
    readiness_reached: bool,
    ending: Option<Ending>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Ending {
    Close(CloseCause),
    Failure(FailedProcess),
}
impl TrialFacts {
    /// The host's own authenticated main, browser and backend readiness was observed.
    pub fn observe_readiness(&mut self) {
        self.readiness_reached = true;
    }
    /// An owner asked the app to close for `cause`.
    pub fn observe_close(&mut self, cause: CloseCause) {
        self.ending.get_or_insert(Ending::Close(cause));
    }
    /// `process` failed.
    pub fn observe_failure(&mut self, process: FailedProcess) {
        self.ending.get_or_insert(Ending::Failure(process));
    }
    /// Whether readiness was observed, after which no failure can make the trial bad.
    pub const fn readiness_reached(&self) -> bool {
        self.readiness_reached
    }
    /// Verdict for a trial settled without acceptance. A host that reached readiness was healthy
    /// even if acceptance never published; a session end is never the release's fault.
    pub fn verdict(&self) -> TrialVerdict {
        if self.readiness_reached {
            return TrialVerdict::NoVerdict(Undecided::AcceptanceUnpublished);
        }
        match self.ending {
            Some(Ending::Failure(FailedProcess::Host)) => {
                TrialVerdict::Bad(BadTrial::StartupFailed)
            }
            Some(Ending::Failure(FailedProcess::Backend)) => {
                TrialVerdict::NoVerdict(Undecided::BackendFailed)
            }
            Some(Ending::Close(CloseCause::SessionEnd)) => {
                TrialVerdict::NoVerdict(Undecided::SessionEnd)
            }
            Some(Ending::Close(CloseCause::Requested)) | None => {
                TrialVerdict::NoVerdict(Undecided::CloseRequested)
            }
        }
    }
}

/// Exit status the custody watchdog gives a coordinator it ends for not responding ("SSWD").
pub const UNRESPONSIVE_EXIT_STATUS: u32 = 0x5353_5744;
/// Exit status of a Rust process whose main thread panicked.
const PANIC_EXIT_STATUS: u32 = 101;

/// How the trial host's process ended, as its custody witness observed it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostExit {
    /// The host sent its authenticated finish before exiting.
    Finished,
    /// The host process ended with this exit status and never sent its finish.
    Ended(u32),
}
impl HostExit {
    /// Witness verdict for a trial still unsettled when its host exited. A crash is an exit
    /// status with NTSTATUS error severity (an unhandled exception or a fail-fast) or a Rust
    /// panic; the watchdog's status means the host hung. Any other status is an outside
    /// termination, such as Task Manager's, which is not the release's fault.
    pub const fn verdict(self) -> TrialVerdict {
        match self {
            Self::Finished => TrialVerdict::NoVerdict(Undecided::CloseRequested),
            Self::Ended(UNRESPONSIVE_EXIT_STATUS) => TrialVerdict::Bad(BadTrial::Unresponsive),
            Self::Ended(status) if status >> 30 == 0b11 || status == PANIC_EXIT_STATUS => {
                TrialVerdict::Bad(BadTrial::Crashed)
            }
            Self::Ended(_) => TrialVerdict::NoVerdict(Undecided::ProcessEnded),
        }
    }
}

#[cfg(test)]
mod tests;
