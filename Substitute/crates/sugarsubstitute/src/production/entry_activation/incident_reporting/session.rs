// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep preparation failures or a checked detached receipt within the same fixed metadata role.
use super::*;
#[path = "session/retirement.rs"]
mod retirement;
/// Original preparation results may retain failures and their actual draft source aliases.
pub(super) struct ReporterPreparation {
    pub(super) metadata: delivery::Delivery<delivery::Material>,
    pub(super) launch: delivery::Delivery<delivery::PreparedLaunch>,
}
/// Accepted original Dismiss close interval; construction follows actual source disposal.
#[derive(Clone, Copy)]
struct AcceptedReporterDismiss {
    binding: ReportBinding,
    budget: ShutdownBudget,
}
struct OriginalReporterRetirement {
    binding: ReportBinding,
    _source: ReporterSource,
    _process: ProcessKey,
    family: Option<sugarsubstitute_supervision::SettledRegistryFailure>,
    registered: bool,
}
/// Recycled source stays closed until registry removal and Domain acceptance both finish.
pub(super) struct RecycledReporterRetirement {
    pub(super) binding: ReportBinding,
    key: DiagnosticKey,
    pub(super) startup: sugarsubstitute_execution::RegisteredStartupSource,
    pub(super) receipt: Option<RecycledReporter>,
    pub(super) registered: bool,
}
enum ReporterRetirement {
    Original(OriginalReporterRetirement),
    Recycled(RecycledReporterRetirement),
}
/// Retain failed preparation alongside original retirement and one accepted Dismiss history.
/// Healthy recycle releases actual preparation owners before installing its checked receipt.
pub(super) struct ReporterSession {
    preparation: ReporterPreparation,
    retirement: Option<ReporterRetirement>,
    dismissed: Option<AcceptedReporterDismiss>,
}
enum RetirementBorrow<'a> {
    Original(&'a OriginalReporterRetirement),
    Recycled(&'a RecycledReporterRetirement),
}
/// Borrow only genuine completed family/pipe retirement from this receiving owner.
pub(in crate::production) struct RetiredReporter<'a> {
    original: RetirementBorrow<'a>,
}
impl RetiredReporter<'_> {
    /// The exact retired binding; no copied facts can construct this sealed owner view.
    pub(in crate::production) fn binding(&self) -> ReportBinding {
        match self.original {
            RetirementBorrow::Original(original) => original.binding,
            RetirementBorrow::Recycled(original) => original.binding,
        }
    }
}
impl ReporterSession {
    pub(super) const fn new() -> Self {
        Self {
            preparation: ReporterPreparation {
                metadata: delivery::Delivery::new(),
                launch: delivery::Delivery::new(),
            },
            retirement: None,
            dismissed: None,
        }
    }
    pub(super) fn recycled(&self) -> Option<&RecycledReporterRetirement> {
        match &self.retirement {
            Some(ReporterRetirement::Recycled(value)) => Some(value),
            _ => None,
        }
    }
    pub(super) fn recycled_mut(&mut self) -> Option<&mut RecycledReporterRetirement> {
        match &mut self.retirement {
            Some(ReporterRetirement::Recycled(value)) => Some(value),
            _ => None,
        }
    }
    pub(super) fn preparing(&self) -> Result<&ReporterPreparation> {
        if self.recycled().is_some() {
            return Err("Reporter receipt remains retained".into());
        }
        Ok(&self.preparation)
    }
    pub(super) fn preparing_mut(&mut self) -> Result<&mut ReporterPreparation> {
        if self.recycled().is_some() {
            return Err("Reporter receipt remains retained".into());
        }
        Ok(&mut self.preparation)
    }
    pub(super) fn delivered(&self) -> bool {
        self.recycled().is_some()
            || self.preparation.metadata.delivered() && self.preparation.launch.delivered()
    }
    pub(super) fn cancel(&self) {
        if self.recycled().is_none() {
            self.preparation.metadata.cancel();
            self.preparation.launch.cancel();
        }
    }
    pub(super) fn capture(&mut self, batch: &CompletionBatch) -> Result<()> {
        if self.recycled().is_none() {
            let metadata = self.preparation.metadata.capture(batch);
            let launch = self.preparation.launch.capture(batch);
            metadata?;
            launch?;
        }
        Ok(())
    }
    pub(super) fn failure(&self) -> Option<&(dyn Error + 'static)> {
        self.preparation.metadata.failure().or_else(|| self.preparation.launch.failure()).or_else(
            || match &self.retirement {
                Some(ReporterRetirement::Original(original)) => {
                    original.family.as_ref().map(|cause| cause as &(dyn Error + 'static))
                }
                _ => None,
            },
        )
    }
    pub(super) fn retire_healthy_preparation(
        &mut self,
        binding: ReportBinding,
        key: DiagnosticKey,
    ) -> Result<()> {
        let value = self.preparing()?;
        value.metadata.recyclable()?;
        value.launch.recyclable()?;
        let startup = value
            .launch
            .value()?
            .ok_or("Actual matched Reporter preparation missing")?
            .startup()
            .clone();
        if self.retirement.is_some() {
            return Err("Original retirement remains retained".into());
        }
        self.preparation = ReporterPreparation {
            metadata: delivery::Delivery::new(),
            launch: delivery::Delivery::new(),
        };
        self.retirement = Some(ReporterRetirement::Recycled(RecycledReporterRetirement {
            binding,
            key,
            startup,
            receipt: None,
            registered: false,
        }));
        Ok(())
    }
    pub(super) fn retired(&self) -> Option<RetiredReporter<'_>> {
        match &self.retirement {
            Some(ReporterRetirement::Original(original)) if original.registered => {
                Some(RetiredReporter { original: RetirementBorrow::Original(original) })
            }
            Some(ReporterRetirement::Recycled(original))
                if original.registered && original.receipt.is_some() =>
            {
                Some(RetiredReporter { original: RetirementBorrow::Recycled(original) })
            }
            _ => None,
        }
    }
    /// Borrow only the latest already accepted original Dismiss close interval.
    pub(super) fn normal_close_budget(&self) -> Option<ShutdownBudget> {
        let original = self.dismissed.as_ref()?;
        let _binding = original.binding;
        Some(original.budget)
    }
    /// Validate the complete accepted boundary while the original receipt is still owned.
    pub(super) fn validate_disposal(&self) -> Result<ReportBinding> {
        let Some(original) = self.recycled() else {
            return Err("Actual checked Reporter receipt missing".into());
        };
        let receipt = original.receipt.as_ref().ok_or("Original Reporter receipt missing")?;
        let binding = original.binding;
        let expected = original.key;
        if expected.role() != DiagnosticRole::Reporter
            || expected.owner().namespace != binding.incident().subject().coordinator().namespace
            || expected.session().get() != binding.session().get()
        {
            return Err("Actual current Reporter role differs from historical data".into());
        }
        if !original.registered || receipt.key() != expected {
            return Err("Actual accepted Reporter correlation changed".into());
        }
        Ok(binding)
    }
    /// Validate and dispose the whole receipt in one receiving transition.
    /// Busy preserves the original receipt; no fallible adoption follows checked source disposal.
    pub(super) fn finish_disposal(
        &mut self,
        relay: &mut ReporterRelay,
        dismiss: Option<ShutdownBudget>,
    ) -> Result<bool> {
        let binding = self.validate_disposal()?;
        let Some(original) = self.recycled_mut() else {
            return Err("Actual checked source disposal missing".into());
        };
        let disposed = match relay.finish_recycled(&mut original.receipt) {
            Ok(disposed) => disposed,
            Err(ReporterSourceIssue::Busy) => return Ok(false),
            Err(cause) => return Err(cause.into()),
        };
        // The source issues this passive disposition from the exact receipt validated above.
        let _disposed_key = disposed.key();
        if let Some(budget) = dismiss {
            self.dismissed = Some(AcceptedReporterDismiss { binding, budget });
        }
        self.retirement = None;
        self.preparation = ReporterPreparation {
            metadata: delivery::Delivery::new(),
            launch: delivery::Delivery::new(),
        };
        Ok(true)
    }
}
