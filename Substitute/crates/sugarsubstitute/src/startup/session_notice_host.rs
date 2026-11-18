// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route the page's session notice messages through the startup host.
//!
//! The workspace owns the previous close's record: the host hands what it read to the page's
//! notice owner, and has the record marked once a page says it showed the notice.
use super::*;

impl<B: StartupBackend> StartupHost<B> {
    /// Hand over what the previous close left once it is known, and have the close record
    /// marked once a page has shown its notice.
    pub(super) fn advance_session_notice(&mut self) -> Result<(), Box<dyn Error>> {
        #[cfg(feature = "native-composition")]
        if let Some(workspace) = &mut self.workspace {
            if let Some(previous) = workspace.take_previous_close() {
                self.session_notice.settle(previous.as_ref())?;
            }
            if self.session_notice.take_shown() {
                workspace.submit_notice_shown()?;
            }
        }
        // Without a workspace owner no close is recorded, so no notice is ever owed.
        #[cfg(not(feature = "native-composition"))]
        self.session_notice.settle(None)?;
        Ok(())
    }

    /// Consume the page's session notice messages and retire its session with its navigation;
    /// returns any other observation unchanged.
    pub(super) fn observe_session_notice(
        &mut self,
        observation: DesktopStartupObservation,
    ) -> Result<Option<DesktopStartupObservation>, Box<dyn Error>> {
        match observation {
            DesktopStartupObservation::SessionNoticeMessage(observed) => {
                let (identity, message, lease, claim) = observed.into_parts();
                self.session_notice.observe(identity, message, lease, claim)?;
                Ok(None)
            }
            other => {
                if let DesktopStartupObservation::MainRetired(identity)
                | DesktopStartupObservation::MainAbsent(identity)
                | DesktopStartupObservation::MainReloadRequested { identity, .. } = &other
                {
                    self.session_notice.retire(*identity);
                }
                Ok(Some(other))
            }
        }
    }
}
