// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep immutable trust continuity outside the directory containing mutable verification floors.
use super::*;

pub(super) const MARKER: &str = ".update-trust-enabled";
const CONTENT: &[u8] = b".update-trust";

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
    let staged = staging::unique_path(&store._installation_root, ".update-trust-enable-stage")?;
    let mut file = protection.file(&staged, context)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    check(context)?;
    staging::publish(&staged, &store._installation_root.path.join(MARKER))?;
    read(store, protection)?
        .ok_or_else(|| invalid("Published update trust continuity is absent").into())
}

/// Retain the exact protected marker after checking its version, namespace and identity binding.
fn read(store: &TrustStore, protection: &Protection) -> Result<Option<File>, TrustFailure> {
    let mut file = match store._installation_root.read(MARKER) {
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
