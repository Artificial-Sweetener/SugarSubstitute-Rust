// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Sample system memory at the budget's cadence and decide what the application's caches keep.
//!
//! The governor runs on the host's UI-thread poll: each sample is a nonblocking platform query.
//! It owns the budget state and the identities of the embedded browser's processes, whose
//! private bytes count toward the application footprint.

use std::{error::Error, time::Instant};
use sugarsubstitute_contracts::execution::Moment;
use sugarsubstitute_execution::memory_budget::{
    BudgetDecision, CacheCharge, MemoryBudget, MemoryPreference, SystemMemory,
};

/// One application-wide memory budget and the platform source it samples.
pub(super) struct MemoryGovernor {
    source: Box<dyn SystemMemory>,
    budget: MemoryBudget,
    browser_processes: Vec<u32>,
    due: Instant,
    decision: Option<BudgetDecision>,
}

impl MemoryGovernor {
    /// Govern the default preference with the platform sampler, together with the
    /// retained-document admission the canvas funds on this machine.
    ///
    /// # Errors
    /// Preserves the platform error when the sampler cannot open or read system memory.
    pub(super) fn open() -> Result<(Self, u64), Box<dyn Error>> {
        let governor = Self::new(
            Box::new(sugarsubstitute_execution::memory_budget::SystemMemorySampler::new()?),
            MemoryPreference::default(),
        );
        let funding =
            crate::native_canvas::budget::retained_document_funding(governor.total_bytes()?);
        Ok((governor, funding))
    }

    /// Govern with `preference`, sampling `source` on the first poll.
    pub(super) fn new(source: Box<dyn SystemMemory>, preference: MemoryPreference) -> Self {
        Self {
            source,
            budget: MemoryBudget::new(preference),
            browser_processes: Vec::new(),
            due: Instant::now(),
            decision: None,
        }
    }

    /// Installed physical memory, sampled now.
    ///
    /// # Errors
    /// Preserves the platform error when system memory cannot be read.
    pub(super) fn total_bytes(&self) -> Result<u64, Box<dyn Error>> {
        Ok(self.source.sample(&[])?.total_bytes)
    }

    /// When the next sample is due.
    pub(super) const fn next_deadline(&self) -> Instant {
        self.due
    }

    /// Sample and decide when due. Returns the decision when it differs from the previous one.
    ///
    /// # Errors
    /// Preserves the platform error when system memory cannot be read; the previous decision
    /// stays in force and the next sample is scheduled as usual.
    pub(super) fn poll(
        &mut self,
        now: Instant,
        moment: Moment,
        charge: CacheCharge,
    ) -> Result<Option<BudgetDecision>, Box<dyn Error>> {
        if now < self.due {
            return Ok(None);
        }
        // A failed sample still waits a full interval before retrying.
        self.due = now + self.budget.sample_interval();
        let sample = self.source.sample(&self.browser_processes)?;
        let decision = self.budget.observe(sample, charge, moment);
        self.due = now + self.budget.sample_interval();
        if self.decision == Some(decision) {
            return Ok(None);
        }
        let previous = self.decision.replace(decision);
        if previous.is_none_or(|previous| previous.level != decision.level) {
            tracing::info!(
                operation = "memory_budget_level",
                level = decision.level.as_str(),
                available_bytes = sample.available_bytes,
                footprint_bytes = sample.footprint_bytes,
                cache_bytes = decision.cache_bytes,
                ceiling_bytes = decision.ceiling_bytes,
                "Application memory pressure changed"
            );
        } else {
            tracing::debug!(
                operation = "memory_budget_allowance",
                level = decision.level.as_str(),
                cache_bytes = decision.cache_bytes,
                native_bytes = decision.native_bytes,
                page_bytes = decision.page_bytes,
                charged_bytes = charge.native_bytes + charge.page_bytes,
                "Application cache allowance changed"
            );
        }
        Ok(Some(decision))
    }
}

#[cfg(test)]
mod tests;
