// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the closed FailureRecord record without untyped content buffering.
use super::*;
#[derive(Serialize)]
#[serde(tag = "mechanism", rename_all = "snake_case", deny_unknown_fields)]
pub(in super::super) enum FailureRecord {
    Unexplained {},
    Startup { code: StartupRecord },
    RustPanic {},
    Abort {},
    NativeFault { code: u32, address: u64 },
}
impl FailureRecord {
    pub(in super::super) fn into_facts(self) -> FailureEvidence {
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

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Tag {
    Unexplained,
    Startup,
    RustPanic,
    Abort,
    NativeFault,
}
#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum Field {
    Mechanism,
    Code,
    Address,
}
impl<'de> Deserialize<'de> for FailureRecord {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = FailureRecord;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a closed FailureRecord map")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                use serde::de::Error as _;
                let mut mechanism = None;
                let mut code: Option<CodeRecord> = None;
                let mut address = None;
                while let Some(field) = map.next_key::<Field>()? {
                    match field {
                        Field::Mechanism => {
                            if mechanism.is_some() {
                                return Err(A::Error::duplicate_field("mechanism"));
                            }
                            mechanism = Some(map.next_value::<Tag>()?);
                        }
                        Field::Code => {
                            if code.is_some() {
                                return Err(A::Error::duplicate_field("code"));
                            }
                            code = Some(map.next_value()?);
                        }
                        Field::Address => {
                            if address.is_some() {
                                return Err(A::Error::duplicate_field("address"));
                            }
                            address = Some(map.next_value()?);
                        }
                    }
                }
                match mechanism.ok_or_else(|| A::Error::missing_field("mechanism"))? {
                    Tag::Unexplained => {
                        if code.is_some() {
                            return Err(A::Error::unknown_field("code", &["mechanism"]));
                        }
                        if address.is_some() {
                            return Err(A::Error::unknown_field("address", &["mechanism"]));
                        }
                        Ok(FailureRecord::Unexplained {})
                    }
                    Tag::Startup => {
                        if address.is_some() {
                            return Err(A::Error::unknown_field("address", &["mechanism", "code"]));
                        }
                        Ok(FailureRecord::Startup {
                            code: code.ok_or_else(|| A::Error::missing_field("code"))?.startup()?,
                        })
                    }
                    Tag::RustPanic => {
                        if code.is_some() {
                            return Err(A::Error::unknown_field("code", &["mechanism"]));
                        }
                        if address.is_some() {
                            return Err(A::Error::unknown_field("address", &["mechanism"]));
                        }
                        Ok(FailureRecord::RustPanic {})
                    }
                    Tag::Abort => {
                        if code.is_some() {
                            return Err(A::Error::unknown_field("code", &["mechanism"]));
                        }
                        if address.is_some() {
                            return Err(A::Error::unknown_field("address", &["mechanism"]));
                        }
                        Ok(FailureRecord::Abort {})
                    }
                    Tag::NativeFault => Ok(FailureRecord::NativeFault {
                        code: code.ok_or_else(|| A::Error::missing_field("code"))?.native()?,
                        address: address.ok_or_else(|| A::Error::missing_field("address"))?,
                    }),
                }
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}

/// The shared field has two bounded scalar representations; no JSON value tree is retained.
enum CodeRecord {
    Startup(StartupRecord),
    Native(u32),
}
impl CodeRecord {
    fn startup<E: serde::de::Error>(self) -> Result<StartupRecord, E> {
        match self {
            Self::Startup(value) => Ok(value),
            Self::Native(value) => Err(E::invalid_type(
                serde::de::Unexpected::Unsigned(u64::from(value)),
                &"a startup failure code",
            )),
        }
    }
    fn native<E: serde::de::Error>(self) -> Result<u32, E> {
        match self {
            Self::Native(value) => Ok(value),
            Self::Startup(_) => Err(E::invalid_type(
                serde::de::Unexpected::Str("startup failure code"),
                &"a native fault u32",
            )),
        }
    }
}
impl<'de> Deserialize<'de> for CodeRecord {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct CodeVisitor;
        impl<'de> serde::de::Visitor<'de> for CodeVisitor {
            type Value = CodeRecord;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a startup code string or native u32")
            }
            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
                let value = u32::try_from(value).map_err(|_| {
                    E::invalid_value(serde::de::Unexpected::Unsigned(value), &"a native fault u32")
                })?;
                Ok(CodeRecord::Native(value))
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                use serde::de::IntoDeserializer;
                StartupRecord::deserialize(value.into_deserializer()).map(CodeRecord::Startup)
            }
        }
        deserializer.deserialize_any(CodeVisitor)
    }
}
