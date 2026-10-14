// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Fixed disjoint shares of the same original paired symbol reservation.
use crate::native_resources::ResourceRecord;
use sugarsubstitute_contracts::execution::SymbolNativePartition;
#[derive(Clone, Copy)]
pub(crate) enum Purpose {
    Selected,
    CommittedReader,
    Provider,
}
impl Purpose {
    pub(super) const fn bytes(self) -> u64 {
        match self {
            Self::Selected => 2_097_152,
            Self::CommittedReader => 3_145_728,
            Self::Provider => 1_048_576,
        }
    }
    pub(super) const fn partition(self) -> SymbolNativePartition {
        match self {
            Self::Selected => SymbolNativePartition::SelectedRelease,
            Self::CommittedReader => SymbolNativePartition::CommittedReader,
            Self::Provider => SymbolNativePartition::Provider,
        }
    }
    pub(super) const fn issued(self, record: &ResourceRecord) -> bool {
        match self {
            Self::Selected => record.bundle_issued,
            Self::CommittedReader => record.reader_issued,
            Self::Provider => record.provider_issued,
        }
    }
    pub(super) fn mark_issued(self, record: &mut ResourceRecord) {
        match self {
            Self::Selected => record.bundle_issued = true,
            Self::CommittedReader => record.reader_issued = true,
            Self::Provider => record.provider_issued = true,
        }
    }
}
