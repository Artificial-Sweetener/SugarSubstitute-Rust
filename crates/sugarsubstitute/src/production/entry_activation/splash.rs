// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Join actual covered Desktop requests to the Root's retained native hide callback.
use super::*;
use crate::production::entry_runner::EntrySplashState;
use sugarsubstitute_contracts::startup::MainIdentity;

impl ProductionActivation {
    /// Expose authenticated appearance and the exact pending covered-frame request.
    pub(super) fn entry_splash_state(&self) -> EntrySplashState {
        let mut presentation = None;
        for retained in self.records.iter().flatten() {
            let record = &retained.record;
            if record.closed || record.failed.is_some() || record.pending_desktop_failure.is_some()
            {
                continue;
            }
            let projection = record
                .projected
                .as_ref()
                .or_else(|| record.start.as_ref().map(|start| &start.projection));
            if let Some(JobOutcome::Success(projected)) = projection.map(|value| &value.outcome) {
                presentation = Some(projected.presentation);
            }
            if let Some(identity) =
                record.parent.as_deref().and_then(ParentLifecycle::entry_splash_handoff)
                && identity
                    == (MainIdentity {
                        run: record.run,
                        launch: record.launch,
                        navigation: record.navigation,
                    })
            {
                return EntrySplashState { presentation, handoff: Some(identity) };
            }
        }
        EntrySplashState { presentation, handoff: None }
    }
    /// Route an actual Root hide acknowledgment to only its exact pending child request.
    pub(super) fn release_entry_splash(&mut self, identity: MainIdentity) -> Result<()> {
        if self.closing {
            return Err("Entry splash handoff admission is closed".into());
        }
        let record = self
            .records
            .iter_mut()
            .flatten()
            .map(|retained| &mut retained.record)
            .find(|record| {
                record.run == identity.run
                    && record.launch == identity.launch
                    && record.navigation == identity.navigation
            })
            .ok_or("Entry splash handoff has no original retained Desktop")?;
        if record.closed || record.failed.is_some() || record.pending_desktop_failure.is_some() {
            return Err("Entry splash handoff Desktop is retiring".into());
        }
        record
            .parent
            .as_mut()
            .ok_or("Entry splash lifecycle owner absent")?
            .complete_entry_splash_handoff(identity)
            .map_err(outer)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_activation_cannot_authorize_a_copied_presentation_identity() -> Result<()> {
        let mut activation = ProductionActivation::new(
            r"E:\unopened\sugarsubstitute.exe".into(),
            r"E:\unopened".into(),
            r"D:\unopened".into(),
        )?;
        let identity = MainIdentity {
            run: crate::production::child::test_support::run()?,
            launch: LaunchId::new(1)?,
            navigation: Generation::new(1)?,
        };
        assert!(activation.entry_splash().presentation.is_none());
        assert_eq!(activation.entry_splash().handoff, None);
        assert!(activation.complete_entry_splash_handoff(identity).is_err());
        activation.request_close(None)?;
        assert!(activation.complete_entry_splash_handoff(identity).is_err());
        assert_eq!(activation.entry_splash().handoff, None);
        Ok(())
    }
}
