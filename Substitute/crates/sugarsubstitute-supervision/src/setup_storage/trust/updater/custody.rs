// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hold updater preparation as slot exclusion; transferred witnesses only prove it is held.
use super::*;
use crate::durable_storage::protection::SlotExclusion;

const RENDEZVOUS: &str = "updater-preparation-v1";

impl TrustStore {
    /// Bind a witness to this installation's canonical updater rendezvous while its exclusion
    /// is held. This proves live custody, not fresh acquisition or process settlement.
    pub(super) fn verify_updater_preparation(
        &self,
        file: &File,
        protection: &Protection,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        protection.verify_exclusive_rendezvous(
            file,
            &self.root.path().join(RENDEZVOUS),
            context,
        )?;
        Ok(())
    }

    /// Adopt a transferred preparation exclusion into this process once, so it outlives its
    /// creator; later calls verify that the adopted exclusion is still held.
    pub(super) fn adopt_updater_preparation(
        &self,
        witness: &File,
        adopted: &std::sync::Mutex<Option<SlotExclusion>>,
        protection: &Protection,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        let path = self.root.path().join(RENDEZVOUS);
        let mut adopted = crate::native_claims::lock(adopted);
        match &*adopted {
            Some(held) => protection.verify_exclusive_rendezvous(held.witness(), &path, context)?,
            None => {
                *adopted =
                    Some(protection.adopt_exclusion(witness.try_clone()?, &path, context)?);
            }
        }
        Ok(())
    }

    /// Exclude other updater preparation until the returned owner is dropped. Foreign readers
    /// of the rendezvous never conflict. File existence, process IDs and elapsed time never
    /// authorize takeover. This owner acquires no application run or setup lease. A probe child
    /// receives a duplicate of the witness only; the creator keeps this owner until it settles.
    pub(in crate::setup_storage) fn claim_updater_preparation(
        &self,
        context: &JobContext,
    ) -> Result<SlotExclusion, TrustFailure> {
        self.protected(context, |protection| {
            let exclusion = protection.exclusion(&self.root.path().join(RENDEZVOUS), context)?;
            check(context)?;
            Ok(exclusion)
        })
    }
}

#[cfg(test)]
mod tests;
