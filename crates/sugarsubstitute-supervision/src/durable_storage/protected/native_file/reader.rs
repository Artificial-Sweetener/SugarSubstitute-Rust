// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow the sole checked read algorithm with its already retained original job.
use super::*;
#[cfg(test)]
mod tests;

/// Read the same observed read-only inode while its physical cell remains exclusively borrowed.
///
/// This view owns no additional file, handle credit, path or memory reservation. Its
/// context is copied from the physical cell's first actual opener rather than supplied
/// by a consumer. Matching metadata does not establish release or incident trust.
pub(crate) struct NativeReadCore<'file, 'phase, 'origin> {
    file: &'file mut NativeFile,
    phase: &'phase mut crate::durable_storage::protected::native_phase::NativePhase<'origin>,
    original: JobContext,
    length: u64,
}
impl NativeFile {
    /// Borrow an observed counted read-only cell using its original checked context.
    ///
    /// # Errors
    /// Rejects writable/delete grants, uncounted or unopened outputs, missing actual
    /// observations, and cancellation/deadline failures. Every error preserves this
    /// same physical owner; the view never reopens or retires it.
    pub(crate) fn reader<'file, 'phase, 'origin>(
        &'file mut self,
        phase: &'phase mut crate::durable_storage::protected::native_phase::NativePhase<'origin>,
    ) -> Result<super::NativeReadCore<'file, 'phase, 'origin>, ClaimError> {
        phase.check_budget(&self.fields()?.handle.budget())?;
        let (original, length) = match self.reader_binding() {
            Ok(binding) => binding,
            Err(cause) => {
                self.remember(cause.clone());
                return Err(cause);
            }
        };
        Ok(NativeReadCore { file: self, phase, original, length })
    }
    fn reader_binding(&self) -> Result<(JobContext, u64), ClaimError> {
        let budget = self.fields()?.handle.budget();
        if self.fields()?.grant != FileGrant::ReadOnly
            || !self.fields()?.counted
            || !self.fields()?.handle.is_armed()
            || self.fields()?.deleted
        {
            return Err(budget.rejection("native_read_grant"));
        }
        let original = self
            .fields()?
            .original
            .as_ref()
            .ok_or_else(|| budget.rejection("native_read_unopened"))?
            .clone();
        self.check(&original)?;
        let length =
            self.fields()?.facts.ok_or_else(|| budget.rejection("native_read_unobserved"))?.bytes;
        Ok((original, length))
    }
}
impl NativeReadCore<'_, '_, '_> {
    /// Return the actual retained observation's file length in bytes.
    pub(crate) fn length(&self) -> u64 {
        self.length
    }

    /// Read an exact range of at most 64 KiB through the sole physical file owner.
    ///
    /// # Errors
    /// Preserves the original checked native read cause, changed-inode rejection,
    /// range failure and original job cancellation/deadline. The borrowed physical
    /// cell retains its actual output, observation, memory and credit on every error.
    pub(crate) fn read_exact(&mut self, offset: u64, output: &mut [u8]) -> Result<(), ClaimError> {
        let count = self.file.read(offset, output, &self.original, self.phase)?;
        if count != output.len() {
            return Err(self.phase.invalid("native_read_short"));
        }
        Ok(())
    }
}
