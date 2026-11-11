// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain native state, copied allocations and first causes outside provider unwind.

use super::{cause::EntryEnvironmentCause, port::EnvironmentPort};

/// Observe the original capture's phase independently of its job's terminal state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryEnvironmentPhase {
    /// Stable custody exists before the one getter invocation.
    Prepared,
    /// The getter call has not yet returned to the owner.
    GetterInFlight,
    /// An actual original allocation remains owned.
    Held,
    /// Bounded original units are being inspected/copied.
    CopyInFlight,
    /// All copied units are retained; native release remains separate.
    CopyChecked,
    /// The sole Unicode Free attempt is in flight.
    FreeInFlight,
    /// Checked native Free returned successfully.
    Released,
    /// An observed failure requires owned cleanup.
    Retiring,
    /// No checked native release was observed; prohibit read/retry.
    RetirementUncertain,
    /// Original copied data were transferred to their retaining aggregate.
    Delivered,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CaptureDisposition {
    Copied,
    Failed,
    RetirementUncertain,
}

/// Keep actual resource/cause state under an external sealed original owner.
/// The admitted outer owner protects this allocation with its original full claim;
/// Core receives passive metadata only, never this provider or a native pointer.
pub(crate) struct NativeCapture {
    pub(super) port: Box<dyn EnvironmentPort>,
    pub(super) phase: EntryEnvironmentPhase,
    pub(super) inspected: usize,
    pub(super) units: Option<Vec<u16>>,
    pub(super) first: Option<EntryEnvironmentCause>,
    pub(super) secondary: [Option<EntryEnvironmentCause>; 8],
    pub(super) omitted: u64,
    pub(super) native_settled: bool,
}
impl NativeCapture {
    pub(crate) fn new(port: Box<dyn EnvironmentPort>) -> Self {
        Self {
            port,
            phase: EntryEnvironmentPhase::Prepared,
            inspected: 0,
            units: None,
            first: None,
            secondary: std::array::from_fn(|_| None),
            omitted: 0,
            native_settled: true,
        }
    }
    pub(crate) const fn phase(&self) -> EntryEnvironmentPhase {
        self.phase
    }
    #[cfg(test)]
    pub(crate) fn first_cause(&self) -> Option<&EntryEnvironmentCause> {
        self.first.as_ref()
    }
    pub(super) fn record(&mut self, cause: EntryEnvironmentCause) {
        if self.first.is_none() {
            self.first = Some(cause);
        } else if let Some(slot) = self.secondary.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(cause);
        } else {
            self.omitted = self.omitted.saturating_add(1);
        }
    }
    pub(super) fn disposition(&self) -> CaptureDisposition {
        if self.phase == EntryEnvironmentPhase::RetirementUncertain {
            CaptureDisposition::RetirementUncertain
        } else if self.first.is_some() {
            CaptureDisposition::Failed
        } else {
            CaptureDisposition::Copied
        }
    }
    /// Move units only within the sealed owner implementation after checked release.
    /// This internal operation does not transfer or release the outer full claim.
    pub(crate) fn take_units(&mut self) -> Option<Vec<u16>> {
        if self.phase != EntryEnvironmentPhase::Released || self.first.is_some() {
            return None;
        }
        let units = self.units.take()?;
        self.phase = EntryEnvironmentPhase::Delivered;
        Some(units)
    }
}
