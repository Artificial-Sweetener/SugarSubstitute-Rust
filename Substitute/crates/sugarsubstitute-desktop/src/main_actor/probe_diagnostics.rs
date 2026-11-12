// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Count actual probe custody and coalesce unchanged diagnostic dispositions.

pub(super) use crate::frontend_probe_diagnostic::Blocked;
use crate::{
    DesktopError, MainIdentity,
    frontend_probe_diagnostic::{Record, Stage},
    main_readiness_types::{ProbeTarget, ReadinessFault},
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Incomplete {
    incomplete: u16,
}

impl Blocked {
    /// Decode only a closed incomplete envelope; other shapes retain receipt validation.
    pub(super) fn decode(text: &str) -> Result<Option<Self>, DesktopError> {
        let Ok(value) = serde_json::from_str::<Incomplete>(text) else {
            return Ok(None);
        };
        Self::new(value.incomplete).map(Some).ok_or_else(|| {
            DesktopError::new("main_frontend_evidence", ReadinessFault::MalformedObservation)
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Disposition {
    Incomplete(Blocked),
    Receipt,
    Fault,
}

#[derive(Default)]
pub(super) struct ProbeDiagnostics {
    target: Option<ProbeTarget>,
    issued: u64,
    completed: u64,
    last: Option<Disposition>,
}
impl ProbeDiagnostics {
    /// Count only successful actual native submissions; emit the fast first one.
    pub(super) fn submitted(&mut self, target: ProbeTarget) {
        self.issued = target.operation;
        self.target = Some(target);
        if self.issued == 1 {
            crate::frontend_probe_diagnostic::emit(self.record(target.identity, Stage::Submitted));
        }
    }
    /// Consume only matching evaluation metadata; malformed foreign facts gain no attribution.
    pub(super) fn completed(&mut self, target: ProbeTarget, disposition: Disposition) -> bool {
        if self.target != Some(target) || self.completed == self.issued {
            return false;
        }
        self.completed = self.issued;
        if self.last != Some(disposition) {
            self.last = Some(disposition);
            let stage = match disposition {
                Disposition::Incomplete(_) => Stage::Incomplete,
                Disposition::Receipt => Stage::Receipt,
                Disposition::Fault => Stage::Fault,
            };
            crate::frontend_probe_diagnostic::emit(self.record(target.identity, stage));
            return true;
        }
        false
    }
    /// Observe counters and last fixed bits before the existing inclusive-timeout failure.
    pub(super) fn timeout(&self, identity: MainIdentity) {
        crate::frontend_probe_diagnostic::emit(self.record(identity, Stage::Timeout));
    }
    fn record(&self, identity: MainIdentity, stage: Stage) -> Record {
        Record {
            launch: identity.launch.get(),
            navigation: identity.navigation.get(),
            probe_operation: self.target.map_or(0, |target| target.operation),
            issued: self.issued,
            completed: self.completed,
            stage,
            blocked: match self.last {
                Some(Disposition::Incomplete(bits)) => bits.bits(),
                _ => 0,
            },
        }
    }
}

#[cfg(test)]
#[path = "probe_diagnostics_tests.rs"]
mod tests;
