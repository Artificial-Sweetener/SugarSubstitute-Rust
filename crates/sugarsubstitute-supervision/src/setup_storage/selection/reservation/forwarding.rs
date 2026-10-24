// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reserve a live trial's verified image for ordinary coordinator forwarding, without trial custody.
use super::*;
use crate::setup_storage::disk_inheritance::duplicate_readonly_inheritable;

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
            let _writer = writer(&self.root, protection, context)?;
            let (snapshot, _source) =
                self.read_pinned(protection)?.ok_or(SelectionFailure::Missing)?;
            let StartupSelection::ReconcileTrial(trial) = snapshot.startup_selection() else {
                return Ok(None);
            };
            // A separate native object cannot be mistaken for a current-selection pin by an
            // elected receiver. Delete-on-last-close also survives interrupted forwarding.
            let path = staging::unique_path(&self.root, ".selection-forward")?;
            let mut file = protection.transient_file(&path, context)?;
            file.write_all(&snapshot.encode()?)?;
            file.sync_all()?;
            let selection = duplicate_readonly_inheritable(&file)?;
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
