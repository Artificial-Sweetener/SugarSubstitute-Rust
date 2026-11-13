// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the embedded page's caches and the browser's own memory inside the budget.
//!
//! The page reports what its caches hold and the user's stored preference; the link answers with
//! the page's allowance whenever it changes, asks the desktop for the browser's processes so
//! their memory counts toward the footprint, and asks a hidden browser to trim under pressure.

use std::{
    error::Error,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::{
    execution::{MemoryClaim, MemoryLease},
    memory::{MemoryAllowance, MemoryLevel, MemoryMessage, MemoryPreferenceChoice},
    startup::MainIdentity,
};
use sugarsubstitute_desktop::{DesktopStartupCommand, MemoryCommand, PanelIngressReceipt};
use sugarsubstitute_execution::memory_budget::{BudgetDecision, MemoryPreference, PressureLevel};

const MEASURE_INTERVAL: Duration = Duration::from_secs(10);

/// What the page last told the host about its caches.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct PageUsage {
    /// Bytes the page's caches hold.
    pub(super) used_bytes: u64,
    /// Bytes the page's caches would hold without a limit.
    pub(super) demand_bytes: u64,
}

/// The allowance values the page last received; an unchanged allowance is not resent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Sent {
    level: PressureLevel,
    page_bytes: u64,
    ceiling_bytes: u64,
    preference: MemoryPreference,
}

/// The current page navigation and the host-direction work it is owed.
#[derive(Default)]
pub(super) struct PageLink {
    page: Option<(MainIdentity, MemoryLease)>,
    usage: PageUsage,
    generation: u64,
    sent: Option<Sent>,
    browser_low: bool,
    measure_due: Option<Instant>,
    receipt: Option<PanelIngressReceipt>,
    allowance: Option<MemoryCommand>,
    target: Option<(MainIdentity, bool)>,
    measure: Option<MainIdentity>,
}

impl PageLink {
    /// Consume one page report; returns the preference it carries, if any.
    ///
    /// # Errors
    /// Rejects an allocation too small for the control-lane receipt.
    pub(super) fn observe(
        &mut self,
        identity: MainIdentity,
        message: MemoryMessage,
        lease: MemoryLease,
        claim: MemoryClaim,
    ) -> Result<Option<MemoryPreference>, Box<dyn Error>> {
        // Consuming the report returns its control-lane credit to the page.
        self.receipt = Some(PanelIngressReceipt::control(identity, claim)?);
        if self.page.as_ref().is_none_or(|(current, _)| *current != identity) {
            self.sent = None;
            self.browser_low = false;
            self.measure_due = Some(Instant::now());
        }
        self.page = Some((identity, lease));
        Ok(match message {
            MemoryMessage::Usage { used_bytes, demand_bytes, .. } => {
                self.usage = PageUsage { used_bytes, demand_bytes };
                None
            }
            MemoryMessage::Preference { preference, .. } => Some(match preference {
                MemoryPreferenceChoice::Share { percent } => MemoryPreference::Share(percent),
                MemoryPreferenceChoice::Fixed { bytes } => MemoryPreference::Fixed(bytes),
            }),
        })
    }

    /// The page's latest reported usage.
    pub(super) const fn usage(&self) -> PageUsage {
        self.usage
    }

    /// Forget a page whose navigation retired; its pending work can no longer be delivered.
    pub(super) fn retire(&mut self, identity: MainIdentity) {
        if self.page.as_ref().is_some_and(|(current, _)| *current == identity) {
            *self = Self { generation: self.generation, ..Self::default() };
        }
    }

    /// Queue the allowance, browser target and process measurement `decision` calls for.
    ///
    /// # Errors
    /// Rejects allowance storage the page's lease cannot fund, or an unencodable allowance.
    pub(super) fn apply(
        &mut self,
        decision: BudgetDecision,
        preference: MemoryPreference,
        total_bytes: u64,
        hidden: bool,
        now: Instant,
    ) -> Result<(), Box<dyn Error>> {
        let Some((identity, lease)) = &self.page else {
            return Ok(());
        };
        let identity = *identity;
        let sent = Sent {
            level: decision.level,
            page_bytes: decision.page_bytes,
            ceiling_bytes: decision.ceiling_bytes,
            preference,
        };
        if self.sent != Some(sent) {
            self.generation += 1;
            let allowance = MemoryAllowance {
                generation: self.generation,
                level: level(decision.level),
                allowance_bytes: decision.page_bytes,
                ceiling_bytes: decision.ceiling_bytes,
                total_bytes,
                preference: match preference {
                    MemoryPreference::Share(percent) => MemoryPreferenceChoice::Share { percent },
                    MemoryPreference::Fixed(bytes) => MemoryPreferenceChoice::Fixed { bytes },
                },
            };
            let claim = lease.claim(MemoryCommand::STORAGE_BYTES)?;
            self.allowance = Some(MemoryCommand::new(identity, &allowance, claim)?);
            self.sent = Some(sent);
        }
        // Only a hidden browser trims: a visible one would page its own working set back in.
        let low = hidden && decision.level >= PressureLevel::High;
        if low != self.browser_low {
            self.browser_low = low;
            self.target = Some((identity, low));
        }
        if self.measure_due.is_some_and(|due| now >= due) {
            self.measure = Some(identity);
            self.measure_due = Some(now + MEASURE_INTERVAL);
        }
        Ok(())
    }

    /// Move the next owed host-direction command: receipts first, then the latest allowance.
    pub(super) fn take_command(&mut self) -> Option<DesktopStartupCommand> {
        if let Some(receipt) = self.receipt.take() {
            return Some(DesktopStartupCommand::CompletePanelIngress(receipt));
        }
        if let Some(allowance) = self.allowance.take() {
            return Some(DesktopStartupCommand::SendMemory(allowance));
        }
        if let Some((identity, low)) = self.target.take() {
            return Some(DesktopStartupCommand::SetBrowserMemoryTarget { identity, low });
        }
        self.measure.take().map(DesktopStartupCommand::MeasureBrowser)
    }
}

const fn level(level: PressureLevel) -> MemoryLevel {
    match level {
        PressureLevel::Normal => MemoryLevel::Normal,
        PressureLevel::Elevated => MemoryLevel::Elevated,
        PressureLevel::High => MemoryLevel::High,
        PressureLevel::Critical => MemoryLevel::Critical,
    }
}

#[cfg(test)]
mod tests;
