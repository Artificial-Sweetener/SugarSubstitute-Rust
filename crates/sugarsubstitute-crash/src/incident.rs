// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve bounded incident records independently of native capture and presentation.
#[cfg(windows)]
mod allocation;
mod codec;
mod codec_capture;
mod codec_encode;
mod codec_evidence;
mod codec_identity;
mod codec_ledger;
mod codec_migration;
mod codec_parse;
mod codec_retention;
mod codec_seed;
mod codec_summary;
mod codec_symbols;
mod codec_text;
#[cfg(windows)]
pub use allocation::{LoanedAllocationFailure, LoanedCodecFailure};
mod error;
mod evidence;
#[cfg(windows)]
mod migration;
#[cfg(windows)]
mod publication;
#[cfg(windows)]
mod publication_preflight;
#[cfg(windows)]
pub use migration::IncidentSummaryMigrationValidator;
#[cfg(windows)]
mod recovery;
#[cfg(windows)]
mod reporting;
#[cfg(windows)]
mod repository;
#[cfg(windows)]
mod retention;
#[cfg(windows)]
mod retention_disposition;
#[cfg(windows)]
mod retention_policy;
#[cfg(windows)]
pub use publication::{
    CommittedIncident, ExpectedSummary, IncidentCommit, PublishTicket, SeedCommit,
};
#[cfg(windows)]
pub use publication_preflight::{IncidentPublicationDemand, IncidentPublicationPreflight};
#[cfg(windows)]
pub use recovery::IncidentRecoveryValidator;
#[cfg(windows)]
pub use reporting::AcknowledgementCommit;
#[cfg(windows)]
pub use repository::IncidentRepository;
#[cfg(windows)]
pub use retention::IncidentPayloadDiscardValidator;
#[cfg(windows)]
pub use retention_policy::RetentionSnapshot;

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
pub use codec_retention::{PayloadDiscardJournal, PayloadDiscardVictimFacts};
pub use codec_summary::{
    StoredIncidentSummary, decode_summary, encode_summary, migrate_summary_v1,
};
pub use error::{IncidentStoreError, IncidentStoreFailureCode};
pub use evidence::{IncidentLogCode, RedactedDiagnosticFact, RedactedTextChunk};

#[cfg(test)]
mod tests;
