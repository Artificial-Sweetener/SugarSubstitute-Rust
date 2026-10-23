// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Protected launcher ownership is durable before any root executable is published.
use super::*;
use serde::{Deserialize, Serialize};
use sugarsubstitute_contracts::{
    application_launch::{LAUNCHER_CONTRACT_FILE, MAX_CONTRACT_BYTES},
    installation::Generation,
};

const MAX_RECORD_BYTES: usize = 32 * 1024;

/// Retain the exact admitted launcher and its repair source independently of app selection.
/// The recorded source generation is an owned recovery input.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    schema_version: u32,
    pub(super) source: Generation,
    pub(super) bytes: u64,
    pub(super) sha256: String,
    pub(super) contract: String,
}
impl Record {
    /// Only a verified accepted payload may establish initial launcher ownership.
    pub(super) fn from_application(
        application: &VerifiedApplication,
        context: &JobContext,
    ) -> io::Result<Self> {
        let mut source = application.directory.read(EXECUTABLE)?;
        let mut declaration = application.directory.read(LAUNCHER_CONTRACT_FILE)?;
        let contract =
            String::from_utf8(files::read_bounded(&mut declaration, MAX_CONTRACT_BYTES)?)
                .map_err(io::Error::other)?;
        let record = Self {
            schema_version: 1,
            source: application.generation.clone(),
            bytes: source.metadata()?.len(),
            sha256: payload::hash(&mut source, context)?,
            contract,
        };
        record.validate()?;
        Ok(record)
    }
    /// Bound persisted fields before using them for native copying or protocol admission.
    fn validate(&self) -> io::Result<()> {
        if self.schema_version != 1
            || self.bytes == 0
            || self.bytes > 2 * 1024 * 1024 * 1024
            || self.sha256.len() != 64
            || !self.sha256.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(invalid("Stable launcher ownership record is unsupported or invalid"));
        }
        LauncherContract::decode(self.contract.as_bytes()).map_err(io::Error::other)?;
        Ok(())
    }
}

/// Missing state permits first publication; invalid state is retained for explicit recovery.
pub(super) fn read(state: &Directory, protection: &Protection) -> io::Result<Option<Record>> {
    let mut file = match state.read(RECORD) {
        Ok(file) => file,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
            return match std::fs::symlink_metadata(state.path().join(NAME)) {
                Ok(_) => Err(invalid(
                    "Stable launcher ownership is missing; existing shell state has been preserved",
                )),
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(cause) => Err(cause),
            };
        }
        Err(cause) => return Err(cause),
    };
    protection.verify_file(&file)?;
    let record: Record = serde_json::from_slice(&files::read_bounded(&mut file, MAX_RECORD_BYTES)?)
        .map_err(io::Error::other)?;
    record.validate()?;
    Ok(Some(record))
}

/// Create-only publication cannot overwrite another launcher owner or an interrupted record.
pub(super) fn publish(
    state: &Directory,
    record: &Record,
    protection: &Protection,
    context: &JobContext,
) -> io::Result<()> {
    record.validate()?;
    let bytes = serde_json::to_vec(record).map_err(io::Error::other)?;
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(invalid("Stable launcher ownership exceeds its bound"));
    }
    check(context)?;
    files::write_immutable(&state.path().join(RECORD), &bytes, |path| {
        protection.file(path, context)
    })
}
