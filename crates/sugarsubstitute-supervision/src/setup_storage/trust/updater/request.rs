// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read healthy engine identity and independently pin its authenticated preparation image.
use super::*;
use crate::setup_storage::{PreparationImage, bundle::Payload};
use sugarsubstitute_contracts::update::Channel;

impl TrustStore {
    /// Build updater discovery facts from this compiled engine and accepted local release identity.
    /// Prepared/trial generations never contribute a healthy floor. A newer accepted engine does
    /// not change this process's compiled capabilities. Fresh empty engine continuity may be created.
    /// Run on an admitted storage worker with ordinary trust-operation capacity.
    /// # Errors
    /// Preserves corrupt/missing established state, receipt failures and cancellation.
    pub fn updater_request(
        &self,
        channel: Channel,
        context: &JobContext,
    ) -> Result<Request, TrustFailure> {
        self.protected(context, |protection| {
            let _writer = self.writer(protection)?;
            let selection = self.updater_locked(protection, SelectionRead::Initialize, context)?;
            let mut request = UpdateAuthorization::local_request(channel)?;
            request.component = Component::Updater;
            if let Some(generation) = selection.accepted() {
                let receipt = self.updater_receipt(generation, protection)?;
                if receipt.channel == channel {
                    request.sequence_floor = receipt.sequence;
                    request.sequence_digest = Some(receipt.archive_sha256);
                }
            }
            check(context)?;
            Ok(request)
        })
    }

    /// Pin an accepted engine that meets this subscription's required release, without retaining
    /// trust, preparation or application locks in the returned image. All payload bytes and entry
    /// compatibility are rechecked under the selection writer. No network freshness is required
    /// to reuse a healthy installed engine. Reserve payload plus trust-operation capacity.
    /// # Errors
    /// Rejects corrupt continuity/receipts, modified payloads, unsafe paths and cancellation.
    /// Returns `None` when no accepted engine on this channel meets `minimum_release`.
    pub fn pin_updater(
        &self,
        channel: Channel,
        minimum_release: u64,
        context: &JobContext,
    ) -> Result<Option<PreparationImage>, TrustFailure> {
        self.protected(context, |protection| {
            let _writer = self.writer(protection)?;
            let selection = self.updater_locked(protection, SelectionRead::Initialize, context)?;
            let Some(generation) = selection.accepted() else { return Ok(None) };
            let receipt = self.updater_receipt(generation, protection)?;
            if receipt.channel != channel || receipt.sequence < minimum_release {
                return Ok(None);
            }
            let path = self.updater_path(generation);
            protection::with(context, |payload_protection| {
                payload_protection.directory(&path, Mode::Existing, context)?;
                payload_protection.directory(&path.join("app"), Mode::Existing, context)?;
                let payload =
                    Payload::open(&path, generation.package.as_str(), Component::Updater, context)?;
                payload.require_protected(payload_protection)?;
                payload.compatibility.require_updater()?;
                let installation = Directory::open(self._installation_root.path(), false)?;
                check(context)?;
                Ok(Some(PreparationImage::updater(generation.clone(), payload, installation)))
            })
            .map_err(TrustFailure::from)
        })
    }
}

#[cfg(test)]
mod tests;
