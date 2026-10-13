// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check actual consumer wrappers against the producer's closed typed envelopes.
use super::super::{
    ExpectedSummary, IncidentPublicationDemand, StoredIncidentSummary,
    publication::{
        CommittedIncident, IncidentCommit, PendingPublication, PublishTicket, SeedCommit,
    },
    publication_preflight::{IncidentPublicationPreflight, RetainedPreflight},
    reporting::AcknowledgementCommit,
};
use super::SummaryCollection;
use std::mem::size_of;
use sugarsubstitute_contracts::incident::{
    AttachmentFacts, AttachmentKind, BuildIdentity, FileLabel, IncidentKey, MAX_ATTACHMENTS,
    MAX_SUMMARIES, SymbolName,
};
use sugarsubstitute_supervision::diagnostic_storage::*;
const _: () = assert!(size_of::<SeedCommit>() <= DIAGNOSTIC_SEED_RESULT_INLINE_BYTES);
const _: () = assert!(size_of::<IncidentCommit>() <= DIAGNOSTIC_SUMMARY_RESULT_INLINE_BYTES);
const _: () = assert!(size_of::<AcknowledgementCommit>() <= DIAGNOSTIC_SUMMARY_RESULT_INLINE_BYTES);
const _: () = assert!(size_of::<CommittedIncident>() <= DIAGNOSTIC_SUMMARY_RESULT_INLINE_BYTES);
const _: () = assert!(
    size_of::<(StoredIncidentSummary, DiagnosticReadFacts)>()
        <= DIAGNOSTIC_SUMMARY_COLLECTION_CELL_BYTES
);
const _: () = assert!(size_of::<SummaryCollection>() <= DIAGNOSTIC_SUMMARY_COLLECTION_INLINE_BYTES);
const _: () =
    assert!(size_of::<PendingPublication>() <= DIAGNOSTIC_PENDING_PUBLICATION_INLINE_BYTES);
const _: () = assert!(size_of::<RetainedPreflight>() <= DIAGNOSTIC_RETAINED_PREFLIGHT_INLINE_BYTES);
const _: () = assert!(size_of::<ExpectedSummary>() <= DIAGNOSTIC_EXPECTED_SUMMARY_INLINE_BYTES);
const _: () =
    assert!(size_of::<IncidentPublicationDemand>() <= DIAGNOSTIC_PUBLICATION_DEMAND_INLINE_BYTES);
const _: () = assert!(size_of::<PublishTicket>() <= DIAGNOSTIC_PUBLICATION_TOKEN_INLINE_BYTES);
const _: () =
    assert!(size_of::<IncidentPublicationPreflight>() <= DIAGNOSTIC_PUBLICATION_TOKEN_INLINE_BYTES);
const _: () = assert!(
    size_of::<(AttachmentKind, u64, u64, u64, DiagnosticActivation)>()
        <= DIAGNOSTIC_PUBLICATION_PAYLOAD_INLINE_BYTES
);
const SUMMARY_HEAP: usize = 2 * BuildIdentity::MAX_BYTES
    + 2 * FileLabel::MAX_BYTES
    + SymbolName::MAX_BYTES
    + size_of::<[Option<AttachmentFacts>; MAX_ATTACHMENTS]>();
const _: () = assert!(
    size_of::<SummaryCollection>()
        + MAX_SUMMARIES
            * (size_of::<IncidentKey>()
                + size_of::<(StoredIncidentSummary, DiagnosticReadFacts)>()
                + SUMMARY_HEAP)
        <= DIAGNOSTIC_COLLECTION_BYTES
);
