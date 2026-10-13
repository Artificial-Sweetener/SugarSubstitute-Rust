// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the closed SubjectRecord record without untyped content buffering.
use super::*;
#[derive(Serialize)]
#[serde(tag = "subject", rename_all = "snake_case", deny_unknown_fields)]
pub(in super::super) enum SubjectRecord {
    Desktop { owner: OwnerRecord, instance: u64, generation: u64 },
    Supervisor { owner: OwnerRecord },
}
impl SubjectRecord {
    pub(in super::super) fn into_key(self) -> Result<IncidentKey, IncidentStoreError> {
        Ok(IncidentKey::new(match self {
            Self::Supervisor { owner } => IncidentSubject::Supervisor(owner.into_owner()?),
            Self::Desktop { owner, instance, generation } => IncidentSubject::Desktop(RunKey {
                owner: owner.into_owner()?,
                instance: InstanceId::new(instance)?,
                generation: RunGeneration::new(generation)?,
            }),
        }))
    }
}
impl From<IncidentKey> for SubjectRecord {
    fn from(key: IncidentKey) -> Self {
        match key.subject() {
            IncidentSubject::Supervisor(owner) => Self::Supervisor { owner: owner.into() },
            IncidentSubject::Desktop(run) => Self::Desktop {
                owner: run.owner.into(),
                instance: run.instance.get(),
                generation: run.generation.get(),
            },
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Tag {
    Desktop,
    Supervisor,
}
#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum Field {
    Subject,
    Owner,
    Instance,
    Generation,
}
impl<'de> Deserialize<'de> for SubjectRecord {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = SubjectRecord;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a closed SubjectRecord map")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                use serde::de::Error as _;
                let mut subject = None;
                let mut owner = None;
                let mut instance = None;
                let mut generation = None;
                while let Some(field) = map.next_key::<Field>()? {
                    match field {
                        Field::Subject => {
                            if subject.is_some() {
                                return Err(A::Error::duplicate_field("subject"));
                            }
                            subject = Some(map.next_value::<Tag>()?);
                        }
                        Field::Owner => {
                            if owner.is_some() {
                                return Err(A::Error::duplicate_field("owner"));
                            }
                            owner = Some(map.next_value()?);
                        }
                        Field::Instance => {
                            if instance.is_some() {
                                return Err(A::Error::duplicate_field("instance"));
                            }
                            instance = Some(map.next_value()?);
                        }
                        Field::Generation => {
                            if generation.is_some() {
                                return Err(A::Error::duplicate_field("generation"));
                            }
                            generation = Some(map.next_value()?);
                        }
                    }
                }
                match subject.ok_or_else(|| A::Error::missing_field("subject"))? {
                    Tag::Desktop => Ok(SubjectRecord::Desktop {
                        owner: owner.ok_or_else(|| A::Error::missing_field("owner"))?,
                        instance: instance.ok_or_else(|| A::Error::missing_field("instance"))?,
                        generation: generation
                            .ok_or_else(|| A::Error::missing_field("generation"))?,
                    }),
                    Tag::Supervisor => {
                        if instance.is_some() {
                            return Err(A::Error::unknown_field("instance", &["subject", "owner"]));
                        }
                        if generation.is_some() {
                            return Err(A::Error::unknown_field(
                                "generation",
                                &["subject", "owner"],
                            ));
                        }
                        Ok(SubjectRecord::Supervisor {
                            owner: owner.ok_or_else(|| A::Error::missing_field("owner"))?,
                        })
                    }
                }
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}
