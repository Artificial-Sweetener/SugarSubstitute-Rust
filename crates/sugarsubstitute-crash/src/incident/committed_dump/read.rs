// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read complete bounded metadata and exact parser chunks from protected native slots.
use super::*;
use sugarsubstitute_supervision::committed_reader::CommittedReadObject;
impl IncidentDumpReader {
    pub(super) fn source(
        &mut self,
        object: CommittedReadObject,
    ) -> Result<CommittedReadSlot, DumpReadCause> {
        let native = self.native.as_mut().ok_or(DumpReadCause::Identity)?;
        native.check_effect().map_err(DumpReadCause::Control)?;
        let slot = native.open(object).map_err(DumpReadCause::Native)?;
        let facts = native.facts(&slot).map_err(DumpReadCause::Native)?;
        let count = usize::try_from(facts.bytes()).map_err(|_| DumpReadCause::Capacity)?;
        if count == 0 || count > 65_536 {
            return Err(DumpReadCause::Capacity);
        }
        self.fields.source.clear();
        if self.fields.source.capacity() < count {
            self.fields.source.try_reserve_exact(count).map_err(DumpReadCause::Reserve)?;
        }
        if self.fields.source.capacity() > 65_536 {
            return Err(DumpReadCause::Capacity);
        }
        self.fields.source.resize(count, 0);
        let actual =
            native.read_chunk(&slot, 0, &mut self.fields.source).map_err(DumpReadCause::Native)?;
        if actual != count {
            return Err(DumpReadCause::Identity);
        }
        Ok(slot)
    }
    pub(crate) fn read_dump(
        &mut self,
        offset: u64,
        output: &mut [u8],
    ) -> Result<usize, DumpReadCause> {
        if self.fields.first.is_some() {
            return Err(DumpReadCause::Identity);
        }
        let slot = self.fields.dump_slot.as_ref().ok_or(DumpReadCause::Identity)?;
        self.native
            .as_mut()
            .ok_or(DumpReadCause::Identity)?
            .read_chunk(slot, offset, output)
            .map_err(DumpReadCause::Native)
    }
    pub(crate) fn control(&self) -> Result<(), DumpReadCause> {
        self.native
            .as_ref()
            .ok_or(DumpReadCause::Identity)?
            .check_effect()
            .map_err(DumpReadCause::Control)
    }
    /// Observe actual Summary before cleanup; no cleanup-phase read is performed.
    /// # Errors
    /// Changed identity/size/hash, original cancellation or original cutoff retain custody.
    pub fn check_current(&mut self) -> Result<(), DumpReadCode> {
        let result = self.check_current_inner();
        result.map_err(|cause| self.record(cause))
    }
    pub(super) fn check_current_inner(&mut self) -> Result<(), DumpReadCause> {
        let slot = self.fields.summary_slot.as_ref().ok_or(DumpReadCause::Identity)?;
        self.native
            .as_mut()
            .ok_or(DumpReadCause::Identity)?
            .check_current(slot)
            .map_err(DumpReadCause::Native)?;
        self.fields.checked_current = true;
        Ok(())
    }
}
