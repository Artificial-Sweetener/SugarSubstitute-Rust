// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve bounded incident records independently of native capture and presentation.
mod codec;
mod codec_capture;
mod codec_evidence;
mod codec_identity;
mod codec_ledger;
mod codec_migration;
mod codec_seed;
mod codec_summary;
mod codec_symbols;
mod error;
mod evidence;
#[cfg(windows)]
mod migration;
#[cfg(windows)]
mod publication;
#[cfg(windows)]
pub use migration::IncidentSummaryMigrationValidator;
#[cfg(windows)]
mod recovery;
#[cfg(windows)]
mod reporting;
#[cfg(windows)]
mod repository;
#[cfg(windows)]
pub use publication::{
    CommittedIncident, ExpectedSummary, IncidentCommit, PublishTicket, SeedCommit,
};
#[cfg(windows)]
pub use recovery::IncidentRecoveryValidator;
#[cfg(windows)]
pub use reporting::AcknowledgementCommit;
#[cfg(windows)]
pub use repository::IncidentRepository;

pub use codec::{
    decode_release_symbols, decode_seed, decode_symbol_report, encode_release_symbols, encode_seed,
    encode_symbol_report,
};
pub use codec_capture::{
    CaptureDispositionCompletion, CaptureDispositionPlan, CaptureMetadataStageRole,
    CaptureRecordedFileFacts,
};
pub use codec_ledger::{
    IncidentFileFacts, IncidentStoreState, SummaryEvictionJournal, decode_store_state,
    encode_store_state,
};
pub use codec_migration::{SummaryMigrationArchive, SummaryMigrationJournal};
pub use codec_summary::{
    StoredIncidentSummary, decode_summary, encode_summary, migrate_summary_v1,
};
pub use error::{IncidentStoreError, IncidentStoreFailureCode};
pub use evidence::{IncidentLogCode, RedactedDiagnosticFact, RedactedTextChunk};

#[cfg(test)]
mod tests;
