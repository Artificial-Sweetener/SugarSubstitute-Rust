// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Enforce explicit schema and encoded-byte limits before constructing domain facts.
use super::{
    IncidentStoreError,
    codec_seed::SeedRecord,
    codec_symbols::{ReleaseRecord, SymbolsRecord},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sugarsubstitute_contracts::incident::{
    IncidentError, IncidentSeed, IncidentUtc, ReleaseSymbols, SymbolOutcome,
};

pub(super) fn required_option<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(deserializer)
}
pub(super) fn utc(seconds: u64) -> Result<IncidentUtc, IncidentStoreError> {
    let seconds = i64::try_from(seconds).map_err(|_| IncidentError::InvalidTime)?;
    IncidentUtc::from_unix_seconds(seconds).map_err(Into::into)
}

pub(super) const SEED_BOUND: usize = 4096;
pub(super) const STATE_BOUND: usize = 65_536;
pub(super) const REPORT_BOUND: usize = 262_144;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record<T> {
    pub(super) schema_version: u32,
    pub(super) content: T,
}

pub(super) fn decode<T: DeserializeOwned>(
    bytes: &[u8],
    bound: usize,
) -> Result<T, IncidentStoreError> {
    if bytes.len() > bound {
        return Err(IncidentStoreError::RecordBound);
    }
    let record: Record<T> = super::codec_parse::parse(bytes)?;
    if record.schema_version != 1 {
        return Err(IncidentStoreError::UnsupportedSchema(record.schema_version));
    }
    Ok(record.content)
}
/// Decode a schema-one seed within 4 KiB, preserving full historical subject identity.
/// # Errors
/// Rejects unknown fields/versions, invalid identities and inconsistent seed facts.
pub fn decode_seed(bytes: &[u8]) -> Result<IncidentSeed, IncidentStoreError> {
    decode::<SeedRecord>(bytes, SEED_BOUND)?.into_seed()
}
/// Encode canonical schema-one seed bytes without granting durable publication authority.
/// # Errors
/// Returns an encoding error or the fixed encoded-byte bound error.
pub fn encode_seed(seed: &IncidentSeed) -> Result<Vec<u8>, IncidentStoreError> {
    super::codec_encode::encode(1, SeedRecord::from(seed), SEED_BOUND)
}
#[cfg(windows)]
pub(super) fn write_seed<W: std::io::Write>(
    seed: &IncidentSeed,
    output: W,
) -> Result<(), IncidentStoreError> {
    super::codec_encode::write(1, &SeedRecord::from(seed), output)
}
/// Decode exact release matching facts; packaged installation provenance is separate.
/// # Errors
/// Rejects malformed/unknown schema and invalid build, label, architecture or PDB identity.
pub fn decode_release_symbols(bytes: &[u8]) -> Result<ReleaseSymbols, IncidentStoreError> {
    decode::<ReleaseRecord>(bytes, STATE_BOUND)?.into_release()
}
/// Encode exact release association without opening PE/PDB or granting symbol lookup.
/// # Errors
/// Rejects encoding/bound errors.
pub fn encode_release_symbols(release: &ReleaseSymbols) -> Result<Vec<u8>, IncidentStoreError> {
    super::codec_encode::encode(1, ReleaseRecord::from(release), STATE_BOUND)
}
/// Decode one explicit failing-frame outcome within 256 KiB, preserving unavailable facts.
/// # Errors
/// Rejects malformed/unknown schema, invalid identities and fabricated raw unavailable frames.
pub fn decode_symbol_report(bytes: &[u8]) -> Result<SymbolOutcome, IncidentStoreError> {
    decode::<SymbolsRecord>(bytes, REPORT_BOUND)?.into_facts()
}
/// Encode one failing-frame outcome; this does not claim a fully unwound stack.
/// # Errors
/// Rejects encoding/bound errors.
pub fn encode_symbol_report(outcome: &SymbolOutcome) -> Result<Vec<u8>, IncidentStoreError> {
    super::codec_encode::encode(1, SymbolsRecord::from(outcome), REPORT_BOUND)
}
