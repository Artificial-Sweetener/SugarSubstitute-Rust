// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe the compiled consumer layout, independent of allocator/native peak qualification.
use super::*;
use std::mem::size_of;
use sugarsubstitute_supervision::diagnostic_storage::*;
#[test]
fn compiled_record_and_collection_envelopes_cover_the_actual_consumer_types() {
    eprintln!(
        "SeedCommit={} IncidentCommit={} AcknowledgementCommit={} CommittedIncident={} StoredIncidentSummary={} SummaryCell={} SummaryCollection={} IncidentRepository={}",
        size_of::<SeedCommit>(),
        size_of::<IncidentCommit>(),
        size_of::<AcknowledgementCommit>(),
        size_of::<CommittedIncident>(),
        size_of::<StoredIncidentSummary>(),
        size_of::<(StoredIncidentSummary, DiagnosticReadFacts)>(),
        size_of::<super::super::allocation::SummaryCollection>(),
        size_of::<IncidentRepository>()
    );
    assert!(size_of::<IncidentCommit>() <= DIAGNOSTIC_SUMMARY_RESULT_INLINE_BYTES);
    assert!(size_of::<AcknowledgementCommit>() <= DIAGNOSTIC_SUMMARY_RESULT_INLINE_BYTES);
    assert!(size_of::<CommittedIncident>() <= DIAGNOSTIC_SUMMARY_RESULT_INLINE_BYTES);
    assert!(size_of::<SeedCommit>() <= DIAGNOSTIC_SEED_RESULT_INLINE_BYTES);
    assert!(
        size_of::<(StoredIncidentSummary, DiagnosticReadFacts)>()
            <= DIAGNOSTIC_SUMMARY_COLLECTION_CELL_BYTES
    );
    assert!(
        size_of::<super::super::allocation::SummaryCollection>()
            <= DIAGNOSTIC_SUMMARY_COLLECTION_INLINE_BYTES
    );
}
#[test]
fn compiled_preflight_pending_and_tuple_envelopes_match_complete_four_field_demand() {
    eprintln!(
        "ExpectedSummary={} PublicationDemand={} PublishTicket={} IncidentPublicationPreflight={} PendingPublication={} RetainedPreflight={} PayloadTuple={} LoanedCodecFailure={}",
        size_of::<ExpectedSummary>(),
        size_of::<IncidentPublicationDemand>(),
        size_of::<PublishTicket>(),
        size_of::<IncidentPublicationPreflight>(),
        size_of::<super::super::publication::PendingPublication>(),
        size_of::<super::super::publication_preflight::RetainedPreflight>(),
        size_of::<(AttachmentKind, u64, u64, u64, DiagnosticActivation)>(),
        size_of::<LoanedCodecFailure>()
    );
    assert!(size_of::<IncidentPublicationDemand>() <= DIAGNOSTIC_PUBLICATION_DEMAND_INLINE_BYTES);
    assert!(size_of::<ExpectedSummary>() <= DIAGNOSTIC_EXPECTED_SUMMARY_INLINE_BYTES);
    assert!(size_of::<PublishTicket>() <= DIAGNOSTIC_PUBLICATION_TOKEN_INLINE_BYTES);
    assert!(size_of::<IncidentPublicationPreflight>() <= DIAGNOSTIC_PUBLICATION_TOKEN_INLINE_BYTES);
    assert!(
        size_of::<super::super::publication::PendingPublication>()
            <= DIAGNOSTIC_PENDING_PUBLICATION_INLINE_BYTES
    );
    assert!(
        size_of::<super::super::publication_preflight::RetainedPreflight>()
            <= DIAGNOSTIC_RETAINED_PREFLIGHT_INLINE_BYTES
    );
    assert!(
        size_of::<(AttachmentKind, u64, u64, u64, DiagnosticActivation)>()
            <= DIAGNOSTIC_PUBLICATION_PAYLOAD_INLINE_BYTES
    );
    assert_eq!(
        DIAGNOSTIC_COLLECTION_BYTES + DIAGNOSTIC_DECODE_BYTES + DIAGNOSTIC_RESIDUAL_BYTES,
        DIAGNOSTIC_STORAGE_BYTES as usize
    );
}
