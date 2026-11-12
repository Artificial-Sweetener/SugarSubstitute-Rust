// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the closed SymbolsRecord record without untyped content buffering.
use super::*;
#[derive(Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub(in super::super) enum SymbolsRecord {
    NotRequested {},
    Unresolved {
        failure: FailureRecord,
        #[serde(deserialize_with = "required_option")]
        raw_frame: Option<RawRecord>,
    },
    Resolved {
        frame: FrameRecord,
    },
}
impl SymbolsRecord {
    pub(in super::super) fn into_facts(self) -> Result<SymbolOutcome, IncidentStoreError> {
        Ok(match self {
            Self::NotRequested {} => SymbolOutcome::NotRequested,
            Self::Resolved { frame } => SymbolOutcome::Resolved(frame.into_frame()?),
            Self::Unresolved { failure, raw_frame } => {
                SymbolOutcome::Unresolved(UnresolvedSymbols::new(
                    failure.into_failure(),
                    raw_frame
                        .map(|raw| {
                            Ok::<_, IncidentStoreError>(RawFaultFrame::new(
                                DumpId::new(raw.dump)?,
                                raw.module.into_facts()?,
                                raw.address,
                                raw.module_offset,
                            ))
                        })
                        .transpose()?,
                )?)
            }
        })
    }
}
impl From<&SymbolOutcome> for SymbolsRecord {
    fn from(value: &SymbolOutcome) -> Self {
        match value {
            SymbolOutcome::NotRequested => Self::NotRequested {},
            SymbolOutcome::Resolved(frame) => Self::Resolved { frame: frame.into() },
            SymbolOutcome::Unresolved(value) => Self::Unresolved {
                failure: value.failure().into(),
                raw_frame: value.raw_frame().map(|raw| RawRecord {
                    dump: *raw.dump().bytes(),
                    module: raw.module().into(),
                    address: raw.address(),
                    module_offset: raw.module_offset(),
                }),
            },
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Tag {
    NotRequested,
    Unresolved,
    Resolved,
}
#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum Field {
    Outcome,
    Failure,
    RawFrame,
    Frame,
}
impl<'de> Deserialize<'de> for SymbolsRecord {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = SymbolsRecord;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a closed SymbolsRecord map")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                use serde::de::Error as _;
                let mut outcome = None;
                let mut failure = None;
                let mut raw_frame = None;
                let mut frame = None;
                while let Some(field) = map.next_key::<Field>()? {
                    match field {
                        Field::Outcome => {
                            if outcome.is_some() {
                                return Err(A::Error::duplicate_field("outcome"));
                            }
                            outcome = Some(map.next_value::<Tag>()?);
                        }
                        Field::Failure => {
                            if failure.is_some() {
                                return Err(A::Error::duplicate_field("failure"));
                            }
                            failure = Some(map.next_value()?);
                        }
                        Field::RawFrame => {
                            if raw_frame.is_some() {
                                return Err(A::Error::duplicate_field("raw_frame"));
                            }
                            raw_frame = Some(map.next_value()?);
                        }
                        Field::Frame => {
                            if frame.is_some() {
                                return Err(A::Error::duplicate_field("frame"));
                            }
                            frame = Some(map.next_value()?);
                        }
                    }
                }
                match outcome.ok_or_else(|| A::Error::missing_field("outcome"))? {
                    Tag::NotRequested => {
                        if failure.is_some() {
                            return Err(A::Error::unknown_field("failure", &["outcome"]));
                        }
                        if raw_frame.is_some() {
                            return Err(A::Error::unknown_field("raw_frame", &["outcome"]));
                        }
                        if frame.is_some() {
                            return Err(A::Error::unknown_field("frame", &["outcome"]));
                        }
                        Ok(SymbolsRecord::NotRequested {})
                    }
                    Tag::Unresolved => {
                        if frame.is_some() {
                            return Err(A::Error::unknown_field(
                                "frame",
                                &["outcome", "failure", "raw_frame"],
                            ));
                        }
                        Ok(SymbolsRecord::Unresolved {
                            failure: failure.ok_or_else(|| A::Error::missing_field("failure"))?,
                            raw_frame: raw_frame
                                .ok_or_else(|| A::Error::missing_field("raw_frame"))?,
                        })
                    }
                    Tag::Resolved => {
                        if failure.is_some() {
                            return Err(A::Error::unknown_field("failure", &["outcome", "frame"]));
                        }
                        if raw_frame.is_some() {
                            return Err(A::Error::unknown_field(
                                "raw_frame",
                                &["outcome", "frame"],
                            ));
                        }
                        Ok(SymbolsRecord::Resolved {
                            frame: frame.ok_or_else(|| A::Error::missing_field("frame"))?,
                        })
                    }
                }
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}
