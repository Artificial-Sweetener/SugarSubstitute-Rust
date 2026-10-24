// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind signed release identity to an immutable generation and consult only accepted selection.
use super::receipt::{MAX_BYTES, Receipt};
use super::*;
use sugarsubstitute_contracts::{
    installation::Generation,
    update::{Channel, Component},
};
mod publication;
pub(in crate::setup_storage) use publication::PublishedRelease;

const RECORD: &str = "application-update-v1.record";
impl UpdateAuthorization<'_> {
    /// Called only inside `publish`, using its currently verified offer and retained trust writer.
    /// Record exact release identity before exposing candidate completion. Interrupted writes
    /// leave no partial canonical receipt, and retries accept only byte-identical content.
    pub(in crate::setup_storage) fn record_generation(
        &self,
        offer: &VerifiedOffer,
        generation: &Generation,
        directory: &Directory,
        context: &JobContext,
    ) -> io::Result<()> {
        check(context)?;
        let expected =
            self.trust._installation_root.path.join("releases").join(generation.id.as_str());
        let expected = Directory::open(&expected, false)?;
        if directory.identity()? != expected.identity()? {
            return Err(invalid("Update receipt generation directory differs"));
        }
        let receipt = Receipt {
            schema_version: 1,
            generation: generation.clone(),
            channel: self.request.channel,
            platform: self.request.platform,
            component: self.request.component,
            sequence: offer.offer().sequence(),
            archive_sha256: offer.offer().package_digest(),
        };
        let bytes = serde_json::to_vec(&receipt).map_err(io::Error::other)?;
        if bytes.len() > MAX_BYTES {
            return Err(invalid("Update receipt exceeds its bound"));
        }
        let bytes = envelope::encode(
            envelope::Kind::InstalledRelease,
            &self.trust.installation,
            &self.trust.repository,
            &bytes,
        )?;
        protection::with(context, |protection| {
            files::write_immutable(&directory.path.join(RECORD), &bytes, |path| {
                protection.file(path, context)
            })?;
            protection.verify_file(&directory.read(RECORD)?)
        })
    }
}

impl TrustStore {
    /// Build automatic application-update facts from this engine and accepted installation state.
    /// `channel` is the user's current subscription. Candidates and failed attempts never supply
    /// the healthy sequence; independent protected history still prevents publication rollback.
    /// Older native generations without a receipt have unknown healthy identity (zero/None),
    /// not permission to reset trust. A present malformed or foreign receipt remains an error.
    /// This returns copied facts only and retains no application run/profile custody.
    /// Reserve ordinary trust-operation capacity alongside this store's retained workspace.
    /// # Errors
    /// Rejects missing/changed installation identity, malformed/foreign/unprotected receipts,
    /// unsafe paths, cancellation and storage errors. It never initializes or repairs state.
    pub fn application_request(
        &self,
        channel: Channel,
        context: &JobContext,
    ) -> Result<Request, TrustFailure> {
        let mut request = UpdateAuthorization::local_request(channel)?;
        let selection = Self::selection(&self._installation_root.path, context)?;
        if selection.installation() != &self.installation {
            return Err(invalid("Update request installation identity changed").into());
        }
        let Some(generation) = selection.accepted() else { return Ok(request) };
        if let Some(publication) = self.published_release(generation, context)?
            && publication.receipt.channel == request.channel
        {
            request.sequence_floor = publication.receipt.sequence;
            request.sequence_digest = Some(publication.receipt.archive_sha256);
        }
        check(context)?;
        Ok(request)
    }
}

#[cfg(test)]
mod tests;
