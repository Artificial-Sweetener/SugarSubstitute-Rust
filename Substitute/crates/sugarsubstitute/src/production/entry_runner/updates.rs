// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit optional preparation only after readiness and retain its independent family through cleanup.
use super::*;
impl<C: EntryActivation> EntryRunner<C> {
    /// Every updater job uses this runner's actual executor and globally shared identity sequences.
    pub(super) fn advance_updates(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<(), Box<dyn Error>> {
        if !self.closing
            && !self.update_attempted
            && crate::updates::configured()
            && let Some((key, application)) =
                self.authority.as_ref().and_then(CoordinatorAuthority::update_source)
        {
            self.update_attempted = true;
            let start = || -> Result<crate::updates::parent::Parent, Box<dyn Error>> {
                let client = self.client()?.clone();
                let work = client.create_scope(
                    ScopeSpec { id: self.scopes.allocate()?, deadline: None },
                    None,
                )?;
                let cleanup = client.create_scope(
                    ScopeSpec { id: self.scopes.allocate()?, deadline: None },
                    None,
                )?;
                let native = ScopeSpec { id: self.scopes.allocate()?, deadline: None };
                crate::updates::parent::Parent::new(client, work, cleanup, native, key, application)
                    .map_err(|cause| cause as Box<dyn Error>)
            };
            match start() {
                Ok(updater) => self.updater = Some(updater),
                Err(cause) => {
                    tracing::warn!(operation="update_helper_admission_failed", cause=%crate::failure_details::Details(cause.as_ref()), "Application is ready; background preparation was not admitted")
                }
            }
        }
        let normal = self.normal_shutdown && self.failure_source().is_none();
        if let Some(updater) = &mut self.updater {
            if self.closing {
                updater
                    .application_closing(self.shutdown, normal)
                    .map_err(|cause| cause as Box<dyn Error>)?;
            }
            updater.advance(batch, &self.jobs).map_err(|cause| cause as Box<dyn Error>)?;
        }
        Ok(())
    }
}
