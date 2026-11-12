// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the closed AttemptRecord record without untyped content buffering.
use super::*;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReportingRecord {
    pub(super) attempts: u64,
    #[serde(deserialize_with = "super::super::codec::required_option")]
    pub(super) last: Option<AttemptRecord>,
}
#[derive(Serialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum AttemptRecord {
    Presented {},
    Failed { cause: ReportFailureRecord },
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ReportFailureRecord {
    Launch,
    Load,
    Presentation,
    UnexpectedExit,
    Acknowledgement,
    Export,
    Retirement,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Tag {
    Presented,
    Failed,
}
#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum Field {
    Result,
    Cause,
}
impl<'de> Deserialize<'de> for AttemptRecord {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = AttemptRecord;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a closed AttemptRecord map")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                use serde::de::Error as _;
                let mut result = None;
                let mut cause = None;
                while let Some(field) = map.next_key::<Field>()? {
                    match field {
                        Field::Result => {
                            if result.is_some() {
                                return Err(A::Error::duplicate_field("result"));
                            }
                            result = Some(map.next_value::<Tag>()?);
                        }
                        Field::Cause => {
                            if cause.is_some() {
                                return Err(A::Error::duplicate_field("cause"));
                            }
                            cause = Some(map.next_value()?);
                        }
                    }
                }
                match result.ok_or_else(|| A::Error::missing_field("result"))? {
                    Tag::Presented => {
                        if cause.is_some() {
                            return Err(A::Error::unknown_field("cause", &["result"]));
                        }
                        Ok(AttemptRecord::Presented {})
                    }
                    Tag::Failed => Ok(AttemptRecord::Failed {
                        cause: cause.ok_or_else(|| A::Error::missing_field("cause"))?,
                    }),
                }
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}
