// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the closed ExitRecord record without untyped content buffering.
use super::*;
#[derive(Serialize)]
#[serde(tag = "subject", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum ExitRecord {
    Desktop { kind: ExitKindRecord, status: i64 },
    Supervisor { status: i64 },
    CaptureOnly,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ExitKindRecord {
    Normal,
    StartupCancelled,
    StartupFailed,
    Unexpected,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Tag {
    Desktop,
    Supervisor,
    CaptureOnly,
}
#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum Field {
    Subject,
    Kind,
    Status,
}
impl<'de> Deserialize<'de> for ExitRecord {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = ExitRecord;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a closed ExitRecord map")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                use serde::de::Error as _;
                let mut subject = None;
                let mut kind = None;
                let mut status = None;
                while let Some(field) = map.next_key::<Field>()? {
                    match field {
                        Field::Subject => {
                            if subject.is_some() {
                                return Err(A::Error::duplicate_field("subject"));
                            }
                            subject = Some(map.next_value::<Tag>()?);
                        }
                        Field::Kind => {
                            if kind.is_some() {
                                return Err(A::Error::duplicate_field("kind"));
                            }
                            kind = Some(map.next_value()?);
                        }
                        Field::Status => {
                            if status.is_some() {
                                return Err(A::Error::duplicate_field("status"));
                            }
                            status = Some(map.next_value()?);
                        }
                    }
                }
                match subject.ok_or_else(|| A::Error::missing_field("subject"))? {
                    Tag::Desktop => Ok(ExitRecord::Desktop {
                        kind: kind.ok_or_else(|| A::Error::missing_field("kind"))?,
                        status: status.ok_or_else(|| A::Error::missing_field("status"))?,
                    }),
                    Tag::CaptureOnly => {
                        if kind.is_some() || status.is_some() {
                            return Err(A::Error::custom(
                                "Capture-only attribution has no exit fields",
                            ));
                        }
                        Ok(ExitRecord::CaptureOnly)
                    }
                    Tag::Supervisor => {
                        if kind.is_some() {
                            return Err(A::Error::unknown_field("kind", &["subject", "status"]));
                        }
                        Ok(ExitRecord::Supervisor {
                            status: status.ok_or_else(|| A::Error::missing_field("status"))?,
                        })
                    }
                }
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}
