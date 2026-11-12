// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish prepared update candidates while retaining the accepted generation and shell entry.
use super::*;
use sugarsubstitute_contracts::installation::{
    Candidate, Generation, GenerationId, InstallationRecord, PackageDigest,
};
mod installation;
mod reopen;

/// Preparation retains its source; restart retains protected publication and inventory instead.
enum Evidence {
    Archive(Arc<VerifiedBundle>),
    Published {
        publication: Box<super::super::trust::PublishedRelease>,
        _inventory: File,
        _marker: File,
    },
    Installed {
        _inventory: File,
        _marker: File,
    },
}

/// Prepared candidate runtime and its current durable selection identity.
/// Only recovery under the original unstarted creator custody may advance this revision.
/// Retained directory and runtime custody prevent replacement while the caller arranges a trial.
/// Initial preparation verifies every copied file; diagnostic consumption authenticates symbols.
/// This object proves preparation only. Launch admission must recheck current repository trust;
/// accepting a trial requires authenticated application health and the live installation owner.
pub struct PreparedCandidate {
    generation: Generation,
    directory: Directory,
    pub(super) installation: Directory,
    pub(super) snapshot: std::sync::Mutex<InstallationRecord>,
    _payload: super::super::bundle::VerifiedRuntime,
    evidence: Evidence,
}
impl PreparedCandidate {
    /// Native launch transfer retains the actual candidate files and all pinned ancestors.
    pub(super) fn launch_pins(&self) -> impl Iterator<Item = &File> {
        self.directory
            .native_pins()
            .chain(self.installation.native_pins())
            .chain(self._payload.launch_pins())
    }

    /// Exact generation recorded as prepared, with its verified manifest digest.
    pub fn generation(&self) -> &Generation {
        &self.generation
    }

    /// Pinned immutable generation location; not permission to launch or select it.
    pub fn directory(&self) -> &Path {
        self.directory.path()
    }

    /// Native installation pinned by this candidate independently of its delivery source.
    pub(in crate::setup_storage) fn installation_root(&self) -> &Path {
        self.installation.path()
    }

    /// Compatibility is derived from the verified destination in both preparation and restart.
    pub(in crate::setup_storage) fn compatibility(
        &self,
    ) -> &super::super::compatibility::Compatibility {
        &self._payload.compatibility
    }

    /// Recheck current signed authority using retained delivery or protected publication custody.
    pub(super) fn authorize<T>(
        &self,
        authorization: &super::super::UpdateAuthorization<'_>,
        now: impl FnMut() -> i64,
        context: &JobContext,
        publish: impl FnOnce(
            &sugarsubstitute_update::VerifiedOffer,
            &mut dyn FnMut() -> io::Result<()>,
        ) -> io::Result<T>,
    ) -> Result<T, super::super::TrustFailure> {
        match &self.evidence {
            Evidence::Installed { .. } => {
                Err(invalid("Installed authorization requires its retained installation choice")
                    .into())
            }
            Evidence::Archive(bundle) => {
                let bytes = bundle
                    .download_identity()
                    .ok_or_else(|| invalid("Update archive custody is absent"))?;
                authorization.publish(&self.installation, bytes, now, context, publish)
            }
            Evidence::Published { publication, .. } => authorization.publish_prepared(
                &self.installation,
                publication,
                now,
                context,
                publish,
            ),
        }
    }
}

impl PreparedInstallation {
    /// Prepare a downloaded distribution under current signed repository authority.
    /// The returned shared owner pins immutable files across launch retries; it grants no run
    /// authority and each trial must independently recheck the recorded preparation and trust.
    /// Preserves accepted selection and restores any established launcher ownership.
    /// A persisted staging record precedes configuration; a prepared record follows the
    /// final authority check. Launcher restoration requires no application run lease.
    /// `now` supplies Unix UTC seconds from the execution adapter's clock boundary.
    /// Reserve `SIGNED_PREPARATION_RESERVATION_BYTES` alongside retained package/trust custody.
    /// # Errors
    /// Preserves prior selection and partial files on expiry, conflict, cancellation or I/O
    /// failure. Interrupted staging remains explicit and must be reconciled before another offer.
    pub fn prepare_update(
        &self,
        runtime: &RuntimePaths,
        authorization: &super::super::UpdateAuthorization<'_>,
        now: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<Arc<PreparedCandidate>, super::super::TrustFailure> {
        self.bundle.require_installed_launch()?;
        let bytes = self
            .bundle
            .download_identity()
            .ok_or_else(|| invalid("Update archive custody is absent"))?;
        authorization.publish(
            &self.session.admission.root,
            bytes,
            now,
            context,
            |offer, recheck| {
                let store = Arc::new(
                    super::super::SelectionStore::open(self.session.admission.root.path(), context)
                        .map_err(io::Error::other)?,
                );
                store
                    .require_launcher(&self.bundle.payload.compatibility, context)
                    .map_err(io::Error::other)?;
                let original = store.load_recovered(context).map_err(io::Error::other)?;
                let generation = Generation {
                    id: GenerationId::new(
                        self.generation
                            .file_name()
                            .and_then(|value| value.to_str())
                            .ok_or_else(|| invalid("Generation directory identity is absent"))?
                            .to_owned(),
                    )
                    .map_err(io::Error::other)?,
                    package: PackageDigest::new(self.bundle.digest().to_owned())
                        .map_err(io::Error::other)?,
                    release: crate::setup_storage::signed_release(offer)?,
                };
                let staging = match original.candidate() {
                    None => {
                        let now = super::super::selection::unix_now().map_err(io::Error::other)?;
                        let staging = original
                            .prepare(generation.clone(), authorization.request.intent, now)
                            .map_err(io::Error::other)?;
                        store
                            .compare_exchange(&original, &staging, context)
                            .map_err(io::Error::other)?;
                        staging
                    }
                    Some(
                        Candidate::Staging { generation: pending }
                        | Candidate::Prepared { generation: pending },
                    ) if pending == &generation => original,
                    _ => {
                        return Err(invalid(
                            "Another candidate or launch trial owns installation selection",
                        ));
                    }
                };
                self.stage(context)?;
                let payload = super::super::bundle::VerifiedRuntime::verify_copy(
                    &self.bundle.payload.inventory,
                    &self.generation,
                    context,
                )?;
                self.complete(runtime, context)?;
                let directory = Directory::open(&self.generation, false)?;
                let installation = Directory::open(self.session.admission.root.path(), false)?;
                recheck()?;
                authorization.record_generation(offer, &generation, &directory, context)?;
                recheck()?;
                let prepared = if matches!(staging.candidate(), Some(Candidate::Staging { .. })) {
                    let prepared = staging.prepared(&generation).map_err(io::Error::other)?;
                    store
                        .compare_exchange(&staging, &prepared, context)
                        .map_err(io::Error::other)?;
                    prepared
                } else {
                    if store.load(context).map_err(io::Error::other)?.as_ref() != Some(&staging) {
                        return Err(io::Error::other(super::super::SelectionFailure::Changed));
                    }
                    staging
                };
                let candidate = Arc::new(PreparedCandidate {
                    generation,
                    directory,
                    installation,
                    snapshot: std::sync::Mutex::new(prepared),
                    _payload: payload,
                    evidence: Evidence::Archive(Arc::clone(&self.bundle)),
                });
                store
                    .prepare_launcher(&self.session.admission, &candidate, context)
                    .map_err(io::Error::other)?;
                recheck()?;
                Ok(candidate)
            },
        )
    }
}
