// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode producing-installer configuration while preserving exact signed metadata bytes.
use super::*;
use serde::Deserialize;
use sugarsubstitute_contracts::{
    execution::MemoryClaim, installation::PreparationIntent, update::Channel,
};

/// Encoded build configuration bound; decoding uses one normal payload workspace reservation.
pub const MAX_DISTRIBUTION_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    schema_version: u32,
    repository: [u8; 32],
    channel: Channel,
    manifest_sha256: String,
    trusted_root: String,
    roots: Vec<Root>,
    timestamp: String,
    snapshot: String,
    targets: String,
    descriptor: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Root {
    version: u32,
    metadata: String,
}

/// Configuration compiled into the authenticated producing installer, never read from a server
/// or neighboring file. Decoding checks structure only; import authenticates every signed role
/// against current installed trust and verifies the exact archive before reserving a generation.
pub struct InstallerDistribution {
    configuration: Configuration,
    _memory: MemoryClaim,
}
impl InstallerDistribution {
    /// Decode bounded embedded bytes on an admitted worker with `RESERVATION_BYTES` available.
    /// Signed role strings preserve their original UTF-8 bytes, including whitespace.
    /// # Errors
    /// Rejects missing/unknown formats, malformed identities, oversized roles and unordered roots.
    pub fn decode(bytes: &[u8], context: &JobContext) -> io::Result<Self> {
        check(context)?;
        if bytes.is_empty() || bytes.len() > MAX_DISTRIBUTION_BYTES {
            return Err(invalid(
                "Installer distribution configuration is missing or exceeds its bound",
            ));
        }
        let memory = context.memory().claim(RESERVATION_BYTES).map_err(io::Error::other)?;
        let configuration: Configuration =
            serde_json::from_slice(bytes).map_err(io::Error::other)?;
        if configuration.schema_version != 1
            || configuration.repository == [0; 32]
            || configuration.manifest_sha256.len() != 64
            || !configuration
                .manifest_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || configuration.roots.len() > 1024
            || configuration.roots.iter().any(|root| root.version == 0 || !role(&root.metadata))
            || configuration.roots.windows(2).any(|roots| roots[0].version >= roots[1].version)
            || ![
                &configuration.trusted_root,
                &configuration.timestamp,
                &configuration.snapshot,
                &configuration.targets,
            ]
            .into_iter()
            .all(|bytes| role(bytes))
            || configuration.descriptor.is_empty()
            || configuration.descriptor.len() > sugarsubstitute_contracts::update::MAX_OFFER_BYTES
        {
            return Err(invalid("Installer distribution configuration is invalid"));
        }
        check(context)?;
        Ok(Self { configuration, _memory: memory })
    }

    /// Inventory digest compiled with the installer, used to verify its initial provisioning UI.
    /// This digest does not authorize an update or replace signed archive verification.
    pub fn manifest_digest(&self) -> &str {
        &self.configuration.manifest_sha256
    }

    /// Import this embedded distribution under the selected installation's retained setup custody.
    /// Uses the actual engine/platform and the installer's explicit channel; signed metadata
    /// supplies no local release floors or privilege. Installed trust supplies durable floors.
    /// `now` supplies Unix UTC seconds. Reserve `SIGNED_INSTALLER_RESERVATION_BYTES` for this job.
    /// # Errors
    /// Preserves installed authority on signature/rotation/expiry failures; rejects archive or
    /// inventory disagreement before any candidate reservation. This method starts no process.
    pub fn import(
        self,
        session: InstallationSession,
        executable: &Path,
        workspace: &Path,
        now: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<SignedInstallation, TrustFailure> {
        let configuration = &self.configuration;
        let roots = configuration
            .roots
            .iter()
            .map(|root| (root.version, root.metadata.as_bytes()))
            .collect::<Vec<_>>();
        let imported = session.import_installer(
            InstallerSource {
                repository: configuration.repository,
                trusted_root: configuration.trusted_root.as_bytes(),
                roots: &roots,
                metadata: MetadataSet {
                    timestamp: configuration.timestamp.as_bytes(),
                    snapshot: configuration.snapshot.as_bytes(),
                    targets: configuration.targets.as_bytes(),
                },
                descriptor: configuration.descriptor.as_bytes(),
                executable,
            },
            Request {
                intent: PreparationIntent::Repair,
                ..UpdateAuthorization::local_request(configuration.channel)?
            },
            workspace,
            now,
            context,
        )?;
        if imported.bundle.digest() != configuration.manifest_sha256 {
            return Err(
                invalid("Signed archive inventory differs from the producing installer").into()
            );
        }
        Ok(imported)
    }
}

/// Role bounds apply to decoded bytes, not JSON escape spelling in the build configuration.
fn role(bytes: &str) -> bool {
    !bytes.is_empty() && bytes.len() <= MAX_METADATA_BYTES
}

#[cfg(test)]
mod tests;
