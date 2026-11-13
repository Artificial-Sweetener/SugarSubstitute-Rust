// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep immutable trust continuity outside the directory containing mutable verification floors.
use super::*;

pub(super) const MARKER: &str = super::CONTINUITY_MARKER;
const CONTENT: &[u8] = b".update-trust";
/// Prefix of the continuity marker's stage in the installation state folder.
const MARKER_STAGE: &str = ".update-trust-enable-stage";

/// Establish continuity while retaining the trust writer and validated checkpoint/history.
/// Newly initialized stores supply the complete records they just published; reopening stores
/// validate both existing records first. Never replace or reinterpret an occupied marker.
pub(super) fn ensure(
    store: &TrustStore,
    protection: &Protection,
    context: &JobContext,
) -> Result<File, TrustFailure> {
    if let Some(marker) = read(store, protection)? {
        return Ok(marker);
    }
    let bytes = envelope::encode(
        envelope::Kind::Continuity,
        &store.installation,
        &store.repository,
        CONTENT,
    )?;
    let state = ownership::state(&store._installation_root)?;
    let destination = state.path().join(MARKER);
    let mut write = write_port::begin(UpdateFile::Trust, &destination);
    // The caller holds the trust writer, which every marker publication takes.
    protection::with(context, |owner| staging::reclaim(&state, MARKER_STAGE, &[], owner, context))?;
    let staged = staging::unique_path(&state, MARKER_STAGE)?;
    let mut file = protection.file(&staged, context)?;
    write.bytes(&mut file, &bytes)?;
    files::sync(&file, context)?;
    drop(file);
    check(context)?;
    write.publish(|| staging::publish(&staged, &destination))?;
    read(store, protection)?
        .ok_or_else(|| invalid("Published update trust continuity is absent").into())
}

/// Retain the exact protected marker after checking its version, namespace and identity binding.
pub(super) fn read(
    store: &TrustStore,
    protection: &Protection,
) -> Result<Option<File>, TrustFailure> {
    let mut file =
        match ownership::state(&store._installation_root).and_then(|state| state.read(MARKER)) {
            Ok(file) => file,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(cause) => return Err(cause.into()),
        };
    protection.verify_file(&file)?;
    let bytes = files::read_bounded(&mut file, envelope::Kind::Continuity.limit())?;
    let payload = envelope::decode(
        envelope::Kind::Continuity,
        &store.installation,
        &store.repository,
        &bytes,
    )?;
    if payload != CONTENT {
        return Err(invalid("Update trust continuity names an unknown namespace").into());
    }
    Ok(Some(file))
}

#[cfg(test)]
mod tests;
