// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Persist local subscription choices independently of packages, cache and signing-key rotation.
use super::*;
use sugarsubstitute_contracts::update::{Channel, subscription::Subscription};

const RECORD: &str = "subscription-v1";
const MARKER: &str = "subscription-established-v1";
const ESTABLISHED: &[u8] = b"established";

impl TrustStore {
    /// Read the current local preference on an admitted storage worker.
    /// Installs predating this record gain Release once; interrupted initialization resumes.
    /// Reserve ordinary trust-operation capacity alongside the retained trust workspace.
    /// # Errors
    /// Preserves corrupt/foreign data, rejects missing established preferences and live writers.
    pub fn subscription(&self, context: &JobContext) -> Result<Subscription, TrustFailure> {
        self.protected(context, |protection| {
            let _writer = self.writer(protection, context)?;
            self.subscription_locked(protection, context)
        })
    }

    /// Publish an explicit local choice only if the displayed revision is still current.
    /// This does not change release floors, application selection, ComfyUI or running processes.
    /// Existing automatic campaigns must recheck this choice before publishing or activating.
    /// # Errors
    /// Rejects stale revisions, exhausted counters, corrupt state and ownership/retirement errors.
    /// Interrupted publication leaves the previous or complete successor record, never a prefix.
    pub fn select_subscription(
        &self,
        expected: Subscription,
        channel: Channel,
        context: &JobContext,
    ) -> Result<Subscription, TrustFailure> {
        self.protected(context, |protection| {
            let _writer = self.writer(protection, context)?;
            let current = self.subscription_locked(protection, context)?;
            if current != expected {
                return Err(
                    invalid("Update subscription changed before this choice was saved").into()
                );
            }
            let next = current.select(channel).map_err(io::Error::other)?;
            if next != current {
                self.write_subscription(next, protection, context)?;
            }
            Ok(next)
        })
    }

    /// Read the installation's local preference for presentation, without the trust writer.
    ///
    /// Trust records are replaced atomically, so this snapshot read neither waits for nor
    /// blocks an update campaign's trust work, and it writes nothing: an installation whose
    /// preference was never recorded follows releases until a campaign records it. Run on an
    /// admitted storage worker.
    /// Returns `None` for an installation without a selection or without update trust.
    /// # Errors
    /// Rejects damaged, foreign or unprotected records without changing them.
    pub fn read_subscription(
        installation_root: &Path,
        context: &JobContext,
    ) -> Result<Option<Subscription>, TrustFailure> {
        check(context)?;
        let Some(selection) = SelectionStore::existing(installation_root, context)
            .map_err(io::Error::other)?
            .map(|store| store.load(context))
            .transpose()
            .map_err(io::Error::other)?
            .flatten()
        else {
            return Ok(None);
        };
        let _memory = context.memory().claim(1024 * 1024).map_err(io::Error::other)?;
        let parent = Directory::open(installation_root, false)?;
        let _ownership = ownership::read(&parent)?;
        let root = match Directory::open(&ownership::state_path(&parent).join(DIRECTORY), false) {
            Ok(root) => root,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(cause) => return Err(cause.into()),
        };
        let installation = selection.installation();
        protection::with_domain(context, |protection| -> Result<_, TrustFailure> {
            protection.directory(root.path(), Mode::Existing, context)?;
            let mut file = root.read(super::RECORD)?;
            protection.verify_file(&file)?;
            let bytes = files::read_bounded(&mut file, envelope::Kind::Checkpoint.limit())?;
            let (repository, _) =
                envelope::decode_binding(envelope::Kind::Checkpoint, installation, &bytes)?;
            let read = |name, kind| {
                super::records::read_bound(&root, name, kind, installation, &repository, protection)
            };
            let marker = read(MARKER, envelope::Kind::SubscriptionMarker)?;
            let record = read(RECORD, envelope::Kind::Subscription)?;
            check(context)?;
            Ok(Some(established(marker.as_deref(), record.as_deref())?.unwrap_or_default()))
        })
        .map_err(TrustFailure::from)
    }

    /// Called only while holding the same trust writer used by final publication authorization.
    pub(super) fn subscription_locked(
        &self,
        protection: &Protection,
        context: &JobContext,
    ) -> Result<Subscription, TrustFailure> {
        let marker =
            self.read_bound_record(MARKER, envelope::Kind::SubscriptionMarker, protection)?;
        let record = self.read_bound_record(RECORD, envelope::Kind::Subscription, protection)?;
        let value = match established(marker.as_deref(), record.as_deref())? {
            Some(value) => value,
            None => {
                let initial = Subscription::default();
                self.write_subscription(initial, protection, context)?;
                initial
            }
        };
        if marker.is_none() {
            let bytes = envelope::encode(
                envelope::Kind::SubscriptionMarker,
                &self.installation,
                &self.repository,
                ESTABLISHED,
            )?;
            files::write_immutable(
                crate::durable_storage::write_port::UpdateFile::Trust,
                &self.root.path().join(MARKER),
                &bytes,
                |path| protection.file(path, context),
            )?;
        }
        check(context)?;
        Ok(value)
    }

    /// Automatic publication follows the local choice; explicit installer repair owns its route.
    pub(super) fn require_subscription(
        &self,
        request: &Request,
        protection: &Protection,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        if request.intent == sugarsubstitute_contracts::installation::PreparationIntent::Automatic
            && self.subscription_locked(protection, context)?.channel() != request.channel
        {
            return Err(invalid("Update subscription changed during automatic preparation").into());
        }
        Ok(())
    }

    /// Flush a protected sibling before atomic replacement under the retained writer and root.
    fn write_subscription(
        &self,
        value: Subscription,
        protection: &Protection,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        self.replace_bound_record(
            RECORD,
            envelope::Kind::Subscription,
            &value.encode().map_err(io::Error::other)?,
            protection,
            context,
        )?;
        check(context)?;
        Ok(())
    }
}

/// The preference the continuity marker and record state; `None` while it was never recorded.
fn established(
    marker: Option<&[u8]>,
    record: Option<&[u8]>,
) -> Result<Option<Subscription>, TrustFailure> {
    if marker.is_some_and(|bytes| bytes != ESTABLISHED) {
        return Err(invalid("Update subscription continuity marker is invalid").into());
    }
    match record {
        Some(bytes) => Ok(Some(Subscription::decode(bytes).map_err(io::Error::other)?)),
        None if marker.is_some() => {
            Err(invalid("Established update subscription is missing").into())
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests;
