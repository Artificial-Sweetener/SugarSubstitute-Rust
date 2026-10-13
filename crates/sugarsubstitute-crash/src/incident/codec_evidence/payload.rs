// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the closed PayloadRecord record without untyped content buffering.
use super::*;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct AttachmentRecord {
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
#[derive(Serialize)]
#[serde(tag = "disposition", rename_all = "snake_case", deny_unknown_fields)]
pub(in super::super) enum PayloadRecord {
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
    pub(in super::super) fn into_facts(self) -> Result<PayloadDisposition, IncidentStoreError> {
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

struct AttachmentList(Vec<AttachmentRecord>);
impl<'de> Deserialize<'de> for AttachmentList {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        bounded_attachments(deserializer).map(Self)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Tag {
    Unavailable,
    Retained,
    Discarded,
    Quarantined,
}
#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum Field {
    Disposition,
    Attachments,
    Reason,
    SourceBytes,
}
impl<'de> Deserialize<'de> for PayloadRecord {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = PayloadRecord;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a closed PayloadRecord map")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                use serde::de::Error as _;
                let mut disposition = None;
                let mut attachments = None;
                let mut reason = None;
                let mut source_bytes = None;
                while let Some(field) = map.next_key::<Field>()? {
                    match field {
                        Field::Disposition => {
                            if disposition.is_some() {
                                return Err(A::Error::duplicate_field("disposition"));
                            }
                            disposition = Some(map.next_value::<Tag>()?);
                        }
                        Field::Attachments => {
                            if attachments.is_some() {
                                return Err(A::Error::duplicate_field("attachments"));
                            }
                            attachments = Some(map.next_value::<AttachmentList>()?.0);
                        }
                        Field::Reason => {
                            if reason.is_some() {
                                return Err(A::Error::duplicate_field("reason"));
                            }
                            reason = Some(map.next_value()?);
                        }
                        Field::SourceBytes => {
                            if source_bytes.is_some() {
                                return Err(A::Error::duplicate_field("source_bytes"));
                            }
                            source_bytes = Some(map.next_value()?);
                        }
                    }
                }
                match disposition.ok_or_else(|| A::Error::missing_field("disposition"))? {
                    Tag::Unavailable => {
                        if attachments.is_some() {
                            return Err(A::Error::unknown_field("attachments", &["disposition"]));
                        }
                        if reason.is_some() {
                            return Err(A::Error::unknown_field("reason", &["disposition"]));
                        }
                        if source_bytes.is_some() {
                            return Err(A::Error::unknown_field("source_bytes", &["disposition"]));
                        }
                        Ok(PayloadRecord::Unavailable {})
                    }
                    Tag::Retained => {
                        if reason.is_some() {
                            return Err(A::Error::unknown_field(
                                "reason",
                                &["disposition", "attachments"],
                            ));
                        }
                        if source_bytes.is_some() {
                            return Err(A::Error::unknown_field(
                                "source_bytes",
                                &["disposition", "attachments"],
                            ));
                        }
                        Ok(PayloadRecord::Retained {
                            attachments: attachments
                                .ok_or_else(|| A::Error::missing_field("attachments"))?,
                        })
                    }
                    Tag::Discarded => {
                        if attachments.is_some() {
                            return Err(A::Error::unknown_field(
                                "attachments",
                                &["disposition", "reason", "source_bytes"],
                            ));
                        }
                        Ok(PayloadRecord::Discarded {
                            reason: reason.ok_or_else(|| A::Error::missing_field("reason"))?,
                            source_bytes: source_bytes
                                .ok_or_else(|| A::Error::missing_field("source_bytes"))?,
                        })
                    }
                    Tag::Quarantined => {
                        if attachments.is_some() {
                            return Err(A::Error::unknown_field("attachments", &["disposition"]));
                        }
                        if reason.is_some() {
                            return Err(A::Error::unknown_field("reason", &["disposition"]));
                        }
                        if source_bytes.is_some() {
                            return Err(A::Error::unknown_field("source_bytes", &["disposition"]));
                        }
                        Ok(PayloadRecord::Quarantined {})
                    }
                }
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}
