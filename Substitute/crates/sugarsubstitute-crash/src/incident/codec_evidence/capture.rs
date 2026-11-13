// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the closed CaptureRecord record without untyped content buffering.
use super::*;
#[derive(Serialize)]
#[serde(tag = "availability", rename_all = "snake_case", deny_unknown_fields)]
pub(in super::super) enum CaptureRecord {
    Awaiting {},
    Unavailable { reason: AbsenceRecord },
    Finished { dump: [u8; 16] },
}
impl CaptureRecord {
    pub(in super::super) fn into_facts(self) -> Result<CaptureEvidence, IncidentStoreError> {
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

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Tag {
    Awaiting,
    Unavailable,
    Finished,
}
#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum Field {
    Availability,
    Reason,
    Dump,
}
impl<'de> Deserialize<'de> for CaptureRecord {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = CaptureRecord;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a closed CaptureRecord map")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                use serde::de::Error as _;
                let mut availability = None;
                let mut reason = None;
                let mut dump = None;
                while let Some(field) = map.next_key::<Field>()? {
                    match field {
                        Field::Availability => {
                            if availability.is_some() {
                                return Err(A::Error::duplicate_field("availability"));
                            }
                            availability = Some(map.next_value::<Tag>()?);
                        }
                        Field::Reason => {
                            if reason.is_some() {
                                return Err(A::Error::duplicate_field("reason"));
                            }
                            reason = Some(map.next_value()?);
                        }
                        Field::Dump => {
                            if dump.is_some() {
                                return Err(A::Error::duplicate_field("dump"));
                            }
                            dump = Some(map.next_value()?);
                        }
                    }
                }
                match availability.ok_or_else(|| A::Error::missing_field("availability"))? {
                    Tag::Awaiting => {
                        if reason.is_some() {
                            return Err(A::Error::unknown_field("reason", &["availability"]));
                        }
                        if dump.is_some() {
                            return Err(A::Error::unknown_field("dump", &["availability"]));
                        }
                        Ok(CaptureRecord::Awaiting {})
                    }
                    Tag::Unavailable => {
                        if dump.is_some() {
                            return Err(A::Error::unknown_field(
                                "dump",
                                &["availability", "reason"],
                            ));
                        }
                        Ok(CaptureRecord::Unavailable {
                            reason: reason.ok_or_else(|| A::Error::missing_field("reason"))?,
                        })
                    }
                    Tag::Finished => {
                        if reason.is_some() {
                            return Err(A::Error::unknown_field(
                                "reason",
                                &["availability", "dump"],
                            ));
                        }
                        Ok(CaptureRecord::Finished {
                            dump: dump.ok_or_else(|| A::Error::missing_field("dump"))?,
                        })
                    }
                }
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}
