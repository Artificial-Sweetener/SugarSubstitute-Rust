// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The parent lifecycles of desktop children that have not retired, which the coordinator's
//! update status reaches.
use super::*;
use crate::production::parent_lifecycle::ParentLifecycle;

impl ProductionActivation {
    /// Every live child's parent lifecycle.
    pub(super) fn live_parents(&self) -> impl Iterator<Item = &ParentLifecycle> {
        self.records
            .iter()
            .flatten()
            .filter(|retained| retained.record.retired.is_none())
            .filter_map(|retained| retained.record.parent.as_deref())
    }

    /// Every live child's parent lifecycle, to change.
    pub(super) fn live_parents_mut(&mut self) -> impl Iterator<Item = &mut ParentLifecycle> {
        self.records
            .iter_mut()
            .flatten()
            .filter(|retained| retained.record.retired.is_none())
            .filter_map(|retained| retained.record.parent.as_deref_mut())
    }

    /// A live window that asked to restart while it closes, so the restart notice can cover
    /// the screen before it hides, or a window that then completed that close normally.
    pub(super) fn restart_requested(&self) -> bool {
        self.live_parents().any(ParentLifecycle::restart_to_apply_requested)
            || self.records.iter().flatten().any(|retained| {
                retained
                    .record
                    .completed_normal
                    .as_ref()
                    .is_some_and(super::normal_retirement::CompletedNormal::restart_to_apply)
            })
    }
}
