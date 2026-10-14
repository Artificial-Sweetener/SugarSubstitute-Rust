// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Move one genuine native/control primary cause into the retained journal outcome.
use super::{
    control::ControlCause,
    observation::{Failure, Origin},
    token::Issue,
};
use std::{any::Any, error::Error, fmt, sync::Mutex};
use sugarsubstitute_contracts::execution::TimeError;
use sugarsubstitute_supervision::diagnostic_storage::DiagnosticStorageError;

/// Original unwind is moved into an inline synchronization cell without another Box.
struct PanicSource(Mutex<Box<dyn Any + Send>>);
impl fmt::Debug for PanicSource {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("FinishedSettlementPanic")
    }
}
impl fmt::Display for PanicSource {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, out)
    }
}
impl Error for PanicSource {}
enum Cause {
    Native { issue: Issue, origin: Origin, code: u32 },
    Clock { issue: Issue, source: TimeError },
    Source { issue: Issue, source: DiagnosticStorageError },
    Panic { issue: Issue, source: PanicSource },
    Control { issue: Issue, category: ControlCategory },
    Invalid { issue: Issue, source: super::observation::InvalidResult },
    Rejected(Issue),
}
#[derive(Debug)]
enum ControlCategory {
    Cancelled,
    TimedOut,
    MissingDeadline,
    ForeignClock,
}
impl fmt::Debug for Cause {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Native { origin, code, .. } => out
                .debug_struct("FinishedNative")
                .field("origin", origin)
                .field("code", code)
                .finish(),
            Self::Clock { .. } => out.write_str("FinishedClock"),
            Self::Source { .. } => out.write_str("FinishedSource"),
            Self::Panic { .. } => out.write_str("FinishedPanic"),
            Self::Control { category, .. } => fmt::Debug::fmt(category, out),
            Self::Invalid { .. } => out.write_str("FinishedInvalidResult"),
            Self::Rejected(issue) => fmt::Debug::fmt(issue, out),
        }
    }
}
impl fmt::Display for Cause {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, out)
    }
}
impl Error for Cause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Clock { source, .. } => Some(source),
            Self::Source { source, .. } => Some(source),
            Self::Panic { source, .. } => Some(source),
            Self::Invalid { source, .. } => Some(source),
            _ => None,
        }
    }
}
pub(crate) struct SettlementFailure {
    cause: Cause,
}
const _: () = assert!(size_of::<SettlementFailure>() <= 128);
impl SettlementFailure {
    pub(super) fn new(issue: Issue, cause: Option<Failure>) -> Self {
        let cause = cause.map_or(Cause::Rejected(issue), |failure| match failure {
            Failure::Native { origin, code } => Cause::Native { issue, origin, code },
            Failure::Invalid(source) => Cause::Invalid { issue, source },
            Failure::Control(control) => match control {
                ControlCause::Clock(source) => Cause::Clock { issue, source },
                ControlCause::Source(source) => Cause::Source { issue, source },
                ControlCause::Panic(payload) => {
                    Cause::Panic { issue, source: PanicSource(Mutex::new(payload)) }
                }
                ControlCause::Cancelled => {
                    Cause::Control { issue, category: ControlCategory::Cancelled }
                }
                ControlCause::TimedOut => {
                    Cause::Control { issue, category: ControlCategory::TimedOut }
                }
                ControlCause::MissingDeadline => {
                    Cause::Control { issue, category: ControlCategory::MissingDeadline }
                }
                ControlCause::ForeignClock => {
                    Cause::Control { issue, category: ControlCategory::ForeignClock }
                }
            },
        });
        Self { cause }
    }
    pub(crate) fn inspect_panic<T>(
        &self,
        inspect: impl FnOnce(&(dyn Any + Send)) -> T,
    ) -> Option<T> {
        let Cause::Panic { source, .. } = &self.cause else {
            return None;
        };
        let guard = source.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        Some(inspect(guard.as_ref()))
    }
}
impl fmt::Debug for SettlementFailure {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let issue = match &self.cause {
            Cause::Native { issue, .. }
            | Cause::Clock { issue, .. }
            | Cause::Source { issue, .. }
            | Cause::Panic { issue, .. }
            | Cause::Control { issue, .. }
            | Cause::Invalid { issue, .. }
            | Cause::Rejected(issue) => issue,
        };
        out.debug_struct("FinishedSettlement").field("issue", issue).finish()
    }
}
impl fmt::Display for SettlementFailure {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, out)
    }
}
impl Error for SettlementFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}
