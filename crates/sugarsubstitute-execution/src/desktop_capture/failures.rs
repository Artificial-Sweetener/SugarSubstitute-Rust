// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep original typed native failures in already claimed child storage through settlement.
use crate::native::NativeObservationError;

/// Inline slots allocate no new Box/Arc and preserve the exact first owned error.
/// Later actual failures occupy eight fixed slots; overflow remains observable.
#[derive(Default)]
pub(super) struct RetainedFailures {
    first: Option<NativeObservationError>,
    later: [Option<NativeObservationError>; 8],
    count: usize,
    omitted: u64,
}
impl RetainedFailures {
    /// Move the actual returned owner before mailbox cloning/publication can consume it.
    pub(super) fn remember(&mut self, cause: NativeObservationError) {
        if self.first.is_none() {
            self.first = Some(cause);
        } else if let Some(slot) = self.later.get_mut(self.count) {
            *slot = Some(cause);
            self.count += 1;
        } else {
            self.omitted = self.omitted.saturating_add(1);
        }
    }
}
