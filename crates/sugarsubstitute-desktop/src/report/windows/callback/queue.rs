// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound registered native input and coalesce its wake without allocating or rebinding.
use super::{Input, ReportSurfaceError};
use std::cell::{Cell, RefCell};
pub(super) struct Queue {
    slots: RefCell<[Option<Input>; 32]>,
    wake: Cell<bool>,
}
impl Queue {
    pub(super) const fn new() -> Self {
        Self { slots: RefCell::new([None; 32]), wake: Cell::new(false) }
    }
    pub(super) fn push(&self, input: Input) -> Result<bool, ReportSurfaceError> {
        let mut slots = self
            .slots
            .try_borrow_mut()
            .map_err(|_| ReportSurfaceError::rejected("report_input_queue_recursive"))?;
        let slot = slots
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or_else(|| ReportSurfaceError::rejected("report_input_queue_full"))?;
        *slot = Some(input);
        Ok(!self.wake.replace(true))
    }
    pub(super) fn drain(&self) -> Result<[Option<Input>; 32], ReportSurfaceError> {
        let mut slots = self
            .slots
            .try_borrow_mut()
            .map_err(|_| ReportSurfaceError::rejected("report_input_drain_recursive"))?;
        let drained = std::mem::take(&mut *slots);
        self.wake.set(false);
        Ok(drained)
    }
}
#[cfg(test)]
mod tests;
