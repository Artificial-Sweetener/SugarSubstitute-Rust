// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep parked documents' GPU backings within the device's video memory budget.
//!
//! Video memory is shared with other applications on the same GPU, ComfyUI's models above all, so
//! parked backings may use at most a tenth of the driver's budget and give way whenever the device
//! nears it. Displayed backings are never affected.

use super::*;

/// How often the session reads the device budget.
const INTERVAL: Duration = Duration::from_secs(2);

/// Bytes parked documents may hold in video memory: a tenth of the budget, less whatever keeps
/// the device's total use, theirs included, under nine tenths of it.
pub(super) fn parked_limit(
    budget: sugarsubstitute_image_gpu::VideoMemoryBudget,
    parked: u64,
) -> u64 {
    let ceiling = budget.budget_bytes / 10;
    let others = budget.usage_bytes.saturating_sub(parked);
    ceiling.min((budget.budget_bytes - ceiling).saturating_sub(others))
}

impl Session {
    /// Read the device budget when due and shed parked GPU backings beyond what it allows.
    pub(super) fn advance_video_memory(&mut self) {
        let now = self.navigation_time();
        if self.retained.video_memory_due.is_some_and(|due| now < due) {
            return;
        }
        self.retained.video_memory_due = Some(now + INTERVAL);
        let Some(budget) = self.resources.as_ref().and_then(work::Resources::video_memory_budget)
        else {
            return;
        };
        let parked = self.retained.gpu_bytes();
        let limit = parked_limit(budget, parked);
        if parked > limit {
            tracing::debug!(
                operation = "canvas_video_memory_shed",
                budget_bytes = budget.budget_bytes,
                usage_bytes = budget.usage_bytes,
                parked_bytes = parked,
                limit_bytes = limit,
                "Parked canvas documents gave video memory back"
            );
            self.retained.shed_gpu(limit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_image_gpu::VideoMemoryBudget;

    const GIB: u64 = 1024 * 1024 * 1024;

    #[test]
    fn parked_backings_keep_a_tenth_of_the_budget_and_yield_as_the_device_fills() {
        let budget = |usage| VideoMemoryBudget { budget_bytes: 30 * GIB, usage_bytes: usage };
        // An idle device: parked documents may hold a tenth of the budget.
        assert_eq!(parked_limit(budget(4 * GIB), GIB), 3 * GIB);
        // Others use 25 GiB of 30: only 2 GiB remain below nine tenths.
        assert_eq!(parked_limit(budget(26 * GIB), GIB), 2 * GIB);
        // A device at its budget keeps nothing parked.
        assert_eq!(parked_limit(budget(30 * GIB), 0), 0);
        assert_eq!(parked_limit(VideoMemoryBudget::default(), 0), 0);
    }

    #[test]
    fn documents_without_gpu_backings_are_untouched_by_a_video_memory_limit() {
        let mut retained = retention::RetainedDocuments::default();
        retained.shed_gpu(0);
        assert_eq!(retained.gpu_bytes(), 0);
        assert_eq!(retained.count(), 0);
    }
}
