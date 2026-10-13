// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Fixed disjoint shares of the same original paired symbol reservation.
use crate::native_resources::ResourceRecord;
use sugarsubstitute_contracts::execution::SymbolNativePartition;
#[derive(Clone, Copy)]
pub(super) enum Purpose {
    Selected,
    CommittedReader,
}
impl Purpose {
    pub(super) const fn bytes(self) -> u64 {
        match self {
            Self::Selected => 2_097_152,
            Self::CommittedReader => 3_145_728,
        }
    }
    pub(super) const fn partition(self) -> SymbolNativePartition {
        match self {
            Self::Selected => SymbolNativePartition::SelectedRelease,
            Self::CommittedReader => SymbolNativePartition::CommittedReader,
        }
    }
    pub(super) const fn issued(self, record: &ResourceRecord) -> bool {
        match self {
            Self::Selected => record.bundle_issued,
            Self::CommittedReader => record.reader_issued,
        }
    }
    pub(super) fn mark_issued(self, record: &mut ResourceRecord) {
        match self {
            Self::Selected => record.bundle_issued = true,
            Self::CommittedReader => record.reader_issued = true,
        }
    }
}
