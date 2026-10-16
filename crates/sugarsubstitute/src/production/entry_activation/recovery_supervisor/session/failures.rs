// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the first two actual causes and a saturating omission count, without polling logs.
use super::*;
#[derive(Default)]
pub(super) struct Failures {
    pub(super) first: Option<SessionCause>,
    pub(super) secondary: Option<SessionCause>,
    pub(super) omitted: u64,
}
impl Failures {
    pub(super) fn record(&mut self, cause: SessionCause) {
        if self.first.is_none() {
            self.first = Some(cause);
        } else if self.secondary.is_none() {
            self.secondary = Some(cause);
        } else {
            self.omitted = self.omitted.saturating_add(1);
        }
    }
}
