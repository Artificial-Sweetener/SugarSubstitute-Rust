// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reserve retained-output quota before immutable publication; preserve interrupted reservations.
use super::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, io::Read};
use sugarsubstitute_contracts::{
    desktop_session::{InstallationId, MAX_ASSET_BYTES, ProfileId, wire::SessionAsset},
    execution::JobContext,
};
use sugarsubstitute_supervision::{ArtifactPublication, with_artifact_input, write_artifact};
const MAX_INVENTORY_BYTES: usize = 16 * 1024 * 1024;
const SCRATCH: usize = 64 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Record {
    version: u32,
    installation: String,
    profile: String,
    reservations: Vec<Reservation>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Reservation {
    asset: SessionAsset,
    verified: bool,
}

/// Profile-wide quota ledger. Pending reservations remain counted after interrupted writes.
/// A single profile publisher durably writes it before publishing any new immutable asset.
pub struct AssetInventory {
    record: Record,
}
impl AssetInventory {
    /// Start one host-selected installation/profile ledger with no retained assets.
    pub fn new(installation: &InstallationId, profile: &ProfileId) -> Self {
        Self {
            record: Record {
                version: 1,
                installation: installation.as_str().into(),
                profile: profile.as_str().into(),
                reservations: vec![],
            },
        }
    }
    /// Read only closed bounded v1 records belonging to the independently selected profile.
    /// # Errors
    /// Refuses future, malformed, foreign, duplicate or over-quota originals without changing them.
    pub fn decode(
        bytes: &[u8],
        installation: &InstallationId,
        profile: &ProfileId,
    ) -> Result<Self, StoreError> {
        if bytes.len() > MAX_INVENTORY_BYTES {
            return Err(StoreError::Protected);
        }
        let record: Record = serde_json::from_slice(bytes).map_err(|_| StoreError::Protected)?;
        if record.version != 1
            || record.installation != installation.as_str()
            || record.profile != profile.as_str()
            || record.reservations.len() > 65536
        {
            return Err(StoreError::Protected);
        }
        let mut digests = BTreeSet::new();
        let mut bytes = 0_u64;
        for reservation in &record.reservations {
            validate(&reservation.asset)?;
            if !digests.insert(&reservation.asset.digest) {
                return Err(StoreError::Protected);
            }
            bytes = bytes.checked_add(reservation.asset.bytes).ok_or(StoreError::Protected)?;
        }
        if bytes > MAX_ASSET_BYTES {
            return Err(StoreError::Protected);
        }
        Ok(Self { record })
    }
    /// Reserve exact content before disk publication. Identical previously reserved content
    /// is reused, including pending reservations from a prior interrupted launch.
    /// # Errors
    /// Refuses inconsistent digest metadata, count/quota overflow, invalid extension or media.
    pub fn reserve(&mut self, asset: &SessionAsset) -> Result<(), StoreError> {
        validate(asset)?;
        if let Some(existing) =
            self.record.reservations.iter().find(|value| value.asset.digest == asset.digest)
        {
            return if existing.asset == *asset { Ok(()) } else { Err(StoreError::Changed) };
        }
        let bytes = self
            .record
            .reservations
            .iter()
            .try_fold(asset.bytes, |sum, value| sum.checked_add(value.asset.bytes))
            .ok_or(StoreError::Protected)?;
        if bytes > MAX_ASSET_BYTES || self.record.reservations.len() >= 65536 {
            return Err(StoreError::Protected);
        }
        self.record.reservations.push(Reservation { asset: asset.clone(), verified: false });
        Ok(())
    }
    /// Mark content verified only after immutable publication and independent length/digest read.
    /// # Errors
    /// Refuses content that has no exact durable reservation.
    pub fn verified(&mut self, asset: &SessionAsset) -> Result<(), StoreError> {
        let reservation = self
            .record
            .reservations
            .iter_mut()
            .find(|value| value.asset == *asset)
            .ok_or(StoreError::Protected)?;
        reservation.verified = true;
        Ok(())
    }
    /// Encode all reservations, including interrupted ones; no TTL or implicit cleanup exists.
    /// # Errors
    /// Preserves encoding and record byte admission failure.
    pub fn encode(&self) -> Result<Vec<u8>, StoreError> {
        let bytes = serde_json::to_vec(&self.record).map_err(io::Error::other)?;
        if bytes.len() > MAX_INVENTORY_BYTES {
            return Err(StoreError::Protected);
        }
        Ok(bytes)
    }
}
fn validate(asset: &SessionAsset) -> Result<(), StoreError> {
    if !asset.valid()
        || asset.digest.len() != 64
        || !asset.digest.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || if matches!(asset.extension.as_str(), "mp4" | "webm") {
            asset.media != "video"
        } else {
            asset.media != "image"
        }
    {
        return Err(SessionError::Invalid.into());
    }
    Ok(())
}
/// Verify one exact immutable session asset through the regular-file storage authority.
pub(super) fn verify(
    directory: &Path,
    asset: &SessionAsset,
    context: &JobContext,
) -> Result<(), StoreError> {
    validate(asset)?;
    let path = directory.join(format!("{}.{}", asset.digest, asset.extension));
    with_artifact_input(&path, asset.bytes, context, |input| {
        if input.len() != asset.bytes {
            return Err(StoreError::Changed);
        }
        let (digest, length) = stream(input, context, |_| Ok(()))?;
        if digest != asset.digest || length != asset.bytes {
            return Err(StoreError::Changed);
        }
        Ok(())
    })
    .map_err(files::artifact)
}
/// Publish only from an independently verified current-backend reader. The caller must first
/// persist its quota reservation; neither `asset` nor a browser path grants source authority.
pub(super) fn retain(
    directory: &Path,
    asset: &SessionAsset,
    source: &mut dyn Read,
    context: &JobContext,
) -> Result<(), StoreError> {
    validate(asset)?;
    match verify(directory, asset, context) {
        Ok(()) => return Ok(()),
        Err(StoreError::Storage(cause)) if cause.kind() == io::ErrorKind::NotFound => {}
        Err(cause) => return Err(cause),
    }
    let path = directory.join(format!("{}.{}", asset.digest, asset.extension));
    write_artifact(
        &path,
        true,
        ArtifactPublication::CreateOnly,
        asset.bytes as usize,
        context,
        |output| {
            let (digest, bytes) = stream(source, context, |chunk| output.write_all(chunk))?;
            if digest != asset.digest || bytes != asset.bytes {
                return Err(StoreError::Changed);
            }
            Ok::<_, StoreError>(())
        },
    )
    .map_err(files::artifact)?;
    verify(directory, asset, context)
}
fn stream(
    source: &mut dyn Read,
    context: &JobContext,
    mut consume: impl FnMut(&[u8]) -> io::Result<()>,
) -> Result<(String, u64), StoreError> {
    let _claim = context.memory().claim(SCRATCH as u64).map_err(io::Error::other)?;
    let mut buffer = vec![0; SCRATCH];
    let mut hash = Sha256::new();
    let mut bytes = 0_u64;
    loop {
        if context.cancellation_requested() {
            return Err(io::Error::from(io::ErrorKind::Interrupted).into());
        }
        if context.deadline().is_none_or(|deadline| context.now() >= deadline) {
            return Err(io::Error::from(io::ErrorKind::TimedOut).into());
        }
        let count = source.read(&mut buffer)?;
        if count == 0 {
            return Ok((files::hex(&hash.finalize()), bytes));
        }
        bytes += count as u64;
        hash.update(&buffer[..count]);
        consume(&buffer[..count])?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    type Result = std::result::Result<(), Box<dyn Error>>;
    #[test]
    fn interrupted_reservations_survive_restart_and_exact_quota_is_enforced() -> Result {
        let installation = InstallationId::new("1".repeat(32))?;
        let profile = ProfileId::new("real".into())?;
        let mut inventory = AssetInventory::new(&installation, &profile);
        let asset = SessionAsset {
            version: 1,
            digest: "a".repeat(64),
            bytes: MAX_ASSET_BYTES,
            media: "image".into(),
            extension: "png".into(),
        };
        inventory.reserve(&asset)?;
        let mut restored = AssetInventory::decode(&inventory.encode()?, &installation, &profile)?;
        restored.reserve(&asset)?;
        let overflow = SessionAsset { digest: "b".repeat(64), bytes: 1, ..asset.clone() };
        assert!(restored.reserve(&overflow).is_err());
        assert_eq!(restored.encode()?, inventory.encode()?);
        restored.verified(&asset)?;
        assert!(
            AssetInventory::decode(
                &restored.encode()?,
                &installation,
                &ProfileId::new("other".into())?
            )
            .is_err()
        );
        Ok(())
    }
}
