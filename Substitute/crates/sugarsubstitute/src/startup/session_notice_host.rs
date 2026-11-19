// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route the page's session notice messages through the startup host.
//!
//! The workspace owns the previous close's record: the host hands what it read to the page's
//! notice owner, and has the record marked once a page says it showed the notice. The host's
//! own lines (the close save, a ready or failed update, the start-up update messages) are
//! handed over here too, so the page shows them and the title bar never does.
use super::*;

impl<B: StartupBackend> StartupHost<B> {
    /// Hand over what the previous close left once it is known, and have the close record
    /// marked once a page has shown its notice.
    pub(super) fn advance_session_notice(&mut self) -> Result<(), Box<dyn Error>> {
        // A notification the page shows behind a window that is not up yet is gone before the
        // user can read it, so the lines about updates wait for the window to be revealed.
        let revealed = self.startup.as_ref().is_some_and(|startup| {
            startup.session().phase() == sugarsubstitute_application::startup::StartupPhase::Running
        });
        self.tell_host_lines(revealed)?;
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

    /// Owe the page each of the host's own lines that is due this turn. The close save is
    /// said whenever it runs; the start-up update messages and a ready or failed update are
    /// said only once the window is `revealed`, and wait until then.
    pub(super) fn tell_host_lines(&mut self, revealed: bool) -> Result<(), Box<dyn Error>> {
        use sugarsubstitute_application::workspace_session::close_save::CloseSaveProgress;
        let words = sugarsubstitute_desktop::StartupWindowContent::locale_messages(self);
        while revealed && let Some(notice) = self.child.take_update_notice() {
            self.session_notice.tell(session_notice_lines::startup_line(words, &notice))?;
        }
        let saving = matches!(
            self.close_save_progress(),
            Some(CloseSaveProgress::Saving { .. } | CloseSaveProgress::Recording(_))
        );
        let status = self.child.update_status().filter(|_| revealed);
        let failed =
            status.is_some_and(sugarsubstitute_desktop::report::surface::has_update_report);
        for line in self.notice_lines.due(words, saving, status, failed) {
            self.session_notice.tell(line)?;
        }
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
