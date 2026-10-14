// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own each original summary independently of collection and native retirement.
use super::super::StoredIncidentSummary;
use std::ops::Deref;
use sugarsubstitute_supervision::diagnostic_storage::DiagnosticRecoveryRecordLoan;

pub(in crate::incident) struct RetainedSummary {
    pub(in crate::incident) stored: StoredIncidentSummary,
    pub(in crate::incident) _custody: DiagnosticRecoveryRecordLoan,
}
impl RetainedSummary {
    pub(in crate::incident) fn new(
        stored: StoredIncidentSummary,
        custody: DiagnosticRecoveryRecordLoan,
    ) -> Self {
        Self { stored, _custody: custody }
    }
}
impl Deref for RetainedSummary {
    type Target = StoredIncidentSummary;
    fn deref(&self) -> &Self::Target {
        &self.stored
    }
}

/// Dense inline tickets; native authority remains with the protected owner.
pub(in crate::incident) struct OwnerSlots<T, const N: usize> {
    cells: [Option<T>; N],
    count: usize,
}
impl<T, const N: usize> OwnerSlots<T, N> {
    pub(in crate::incident) fn new() -> Self {
        Self { cells: std::array::from_fn(|_| None), count: 0 }
    }
    pub(in crate::incident) const fn len(&self) -> usize {
        self.count
    }
    pub(in crate::incident) const fn is_empty(&self) -> bool {
        self.count == 0
    }
    pub(in crate::incident) fn push(
        &mut self,
        value: T,
    ) -> Result<(), super::super::IncidentStoreError> {
        let cell =
            self.cells.get_mut(self.count).ok_or(super::super::IncidentStoreError::Capacity)?;
        *cell = Some(value);
        self.count += 1;
        Ok(())
    }
    pub(in crate::incident) fn get_mut(
        &mut self,
        index: usize,
    ) -> Result<&mut T, super::super::IncidentStoreError> {
        self.cells
            .get_mut(index)
            .and_then(Option::as_mut)
            .ok_or(super::super::IncidentStoreError::RecoveryConflict)
    }
    pub(in crate::incident) fn three_mut(
        &mut self,
        first: usize,
        second: usize,
        third: usize,
    ) -> Result<(&mut T, &mut T, &mut T), super::super::IncidentStoreError> {
        if first >= second || second >= third || third >= self.count {
            return Err(super::super::IncidentStoreError::RecoveryConflict);
        }
        let (prefix, third_cell) = self.cells.split_at_mut(third);
        let (first_cells, second_cell) = prefix.split_at_mut(second);
        Ok((
            first_cells[first]
                .as_mut()
                .ok_or(super::super::IncidentStoreError::RecoveryConflict)?,
            second_cell[0].as_mut().ok_or(super::super::IncidentStoreError::RecoveryConflict)?,
            third_cell[0].as_mut().ok_or(super::super::IncidentStoreError::RecoveryConflict)?,
        ))
    }
    pub(in crate::incident) fn remove(
        &mut self,
        index: usize,
    ) -> Result<T, super::super::IncidentStoreError> {
        let cell =
            self.cells.get_mut(index).ok_or(super::super::IncidentStoreError::RecoveryConflict)?;
        let value = cell.take().ok_or(super::super::IncidentStoreError::RecoveryConflict)?;
        self.cells[index..self.count].rotate_left(1);
        self.count -= 1;
        Ok(value)
    }
    pub(in crate::incident) fn clear(&mut self) {
        for cell in &mut self.cells {
            *cell = None;
        }
        self.count = 0;
    }
}
