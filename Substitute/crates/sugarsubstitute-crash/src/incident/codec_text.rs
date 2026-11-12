// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain bounded domain text or its original constructor failure through strict record parsing.
use serde::{Deserialize, Serialize};
use sugarsubstitute_contracts::incident::{BuildIdentity, FileLabel, IncidentError, SymbolName};

pub(super) struct BuildRecord(Result<BuildIdentity, IncidentError>);
impl BuildRecord {
    pub(super) fn into_facts(self) -> Result<BuildIdentity, super::IncidentStoreError> {
        self.0.map_err(Into::into)
    }
}
impl From<&BuildIdentity> for BuildRecord {
    fn from(value: &BuildIdentity) -> Self {
        Self(Ok(value.clone()))
    }
}
impl Serialize for BuildRecord {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::Error as _;
        let value = self.0.as_ref().map_err(S::Error::custom)?;
        serializer.serialize_str(value.as_str())
    }
}
impl<'de> Deserialize<'de> for BuildRecord {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = BuildRecord;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("bounded domain text")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(BuildRecord(BuildIdentity::new(value)))
            }
        }
        deserializer.deserialize_str(Visitor)
    }
}

pub(super) struct LabelRecord(Result<FileLabel, IncidentError>);
impl LabelRecord {
    pub(super) fn into_facts(self) -> Result<FileLabel, super::IncidentStoreError> {
        self.0.map_err(Into::into)
    }
}
impl From<&FileLabel> for LabelRecord {
    fn from(value: &FileLabel) -> Self {
        Self(Ok(value.clone()))
    }
}
impl Serialize for LabelRecord {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::Error as _;
        let value = self.0.as_ref().map_err(S::Error::custom)?;
        serializer.serialize_str(value.as_str())
    }
}
impl<'de> Deserialize<'de> for LabelRecord {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = LabelRecord;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("bounded domain text")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(LabelRecord(FileLabel::new(value)))
            }
        }
        deserializer.deserialize_str(Visitor)
    }
}

pub(super) struct SymbolRecord(Result<SymbolName, IncidentError>);
impl SymbolRecord {
    pub(super) fn into_facts(self) -> Result<SymbolName, super::IncidentStoreError> {
        self.0.map_err(Into::into)
    }
}
impl From<&SymbolName> for SymbolRecord {
    fn from(value: &SymbolName) -> Self {
        Self(Ok(value.clone()))
    }
}
impl Serialize for SymbolRecord {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::Error as _;
        let value = self.0.as_ref().map_err(S::Error::custom)?;
        serializer.serialize_str(value.as_str())
    }
}
impl<'de> Deserialize<'de> for SymbolRecord {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = SymbolRecord;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("bounded domain text")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(SymbolRecord(SymbolName::new(value)))
            }
        }
        deserializer.deserialize_str(Visitor)
    }
}
