// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coalesce native replacement intent only after an actual Main reveal.
use crate::MainReloadReason;
/// One current actor's fixed pending intent; no page or native capability is stored here.
#[derive(Default)]
pub(crate) struct ReloadAdmission {
    pending: bool,
}
impl ReloadAdmission {
    /// Actual current reveal is supplied by MainActor; duplicates have no further effect.
    pub(crate) fn request(
        &mut self,
        revealed: bool,
        reason: MainReloadReason,
    ) -> Option<MainReloadReason> {
        if !revealed || self.pending {
            return None;
        }
        self.pending = true;
        Some(reason)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn initial_process_failure_cannot_request_reload_or_arm_later_admission() {
        let mut admission = ReloadAdmission::default();
        assert_eq!(admission.request(false, MainReloadReason::ProcessFailed), None);
        assert_eq!(
            admission.request(true, MainReloadReason::Navigation),
            Some(MainReloadReason::Navigation)
        );
    }
    #[test]
    fn navigation_and_process_failure_coalesce_for_one_revealed_actor() {
        for first in [MainReloadReason::Navigation, MainReloadReason::ProcessFailed] {
            let mut admission = ReloadAdmission::default();
            assert_eq!(admission.request(true, first), Some(first));
            assert_eq!(admission.request(true, MainReloadReason::Navigation), None);
            assert_eq!(admission.request(true, MainReloadReason::ProcessFailed), None);
        }
    }
}
