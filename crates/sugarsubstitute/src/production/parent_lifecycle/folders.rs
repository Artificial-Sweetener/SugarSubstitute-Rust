// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compare exact requested UTF16 and retain effective units until actual OS validation.
use super::ParentError;
use sugarsubstitute_contracts::startup::MainIdentity;
use sugarsubstitute_supervision::lifecycle_protocol::{FolderFragment, MAX_PROFILE_FOLDER_UNITS};

pub(super) struct Folders {
    identity: Option<MainIdentity>,
    requested: usize,
    effective: usize,
    total: usize,
    units: [u16; MAX_PROFILE_FOLDER_UNITS],
}

impl Folders {
    pub(super) const fn new() -> Self {
        Self {
            identity: None,
            requested: 0,
            effective: 0,
            total: 0,
            units: [0; MAX_PROFILE_FOLDER_UNITS],
        }
    }

    pub(super) fn begin(&mut self, identity: MainIdentity) {
        self.identity = Some(identity);
        self.requested = 0;
        self.effective = 0;
        self.total = 0;
    }

    pub(super) fn requested(
        &mut self,
        fragment: FolderFragment<'_>,
        expected: &[u16],
    ) -> Result<u32, ParentError> {
        let count = fragment.units().unit_len();
        let offset = fragment.offset_units() as usize;
        if self.identity != Some(fragment.identity())
            || fragment.total_units() as usize != expected.len()
            || offset != self.requested
            || expected
                .get(offset..offset + count)
                .is_none_or(|slice| !slice.iter().copied().eq(fragment.units().units()))
        {
            return Err(ParentError::ProfileMismatch);
        }
        self.requested += count;
        u32::try_from(self.requested).map_err(|_| ParentError::ProfileMismatch)
    }

    pub(super) fn effective(
        &mut self,
        fragment: FolderFragment<'_>,
        requested_len: usize,
    ) -> Result<bool, ParentError> {
        let count = fragment.units().unit_len();
        let total = fragment.total_units() as usize;
        let offset = fragment.offset_units() as usize;
        if self.identity != Some(fragment.identity())
            || self.requested != requested_len
            || offset != self.effective
            || (offset != 0 && total != self.total)
            || offset + count > self.units.len()
        {
            return Err(ParentError::ProfileMismatch);
        }
        for (destination, unit) in
            self.units[offset..offset + count].iter_mut().zip(fragment.units().units())
        {
            *destination = unit;
        }
        self.total = total;
        self.effective += count;
        Ok(self.effective == total)
    }

    pub(super) fn units(&self) -> &[u16] {
        &self.units[..self.effective]
    }
}
