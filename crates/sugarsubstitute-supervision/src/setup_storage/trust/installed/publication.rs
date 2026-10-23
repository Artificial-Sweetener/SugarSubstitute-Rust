// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reopen the protected publication identity of an already prepared native generation.
use super::*;

/// Publication identity pinned to one native installation and generation, never health evidence.
/// The consumer must separately verify the installed manifest and every application payload.
pub(in crate::setup_storage) struct PublishedRelease {
    pub(super) receipt: Receipt,
    installation: Directory,
    _directory: Directory,
    _file: File,
}
impl PublishedRelease {
    /// A protected receipt cannot be rebound to another native installation with matching IDs.
    pub(in crate::setup_storage) fn require_installation(
        &self,
        installation: &Directory,
    ) -> io::Result<()> {
        if self.installation.identity()? != installation.identity()? {
            return Err(invalid("Prepared release belongs to another native installation"));
        }
        Ok(())
    }

    /// Match independently reauthenticated routing and archive identity to prior publication.
    pub(in crate::setup_storage) fn matches(
        &self,
        request: &Request,
        offer: &VerifiedOffer,
    ) -> bool {
        self.receipt.channel == request.channel
            && self.receipt.platform == request.platform
            && self.receipt.component == request.component
            && self.receipt.sequence == offer.offer().sequence()
            && self.receipt.archive_sha256 == offer.offer().package_digest()
    }
}
impl TrustStore {
    /// Reopen only the requested generation's protected publication record, retaining its bytes.
    /// Absence is repairable from authenticated delivery; malformed or foreign evidence is not.
    pub(in crate::setup_storage) fn published_release(
        &self,
        generation: &Generation,
        context: &JobContext,
    ) -> Result<Option<PublishedRelease>, TrustFailure> {
        let request = UpdateAuthorization::local_request(Channel::Release)?;
        protection::with_domain(context, |protection| -> Result<_, TrustFailure> {
            check(context)?;
            let installation = Directory::open(&self._installation_root.path, false)?;
            if installation.identity()? != self._installation_root.identity()? {
                return Err(invalid("Prepared release installation changed").into());
            }
            let path = installation.path.join("releases").join(generation.id.as_str());
            protection.directory(&path, Mode::Existing, context)?;
            let directory = Directory::open(&path, false)?;
            let mut file = match directory.read(RECORD) {
                Ok(file) => file,
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(None),
                Err(cause) => return Err(cause.into()),
            };
            protection.verify_file(&file)?;
            let bytes = files::read_bounded(&mut file, envelope::Kind::InstalledRelease.limit())?;
            let payload = envelope::decode(
                envelope::Kind::InstalledRelease,
                &self.installation,
                &self.repository,
                &bytes,
            )?;
            let receipt: Receipt = serde_json::from_slice(payload).map_err(io::Error::other)?;
            if receipt.schema_version != 1
                || receipt.sequence == 0
                || &receipt.generation != generation
                || receipt.component != Component::Application
                || receipt.platform != request.platform
            {
                return Err(
                    invalid("Prepared release receipt is invalid or belongs elsewhere").into()
                );
            }
            check(context)?;
            Ok(Some(PublishedRelease { receipt, installation, _directory: directory, _file: file }))
        })
        .map_err(TrustFailure::from)
    }
}
