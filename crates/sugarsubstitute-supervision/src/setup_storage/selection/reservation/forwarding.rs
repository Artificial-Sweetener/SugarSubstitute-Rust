// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reserve a live trial's verified image for ordinary coordinator forwarding, without trial custody.
use super::*;

impl SelectionStore {
    /// Called only after the run owner's exact sharing-contention result. The record and payload
    /// are independently verified; contention alone grants no executable or process authority.
    /// The receiving Root uses ordinary coordinator election. Forwarding cannot accept a trial,
    /// and an elected receiver without the original trial transfer cannot admit this generation.
    pub(super) fn reserve_forwarding(
        self: &Arc<Self>,
        context: &JobContext,
    ) -> Result<ReservedLaunch, SelectionFailure> {
        let Some((trial, selection)) = self.protected(context, |protection| {
            let _reader = reader(&self.root, protection, context)?;
            let current = self.read(protection)?.ok_or(SelectionFailure::Missing)?;
            let StartupSelection::ReconcileTrial(trial) = current.startup_selection() else {
                return Ok(None);
            };
            // Its forward purpose cannot be mistaken for a launch reservation by an elected
            // receiver. Delete-on-last-close also survives interrupted forwarding.
            let selection = super::snapshot::create(
                self,
                protection,
                &current,
                super::snapshot::Purpose::Forward,
                context,
            )?;
            Ok(Some((trial.clone(), selection)))
        })?
        else {
            return self.reserve_launch(context);
        };
        let application =
            VerifiedApplication::open(&self._installation, trial.generation.clone(), context)?;
        application.require_installed_launch()?;
        check(context)?;
        Ok(ReservedLaunch::Application(ReservedApplication {
            application,
            _selection: selection,
            _store: Arc::clone(self),
        }))
    }
}
