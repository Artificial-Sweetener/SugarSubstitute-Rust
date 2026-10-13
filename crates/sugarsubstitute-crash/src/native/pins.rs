// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt the actual handler output handles and preserve partial closure causes.
use crate::{
    CrashError,
    input::{MAX_ANCESTORS, MAX_UNITS},
};
use std::{os::windows::ffi::OsStrExt, path::Path};
use sugarsubstitute_contracts::{crash::CaptureOperation, execution::JobContext};
use windows::Win32::Foundation::{CloseHandle, HANDLE};

pub(super) const MAX_HANDLES: usize = MAX_ANCESTORS * 8 + 4;
#[cfg(test)]
mod tests;
pub(super) struct Pins {
    pub(super) values: Vec<Option<usize>>,
}
pub(super) struct CloseReport {
    pub(super) first: Option<CrashError>,
    pub(super) additional: [Option<CrashError>; 8],
    pub(super) omitted: u64,
}
impl CloseReport {
    fn new() -> Self {
        Self { first: None, additional: std::array::from_fn(|_| None), omitted: 0 }
    }
    fn retain(&mut self, cause: CrashError) {
        if self.first.is_none() {
            self.first = Some(cause);
        } else if let Some(slot) = self.additional.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(cause);
        } else {
            self.omitted = self.omitted.saturating_add(1);
        }
    }
}
pub(super) fn wide(path: &Path) -> Result<Vec<u16>, CrashError> {
    crate::input::validate_path(path)?;
    let mut units = Vec::new();
    units
        .try_reserve_exact(path.as_os_str().encode_wide().count() + 1)
        .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
    units.extend(path.as_os_str().encode_wide());
    units.push(0);
    if units.capacity() > MAX_UNITS + 1 {
        return Err(CrashError::invalid(CaptureOperation::Runtime));
    }
    Ok(units)
}
pub(super) fn deadline(context: &JobContext) -> Result<(), CrashError> {
    if context.cancellation_requested()
        || context.deadline().is_none_or(|limit| context.now() >= limit)
    {
        Err(CrashError::invalid(CaptureOperation::Runtime))
    } else {
        Ok(())
    }
}
impl Pins {
    pub(super) fn new() -> Result<Self, CrashError> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(MAX_HANDLES)
            .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
        if values.capacity() > MAX_HANDLES {
            return Err(CrashError::invalid(CaptureOperation::Runtime));
        }
        Ok(Self { values })
    }
    pub(super) fn reserve_outputs(&mut self) -> Result<[usize; 2], CrashError> {
        if self.values.len() > MAX_HANDLES - 2 {
            return Err(CrashError::invalid(CaptureOperation::Runtime));
        }
        let first = self.values.len();
        self.values.extend([None, None]);
        Ok([first, first + 1])
    }
    pub(super) fn retain_outputs(
        &mut self,
        slots: [usize; 2],
        outputs: [usize; 2],
    ) -> Result<(), CrashError> {
        let mut invalid = false;
        // Every actual unique output is durably installed before any validation can return.
        for (slot, raw) in slots.into_iter().zip(outputs) {
            if raw == 0 {
                continue;
            }
            if raw == usize::MAX || self.values.iter().flatten().any(|old| *old == raw) {
                invalid = true;
            } else {
                self.values[slot] = Some(raw);
            }
        }
        if invalid { Err(CrashError::invalid(CaptureOperation::Runtime)) } else { Ok(()) }
    }
    pub(super) fn release_unused_outputs(&mut self) {
        while self.values.last() == Some(&None) {
            self.values.pop();
        }
    }
    pub(super) fn is_empty(&self) -> bool {
        self.values.iter().all(Option::is_none)
    }
    pub(super) fn handle(&self, index: usize) -> Result<HANDLE, CrashError> {
        self.values
            .get(index)
            .and_then(|slot| *slot)
            .map(|raw| HANDLE(raw as *mut _))
            .ok_or_else(|| CrashError::invalid(CaptureOperation::Runtime))
    }
    pub(super) fn close(&mut self) -> CloseReport {
        let mut report = CloseReport::new();
        for slot in self.values.iter_mut().rev() {
            if let Some(raw) = *slot {
                // SAFETY: Every slot is an adopted unique owned native handle;
                // only actual successful closure removes that retained ownership.
                match unsafe { CloseHandle(HANDLE(raw as *mut _)) } {
                    Ok(()) => *slot = None,
                    Err(cause) => {
                        report.retain(CrashError::new(CaptureOperation::Retirement, cause))
                    }
                }
            }
        }
        report
    }
}
