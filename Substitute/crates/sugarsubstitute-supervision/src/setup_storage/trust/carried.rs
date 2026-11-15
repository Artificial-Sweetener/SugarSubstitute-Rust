// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep rollback floors outside the trust folder, so fresh trust after a reset starts at them.
//!
//! The record sits in the state folder beside the trust folder and is bound to the same
//! installation and repository. A reset sets the trust folder aside but keeps this record, and
//! fresh trust carries its floors: metadata older than what the installation already accepted is
//! never accepted again, and neither is an updater older than the highest one its trust admitted.
//! Floors here only rise.
//!
//! Trust writes the record when it is established and raises it on every open and publication,
//! always from the release history it enforces. No single lost file lowers a floor: a lost
//! record is written again from the trust, and lost trust is set up again from the record. When
//! both are gone, a reset has nothing to start from and refuses.
use super::*;
use serde::{Deserialize, Serialize};
use sugarsubstitute_contracts::update::{Channel, Component};
use sugarsubstitute_update::VersionFloors;

/// The record in the installation's state folder.
pub(super) const RECORD: &str = "update-floors";
/// Prefix of the record's stage in the state folder.
const STAGE: &str = ".update-floors-stage";
/// Bound on the record's payload.
pub(super) const MAX_BYTES: usize = 1024;

/// Floors kept for one installation's trust.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Carried {
    schema_version: u32,
    /// Root and role version floors of the trust.
    pub(super) metadata: VersionFloors,
    /// Highest updater release the installation's trust admitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) updater: Option<KeptRelease>,
}
impl Carried {
    /// The higher of each floor of `self` and `other`.
    fn max(&self, other: &Self) -> Self {
        let updater = match (self.updater, other.updater) {
            (Some(kept), Some(newer)) if newer.rank() > kept.rank() => Some(newer),
            (kept, newer) => kept.or(newer),
        };
        Self { schema_version: 1, metadata: self.metadata.max(other.metadata), updater }
    }
}

/// One release floor: the channel it was published on, its sequence and, when the trust knows
/// it, the one archive admitted at that sequence.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct KeptRelease {
    pub(super) channel: Channel,
    pub(super) sequence: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) archive_sha256: Option<[u8; 32]>,
}
impl KeptRelease {
    /// Order floors by sequence, and at one sequence prefer the floor that names its archive.
    fn rank(&self) -> (u64, bool) {
        (self.sequence, self.archive_sha256.is_some())
    }
}

impl TrustStore {
    /// Raise the floors kept outside the trust folder to the ones this trust enforces. The caller
    /// holds the trust writer.
    ///
    /// The updater floor is the highest one in this trust's release history, which is what
    /// refuses an older updater here. A kept record that can no longer be read is written again
    /// from this trust, whose floors are never below the ones it was set up with.
    pub(super) fn carry_floors(
        &self,
        protection: &Protection,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        let state = ownership::state(&self._installation_root)?;
        let kept = match read(&state, &self.installation, &self.repository, protection) {
            Ok(kept) => kept,
            Err(cause) => {
                check(context)?;
                tracing::warn!(operation = "update_floors_rewritten", error = %cause,
                    "Kept update floors could not be read and are written again from the trust");
                None
            }
        };
        let updater = self
            .read_releases(protection)?
            .starts()
            .into_iter()
            .filter(|start| start.component == Component::Updater)
            .map(|start| KeptRelease {
                channel: start.channel,
                sequence: start.sequence,
                archive_sha256: start.digest,
            })
            .max_by_key(KeptRelease::rank);
        let current = Carried {
            schema_version: 1,
            metadata: self.read(protection)?.version_floors()?,
            updater,
        };
        let raised = kept.clone().unwrap_or_default().max(&current);
        if kept.as_ref() == Some(&raised) {
            return Ok(());
        }
        let payload = serde_json::to_vec(&raised).map_err(io::Error::other)?;
        let bytes = envelope::encode(
            envelope::Kind::Floors,
            &self.installation,
            &self.repository,
            &payload,
        )?;
        let destination = state.path().join(RECORD);
        let mut write = write_port::begin(UpdateFile::Trust, &destination);
        protection::with(context, |owner| staging::reclaim(&state, STAGE, &[], owner, context))?;
        let staged = staging::unique_path(&state, STAGE)?;
        let mut file = protection.file(&staged, context)?;
        write.bytes(&mut file, &bytes)?;
        files::sync(&file, context)?;
        drop(file);
        check(context)?;
        write.publish(|| staging::replace_record(&staged, &destination))?;
        Ok(())
    }
}

/// The floors kept in `state` for `installation` and `repository`, or `None` when none are kept.
/// # Errors
/// The record is unprotected, foreign, malformed or of an unknown schema.
pub(super) fn read(
    state: &Directory,
    installation: &InstallationId,
    repository: &[u8; 32],
    protection: &Protection,
) -> Result<Option<Carried>, TrustFailure> {
    let Some(payload) = records::read_bound(
        state,
        RECORD,
        envelope::Kind::Floors,
        installation,
        repository,
        protection,
    )?
    else {
        return Ok(None);
    };
    let carried: Carried = serde_json::from_slice(&payload).map_err(io::Error::other)?;
    if carried.schema_version != 1 {
        return Err(invalid("Kept update floors have an unknown schema").into());
    }
    Ok(Some(carried))
}

#[cfg(test)]
mod tests;
