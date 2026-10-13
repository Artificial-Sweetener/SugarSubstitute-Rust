// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve actual Desktop callback provenance before dispatching bounded lifecycle requests.
use super::*;
impl ProductionChild {
    pub(in super::super) fn observe_native(
        &mut self,
        _prepared: Option<&PreparedChildStartup>,
    ) -> Result<()> {
        if let Some(profile) = self.main.profile_mut()
            && profile.observe(&mut self.observation)?
        {
            return self.advance_profile();
        }
        match self.observation.as_ref() {
            Some(DesktopStartupObservation::UiChallengeProcessed(key)) => {
                self.bootstrap.channel_mut().ui_processed(*key)?;
            }
            Some(DesktopStartupObservation::NativeActivity { run, revision, activity })
                if *run == self.run =>
            {
                self.shutdown
                    .queue(Control::Activity { revision: *revision, activity: *activity })?
            }
            Some(DesktopStartupObservation::MainRetired(identity))
                if *identity == self.identity =>
            {
                self.shutdown.queue(Control::MainRetired(*identity))?;
                self.shutdown.main_retired = true;
            }
            Some(DesktopStartupObservation::NativeRetired(run)) if *run == self.run => {
                if !self.shutdown.main_retired && self.main.profile().is_some() {
                    self.shutdown.queue(Control::MainRetired(self.identity))?;
                    self.shutdown.main_retired = true;
                }
                self.shutdown.native_retired = true;
            }
            None => return Ok(()),
            _ => return Err("Native child observation does not match actual current owner".into()),
        }
        self.observation = None;
        self.advance_control()
    }
}
