// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Dispatch closed store variants through typed maps without buffering arbitrary JSON content.
use super::*;
use serde::de::{DeserializeSeed, Error as _, IgnoredAny, IntoDeserializer, MapAccess, Visitor};
#[derive(Serialize)]
#[serde(tag = "record", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum StoreRecord {
    Ledger { ledger: LedgerRecord },
    SummaryEviction { journal: Box<JournalRecord> },
    CapturePromotion { journal: Box<CaptureJournalRecord> },
    SummaryMigration(Box<MigrationRecord>),
    SummaryMigrationArchive(Box<MigrationArchiveRecord>),
    PayloadDiscard { journal: Box<PayloadDiscardRecord> },
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Ledger,
    SummaryEviction,
    CapturePromotion,
    SummaryMigration,
    SummaryMigrationArchive,
    PayloadDiscard,
}
#[derive(Clone, Copy, Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum Field {
    Record,
    Ledger,
    Journal,
    Operation,
    Incident,
    Revision,
    PayloadRevision,
    SourceSchema,
    CandidateSchema,
    OriginalFile,
    OriginalStage,
    CandidateStage,
    Migration,
}
impl Field {
    fn name(self) -> &'static str {
        match self {
            Self::Record => "record",
            Self::Ledger => "ledger",
            Self::Journal => "journal",
            Self::Operation => "operation",
            Self::Incident => "incident",
            Self::Revision => "revision",
            Self::PayloadRevision => "payload_revision",
            Self::SourceSchema => "source_schema",
            Self::CandidateSchema => "candidate_schema",
            Self::OriginalFile => "original_file",
            Self::OriginalStage => "original_stage",
            Self::CandidateStage => "candidate_stage",
            Self::Migration => "migration",
        }
    }
}
impl Kind {
    fn fields(self) -> &'static [&'static str] {
        match self {
            Self::Ledger => &["record", "ledger"],
            Self::SummaryEviction | Self::CapturePromotion | Self::PayloadDiscard => {
                &["record", "journal"]
            }
            Self::SummaryMigration => &[
                "record",
                "operation",
                "incident",
                "revision",
                "payload_revision",
                "source_schema",
                "candidate_schema",
                "original_file",
                "original_stage",
                "candidate_stage",
            ],
            Self::SummaryMigrationArchive => &["record", "migration"],
        }
    }
}
struct Discriminator(Kind);
impl<'de> Deserialize<'de> for Discriminator {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct TagVisitor;
        impl<'de> Visitor<'de> for TagVisitor {
            type Value = Discriminator;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a closed store discriminator")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut kind = None;
                let mut seen = 0_u16;
                while let Some(field) = map.next_key::<Field>()? {
                    let bit = 1_u16 << (field as u8);
                    if seen & bit != 0 {
                        return Err(A::Error::duplicate_field(field.name()));
                    }
                    seen |= bit;
                    match field {
                        Field::Record => kind = Some(map.next_value::<Kind>()?),
                        _ => {
                            map.next_value::<IgnoredAny>()?;
                        }
                    }
                }
                Ok(Discriminator(kind.ok_or_else(|| A::Error::missing_field("record"))?))
            }
        }
        deserializer.deserialize_map(TagVisitor)
    }
}
struct TaggedMap<A> {
    map: A,
    kind: Kind,
    tag: bool,
}
impl<'de, A: MapAccess<'de>> MapAccess<'de> for TaggedMap<A> {
    type Error = A::Error;
    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Self::Error> {
        loop {
            let Some(field) = self.map.next_key::<Field>()? else {
                if !self.tag {
                    return Err(A::Error::missing_field("record"));
                }
                return Ok(None);
            };
            if !self.kind.fields().contains(&field.name()) {
                return Err(A::Error::unknown_field(field.name(), self.kind.fields()));
            }
            if matches!(field, Field::Record) {
                if self.tag {
                    return Err(A::Error::duplicate_field("record"));
                }
                self.tag = true;
                if self.map.next_value::<Kind>()? != self.kind {
                    return Err(A::Error::custom(
                        "store discriminator changed between bounded passes",
                    ));
                }
                continue;
            }
            return seed.deserialize(field.name().into_deserializer()).map(Some);
        }
    }
    fn next_value_seed<V: DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, Self::Error> {
        self.map.next_value_seed(seed)
    }
}
struct TypedVisitor<T> {
    kind: Kind,
    marker: std::marker::PhantomData<T>,
}
impl<'de, T: Deserialize<'de>> Visitor<'de> for TypedVisitor<T> {
    type Value = T;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a typed closed store record")
    }
    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
        T::deserialize(serde::de::value::MapAccessDeserializer::new(TaggedMap {
            map,
            kind: self.kind,
            tag: false,
        }))
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JournalMember<T> {
    journal: Box<T>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LedgerMember {
    ledger: LedgerRecord,
}
struct ContentSeed(Kind);
impl<'de> DeserializeSeed<'de> for ContentSeed {
    type Value = StoreRecord;
    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error> {
        fn typed<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
            deserializer: D,
            kind: Kind,
        ) -> Result<T, D::Error> {
            deserializer.deserialize_map(TypedVisitor { kind, marker: std::marker::PhantomData })
        }
        match self.0 {
            Kind::Ledger => Ok(StoreRecord::Ledger {
                ledger: typed::<_, LedgerMember>(deserializer, self.0)?.ledger,
            }),
            Kind::SummaryEviction => Ok(StoreRecord::SummaryEviction {
                journal: typed::<_, JournalMember<JournalRecord>>(deserializer, self.0)?.journal,
            }),
            Kind::CapturePromotion => Ok(StoreRecord::CapturePromotion {
                journal: typed::<_, JournalMember<CaptureJournalRecord>>(deserializer, self.0)?
                    .journal,
            }),
            Kind::PayloadDiscard => Ok(StoreRecord::PayloadDiscard {
                journal: typed::<_, JournalMember<PayloadDiscardRecord>>(deserializer, self.0)?
                    .journal,
            }),
            Kind::SummaryMigration => {
                Ok(StoreRecord::SummaryMigration(Box::new(typed::<_, MigrationRecord>(
                    deserializer,
                    self.0,
                )?)))
            }
            Kind::SummaryMigrationArchive => {
                Ok(StoreRecord::SummaryMigrationArchive(Box::new(typed::<
                    _,
                    MigrationArchiveRecord,
                >(
                    deserializer, self.0
                )?)))
            }
        }
    }
}
#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum EnvelopeField {
    SchemaVersion,
    Content,
}
struct EnvelopeSeed(Kind);
impl<'de> DeserializeSeed<'de> for EnvelopeSeed {
    type Value = (u32, StoreRecord);
    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error> {
        struct EnvelopeVisitor(Kind);
        impl<'de> Visitor<'de> for EnvelopeVisitor {
            type Value = (u32, StoreRecord);
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a schema-one store envelope")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut schema = None;
                let mut content = None;
                while let Some(field) = map.next_key::<EnvelopeField>()? {
                    match field {
                        EnvelopeField::SchemaVersion => {
                            if schema.is_some() {
                                return Err(A::Error::duplicate_field("schema_version"));
                            }
                            schema = Some(map.next_value()?);
                        }
                        EnvelopeField::Content => {
                            if content.is_some() {
                                return Err(A::Error::duplicate_field("content"));
                            }
                            content = Some(map.next_value_seed(ContentSeed(self.0))?);
                        }
                    }
                }
                Ok((
                    schema.ok_or_else(|| A::Error::missing_field("schema_version"))?,
                    content.ok_or_else(|| A::Error::missing_field("content"))?,
                ))
            }
        }
        deserializer.deserialize_map(EnvelopeVisitor(self.0))
    }
}
pub(super) fn decode(bytes: &[u8]) -> Result<StoreRecord, IncidentStoreError> {
    if bytes.len() > STATE_BOUND {
        return Err(IncidentStoreError::RecordBound);
    }
    // The first pinned parser and its scratch drop before typed content decoding.
    let kind =
        super::super::codec_parse::parse::<super::super::codec::Record<Discriminator>>(bytes)?
            .content
            .0;
    let (schema, content) = super::super::codec_parse::parse_seed(bytes, EnvelopeSeed(kind))?;
    if schema != 1 {
        return Err(IncidentStoreError::UnsupportedSchema(schema));
    }
    Ok(content)
}
