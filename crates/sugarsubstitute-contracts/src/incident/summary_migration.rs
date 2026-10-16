// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Closed contiguous summary format steps independent of storage and platform adapters.
/// One supported original/candidate schema pair, preserved through migration intent and archive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SummaryMigrationStep {
    /// Introduce an independent immutable payload publication generation.
    OneToTwo,
    /// Introduce explicit capture-only evidence without manufacturing an exit status.
    TwoToThree,
}
impl SummaryMigrationStep {
    /// Return the exact original schema version.
    pub const fn source_schema(self) -> u32 {
        match self {
            Self::OneToTwo => 1,
            Self::TwoToThree => 2,
        }
    }
    /// Return the exact contiguous candidate schema version.
    pub const fn candidate_schema(self) -> u32 {
        match self {
            Self::OneToTwo => 2,
            Self::TwoToThree => 3,
        }
    }
    /// Recognize only implemented contiguous pairs; unknown/reversed/skipped/unchanged pairs return None.
    pub const fn from_schemas(source: u32, candidate: u32) -> Option<Self> {
        match (source, candidate) {
            (1, 2) => Some(Self::OneToTwo),
            (2, 3) => Some(Self::TwoToThree),
            _ => None,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_supported_contiguous_summary_steps_can_be_recorded() {
        for source in 0..=5 {
            for candidate in 0..=5 {
                let step = SummaryMigrationStep::from_schemas(source, candidate);
                assert_eq!(step.is_some(), matches!((source, candidate), (1, 2) | (2, 3)));
                if let Some(step) = step {
                    assert_eq!(step.source_schema(), source);
                    assert_eq!(step.candidate_schema(), candidate);
                }
            }
        }
        assert!(SummaryMigrationStep::from_schemas(u32::MAX, 0).is_none());
        assert!(SummaryMigrationStep::from_schemas(2, u32::MAX).is_none());
    }
}
