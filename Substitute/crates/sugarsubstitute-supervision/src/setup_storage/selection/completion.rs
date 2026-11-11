// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain an explicit installation choice independently of expiring update discovery evidence.
use super::*;
use serde::{Deserialize, Serialize};
use sugarsubstitute_contracts::application_launch::installation::AUTHORIZATION_SCHEMA;
use sugarsubstitute_contracts::installation::{Generation, InstallationId};

pub(in crate::setup_storage) const FILE: &str = "installation-authorization-v1.record";
const LIMIT: usize = 4096;

/// Immutable permission for this exact prepared generation, distinct from application health.
#[derive(Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct Record {
    schema_version: u32,
    installation: InstallationId,
    installation_directory: (u32, u32, u32),
    generation: Generation,
    generation_directory: (u32, u32, u32),
    updater_protocol: u32,
    minimum_updater_release: u64,
}

/// Pins the locally committed choice; no wall clock, cache or network response can grant it.
pub(in crate::setup_storage) struct InstallationAuthorization {
    record: Record,
    _file: File,
}
impl InstallationAuthorization {
    /// Reopened runtime must include the declared reader and engine promised at completion.
    pub(in crate::setup_storage) fn require_payload(
        &self,
        compatibility: &crate::setup_storage::compatibility::Compatibility,
    ) -> io::Result<()> {
        compatibility.require_installation_activation(self.record.minimum_updater_release)
    }
    /// A copied receipt, superseded generation or older engine never admits a trial.
    pub(in crate::setup_storage) fn require(
        &self,
        installation: &Directory,
        directory: &Directory,
        snapshot: &InstallationRecord,
    ) -> io::Result<()> {
        let record = &self.record;
        if record.schema_version != AUTHORIZATION_SCHEMA
            || &record.installation != snapshot.installation()
            || record.installation_directory != installation.identity()?
            || record.generation_directory != directory.identity()?
            || record.updater_protocol != sugarsubstitute_update::ENGINE_PROTOCOL
            || record.minimum_updater_release == 0
            || record.minimum_updater_release > sugarsubstitute_update::ENGINE_RELEASE
            || !matches!(snapshot.candidate(), Some(Candidate::Prepared { generation }) if generation == &record.generation)
        {
            return Err(super::super::invalid(
                "Installation authorization does not match this prepared application or engine",
            ));
        }
        Ok(())
    }
}

impl SelectionStore {
    /// Commit only under current signed authority and exact selection writer custody.
    /// The caller retains setup exclusion, runtime files and the repository trust writer.
    /// An existing different record is preserved; retrying the identical choice is idempotent.
    pub(in crate::setup_storage) fn authorize_installation(
        &self,
        expected: &InstallationRecord,
        directory: &Directory,
        minimum_updater_release: u64,
        recheck: &mut dyn FnMut() -> io::Result<()>,
        context: &JobContext,
    ) -> Result<InstallationAuthorization, SelectionFailure> {
        self.protected(context, |protection| {
            let _writer = writer(&self.root, protection, context)?;
            if self.read(protection)?.as_ref() != Some(expected) {
                return Err(SelectionFailure::Changed);
            }
            let Some(Candidate::Prepared { generation }) = expected.candidate() else {
                return Err(SelectionFailure::Changed);
            };
            let record = Record {
                schema_version: AUTHORIZATION_SCHEMA,
                installation: expected.installation().clone(),
                installation_directory: self._installation.identity()?,
                generation: generation.clone(),
                generation_directory: directory.identity()?,
                updater_protocol: sugarsubstitute_update::ENGINE_PROTOCOL,
                minimum_updater_release,
            };
            let bytes = serde_json::to_vec(&record).map_err(io::Error::other)?;
            if bytes.len() > LIMIT
                || minimum_updater_release == 0
                || minimum_updater_release > sugarsubstitute_update::ENGINE_RELEASE
            {
                return Err(super::super::invalid("Unsupported installation authorization").into());
            }
            check(context)?;
            recheck()?;
            super::super::publication::recover(directory, protection, context)?;
            recheck()?;
            super::super::publication::write_immutable(
                directory, FILE, &bytes, protection, context,
            )?;
            read(directory, protection)?.ok_or_else(|| {
                SelectionFailure::from(super::super::invalid(
                    "Installation authorization disappeared",
                ))
            })
        })
    }

    /// Read only the current prepared choice. Absence is not implicit authorization.
    /// Both native directories and the logical selection must still match under its writer.
    pub(in crate::setup_storage) fn installation_authorization(
        &self,
        expected: &InstallationRecord,
        directory: &Directory,
        context: &JobContext,
    ) -> Result<Option<InstallationAuthorization>, SelectionFailure> {
        self.protected(context, |protection| {
            let _writer = writer(&self.root, protection, context)?;
            if self.read(protection)?.as_ref() != Some(expected) {
                return Err(SelectionFailure::Changed);
            }
            let authorization = read(directory, protection)?;
            if let Some(authorization) = &authorization {
                authorization.require(&self._installation, directory, expected)?;
            }
            check(context)?;
            Ok(authorization)
        })
    }
}

/// Validate protection and bounded decoding on the same retained file handle.
pub(super) fn read(
    directory: &Directory,
    protection: &Protection,
) -> io::Result<Option<InstallationAuthorization>> {
    let mut file = match directory.read(FILE) {
        Ok(file) => file,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(cause) => return Err(cause),
    };
    protection.verify_file(&file)?;
    let record = serde_json::from_slice(&files::read_bounded(&mut file, LIMIT)?)
        .map_err(io::Error::other)?;
    Ok(Some(InstallationAuthorization { record, _file: file }))
}
