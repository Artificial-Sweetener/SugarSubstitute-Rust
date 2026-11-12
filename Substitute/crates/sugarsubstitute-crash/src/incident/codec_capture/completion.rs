// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the closed CompletionRecord record without untyped content buffering.
use super::*;
#[derive(Serialize)]
#[serde(tag = "disposition", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum CompletionRecord {
    Promoted { summary_file: FileRecord, payload_file: FileRecord },
    Discarded { summary_file: FileRecord, reason: DiscardRecord, source_bytes: u64 },
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Tag {
    Promoted,
    Discarded,
}
#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum Field {
    Disposition,
    SummaryFile,
    PayloadFile,
    Reason,
    SourceBytes,
}
impl<'de> Deserialize<'de> for CompletionRecord {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = CompletionRecord;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a closed CompletionRecord map")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                use serde::de::Error as _;
                let mut disposition = None;
                let mut summary_file = None;
                let mut payload_file = None;
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
                        Field::SummaryFile => {
                            if summary_file.is_some() {
                                return Err(A::Error::duplicate_field("summary_file"));
                            }
                            summary_file = Some(map.next_value()?);
                        }
                        Field::PayloadFile => {
                            if payload_file.is_some() {
                                return Err(A::Error::duplicate_field("payload_file"));
                            }
                            payload_file = Some(map.next_value()?);
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
                    Tag::Promoted => {
                        if reason.is_some() {
                            return Err(A::Error::unknown_field(
                                "reason",
                                &["disposition", "summary_file", "payload_file"],
                            ));
                        }
                        if source_bytes.is_some() {
                            return Err(A::Error::unknown_field(
                                "source_bytes",
                                &["disposition", "summary_file", "payload_file"],
                            ));
                        }
                        Ok(CompletionRecord::Promoted {
                            summary_file: summary_file
                                .ok_or_else(|| A::Error::missing_field("summary_file"))?,
                            payload_file: payload_file
                                .ok_or_else(|| A::Error::missing_field("payload_file"))?,
                        })
                    }
                    Tag::Discarded => {
                        if payload_file.is_some() {
                            return Err(A::Error::unknown_field(
                                "payload_file",
                                &["disposition", "summary_file", "reason", "source_bytes"],
                            ));
                        }
                        Ok(CompletionRecord::Discarded {
                            summary_file: summary_file
                                .ok_or_else(|| A::Error::missing_field("summary_file"))?,
                            reason: reason.ok_or_else(|| A::Error::missing_field("reason"))?,
                            source_bytes: source_bytes
                                .ok_or_else(|| A::Error::missing_field("source_bytes"))?,
                        })
                    }
                }
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}
