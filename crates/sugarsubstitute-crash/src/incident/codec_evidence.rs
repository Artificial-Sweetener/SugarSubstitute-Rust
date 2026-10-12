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

#[derive(Serialize, Deserialize)]
#[serde(tag = "mechanism", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum FailureRecord {
    Unexplained {},
    Startup { code: StartupRecord },
    RustPanic {},
    Abort {},
    NativeFault { code: u32, address: u64 },
}
impl FailureRecord {
    pub(super) fn into_facts(self) -> FailureEvidence {
        match self {
            Self::Unexplained {} => FailureEvidence::Unexplained,
            Self::RustPanic {} => FailureEvidence::RustPanic,
            Self::Abort {} => FailureEvidence::Abort,
            Self::Startup { code } => FailureEvidence::Startup(code.into_facts()),
            Self::NativeFault { code, address } => FailureEvidence::NativeFault { code, address },
        }
    }
}
impl From<FailureEvidence> for FailureRecord {
    fn from(value: FailureEvidence) -> Self {
        match value {
            FailureEvidence::Unexplained => Self::Unexplained {},
            FailureEvidence::RustPanic => Self::RustPanic {},
            FailureEvidence::Abort => Self::Abort {},
            FailureEvidence::Startup(code) => Self::Startup { code: code.into() },
            FailureEvidence::NativeFault { code, address } => Self::NativeFault { code, address },
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "availability", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum CaptureRecord {
    Awaiting {},
    Unavailable { reason: AbsenceRecord },
    Finished { dump: [u8; 16] },
}
impl CaptureRecord {
    pub(super) fn into_facts(self) -> Result<CaptureEvidence, IncidentStoreError> {
        Ok(match self {
            Self::Awaiting {} => CaptureEvidence::Awaiting,
            Self::Unavailable { reason } => CaptureEvidence::Unavailable(reason.into_facts()),
            Self::Finished { dump } => CaptureEvidence::Finished(DumpId::new(dump)?),
        })
    }
}
impl From<CaptureEvidence> for CaptureRecord {
    fn from(value: CaptureEvidence) -> Self {
        match value {
            CaptureEvidence::Awaiting => Self::Awaiting {},
            CaptureEvidence::Unavailable(reason) => Self::Unavailable { reason: reason.into() },
            CaptureEvidence::Finished(dump) => Self::Finished { dump: *dump.bytes() },
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AttachmentRecord {
    kind: KindRecord,
    retained_bytes: u64,
    original_bytes: u64,
    omitted_bytes: u64,
    sha256: [u8; 32],
}
impl AttachmentRecord {
    fn into_facts(self) -> Result<AttachmentFacts, IncidentStoreError> {
        AttachmentFacts::new(
            self.kind.into_facts(),
            self.retained_bytes,
            self.original_bytes,
            self.omitted_bytes,
            self.sha256,
        )
        .map_err(Into::into)
    }
}
impl From<&AttachmentFacts> for AttachmentRecord {
    fn from(value: &AttachmentFacts) -> Self {
        Self {
            kind: value.kind().into(),
            retained_bytes: value.retained_bytes(),
            original_bytes: value.original_bytes(),
            omitted_bytes: value.omitted_bytes(),
            sha256: *value.sha256(),
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "disposition", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum PayloadRecord {
    Unavailable {},
    Retained {
        #[serde(deserialize_with = "bounded_attachments")]
        attachments: Vec<AttachmentRecord>,
    },
    Discarded {
        reason: DiscardRecord,
        source_bytes: u64,
    },
    Quarantined {},
}
fn bounded_attachments<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<AttachmentRecord>, D::Error> {
    struct Visitor;
    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = Vec<AttachmentRecord>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("at most eight allowlisted attachments")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut sequence: A,
        ) -> Result<Self::Value, A::Error> {
            let mut entries =
                Vec::with_capacity(sugarsubstitute_contracts::incident::MAX_ATTACHMENTS);
            while let Some(entry) = sequence.next_element()? {
                if entries.len() == sugarsubstitute_contracts::incident::MAX_ATTACHMENTS {
                    return Err(serde::de::Error::custom("attachment inventory exceeds its bound"));
                }
                entries.push(entry);
            }
            Ok(entries)
        }
    }
    deserializer.deserialize_seq(Visitor)
}
impl PayloadRecord {
    pub(super) fn into_facts(self) -> Result<PayloadDisposition, IncidentStoreError> {
        Ok(match self {
            Self::Unavailable {} => PayloadDisposition::Unavailable,
            Self::Quarantined {} => PayloadDisposition::Quarantined,
            Self::Discarded { reason, source_bytes } => PayloadDisposition::Discarded(
                DiscardedPayload::new(reason.into_facts(), source_bytes)?,
            ),
            Self::Retained { attachments } => {
                let entries = attachments
                    .into_iter()
                    .map(AttachmentRecord::into_facts)
                    .collect::<Result<Vec<_>, _>>()?;
                PayloadDisposition::Retained(AttachmentInventory::new(&entries)?)
            }
        })
    }
}
impl From<&PayloadDisposition> for PayloadRecord {
    fn from(value: &PayloadDisposition) -> Self {
        match value {
            PayloadDisposition::Unavailable => Self::Unavailable {},
            PayloadDisposition::Quarantined => Self::Quarantined {},
            PayloadDisposition::Discarded(value) => Self::Discarded {
                reason: value.reason().into(),
                source_bytes: value.source_bytes(),
            },
            PayloadDisposition::Retained(value) => {
                Self::Retained { attachments: value.entries().map(Into::into).collect() }
            }
        }
    }
}
