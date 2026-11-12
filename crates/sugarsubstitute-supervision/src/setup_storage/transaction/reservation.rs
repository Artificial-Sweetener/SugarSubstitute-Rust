// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Persist signed preparation intent before allocating any canonical generation files.
use super::*;
use crate::setup_storage::{SelectionStore, TrustFailure, UpdateAuthorization};
use sugarsubstitute_contracts::installation::{Candidate, Generation, PackageDigest};
use sugarsubstitute_update::VerifiedOffer;

impl InstallationSession {
    /// Reserve, resume or supersede a signed candidate before materializing its generation.
    /// Setup exclusion remains held throughout. A journaled staging generation may be absent
    /// after interruption; only this authenticated operation may initialize that exact location.
    /// A different authorized package or published release gets a fresh generation. Supersession
    /// atomically changes only pending selection and preserves every predecessor payload.
    /// `now` supplies Unix UTC seconds. The trust owner retains its parsing workspace;
    /// reserve `SIGNED_PREPARATION_RESERVATION_BYTES` on the active worker.
    /// # Errors
    /// Preserves trials, unknown files and all accepted state on failure.
    /// Cancellation after selection publication leaves resumable intent, never a healthy selection.
    pub fn prepare_signed(
        self,
        bundle: Arc<VerifiedBundle>,
        authorization: &UpdateAuthorization<'_>,
        now: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<PreparedInstallation, TrustFailure> {
        bundle.require_installed_launch()?;
        let bytes = bundle
            .download_identity()
            .ok_or_else(|| invalid("Update archive custody is absent"))?;
        let (path, pinned, marker) = authorization.publish(
            &self.admission.root,
            bytes,
            now,
            context,
            |offer, recheck| {
                let store = Arc::new(
                    SelectionStore::open(self.directory(), context).map_err(io::Error::other)?,
                );
                store
                    .require_launcher(&bundle.payload.compatibility, context)
                    .map_err(io::Error::other)?;
                let before = store.load_recovered(context).map_err(io::Error::other)?;
                let resumable = before
                    .candidate()
                    .map(|candidate| {
                        self.resumable(candidate, &bundle, authorization, offer, context)
                    })
                    .transpose()?
                    .flatten();
                let (generation, initialize) = match resumable {
                    None => {
                        let generation = Generation {
                            id: crate::setup_storage::identity::generation()?,
                            package: PackageDigest::new(bundle.digest().to_owned())
                                .map_err(io::Error::other)?,
                            release: crate::setup_storage::signed_release(offer)?,
                        };
                        let now = crate::setup_storage::selection::unix_now()
                            .map_err(io::Error::other)?;
                        let reserved = match before.candidate() {
                            None => before.prepare(
                                generation.clone(),
                                authorization.request.intent,
                                now,
                            ),
                            Some(previous) => before.supersede_preparation(
                                previous.generation(),
                                generation.clone(),
                                authorization.request.intent,
                                now,
                            ),
                        }
                        .map_err(io::Error::other)?;
                        recheck()?;
                        store
                            .compare_exchange(&before, &reserved, context)
                            .map_err(io::Error::other)?;
                        (generation, true)
                    }
                    Some(resumable) => resumable,
                };
                if generation.package.as_str() != bundle.digest() {
                    return Err(invalid("Resume package differs from the recorded candidate"));
                }
                let path = self.directory().join("releases").join(generation.id.as_str());
                let (pinned, marker) = match crate::durable_storage::files::inspect(&path, context)
                {
                    Ok(_) => generation::open(&path, bundle.digest(), context)?,
                    Err(cause) if cause.kind() == io::ErrorKind::NotFound && initialize => {
                        generation::create(&path, bundle.digest(), context)?
                    }
                    Err(cause) => return Err(cause),
                };
                self.provision_user_folders(context)?;
                recheck()?;
                Ok((path, pinned, marker))
            },
        )?;
        Ok(PreparedInstallation {
            session: self,
            generation: path,
            bundle,
            _generation: pinned,
            _generation_marker: marker,
        })
    }

    /// Resume exact payload and publication identity; never reuse an immutable receipt for a
    /// different sequence, channel or archive. Held setup custody excludes a live candidate owner.
    fn resumable(
        &self,
        candidate: &Candidate,
        bundle: &VerifiedBundle,
        authorization: &UpdateAuthorization<'_>,
        offer: &VerifiedOffer,
        context: &JobContext,
    ) -> io::Result<Option<(Generation, bool)>> {
        let (generation, initialize) = match candidate {
            Candidate::Staging { generation } => (generation, true),
            Candidate::Prepared { generation } => (generation, false),
            Candidate::Trial { .. } => {
                return Err(invalid("An earlier application trial requires reconciliation"));
            }
        };
        if generation.package.as_str() != bundle.digest() {
            return Ok(None);
        }
        if initialize {
            let path = self.directory().join("releases").join(generation.id.as_str());
            match crate::durable_storage::files::inspect(path, context) {
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
                    return Ok(Some((generation.clone(), true)));
                }
                Err(cause) => return Err(cause),
                Ok(_) => {}
            }
        }
        let publication =
            authorization.trust.published_release(generation, context).map_err(io::Error::other)?;
        if publication.is_some_and(|publication| !publication.matches(authorization.request, offer))
        {
            return Ok(None);
        }
        Ok(Some((generation.clone(), initialize)))
    }
}
