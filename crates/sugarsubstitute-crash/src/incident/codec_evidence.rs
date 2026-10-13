// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode explicit evidence enums and bounded unique attachment facts without native authority.
use super::IncidentStoreError;
use serde::{Deserialize, Serialize};
use sugarsubstitute_contracts::{
    incident::{
        AttachmentFacts, AttachmentInventory, AttachmentKind, CaptureAbsence, CaptureEvidence,
        DiscardedPayload, DumpId, FailureEvidence, PayloadDiscard, PayloadDisposition,
    },
    startup::StartupFailureCode,
};

macro_rules! enum_record {
    ($record:ident, $domain:ident, [$($variant:ident),+]) => {
        #[derive(Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub(super) enum $record { $($variant),+ }
        impl From<$domain> for $record {
            fn from(value: $domain) -> Self { match value { $($domain::$variant => Self::$variant),+ } }
        }
        impl $record {
            pub(super) fn into_facts(self) -> $domain { match self { $(Self::$variant => $domain::$variant),+ } }
        }
    };
}
enum_record!(
    StartupRecord,
    StartupFailureCode,
    [ConnectionPreparation, ConnectionUnavailable, ProcessExited, TimedOut, Presentation]
);
enum_record!(
    AbsenceRecord,
    CaptureAbsence,
    [
        HandlerUnavailable,
        HandlerLost,
        Missing,
        Busy,
        DeadlineElapsed,
        WrongIdentity,
        Corrupt,
        Oversized
    ]
);
enum_record!(KindRecord, AttachmentKind, [Startup, Fault, Runtime, Log, SymbolizedFault, Minidump]);
enum_record!(
    DiscardRecord,
    PayloadDiscard,
    [AcknowledgedRetention, StoragePressure, Oversized, UserRequested]
);

mod failure;
pub(super) use failure::FailureRecord;
mod capture;
pub(super) use capture::CaptureRecord;
mod payload;
pub(super) use payload::PayloadRecord;
