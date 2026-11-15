// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decide what a closing native session does with each queued startup command.
use super::DesktopStartupCommand;

/// What a closing native session does with one queued command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClosingAdmission {
    /// Close-owned work still runs.
    Run,
    /// Traffic addressed to the leaving page is dropped: nothing remains to receive it.
    Drop,
    /// Any other command after close began is an owner error.
    Refuse,
}
impl DesktopStartupCommand {
    /// Classify this command once the native session began closing. A page message produced by
    /// work that finished as the window closed, such as a workspace save's outcome, is dropped.
    pub(crate) const fn while_closing(&self) -> ClosingAdmission {
        match self {
            Self::Close
            | Self::RetireMain { .. }
            | Self::ConfirmMainAbsent(_)
            | Self::ProcessUiChallenge(_) => ClosingAdmission::Run,
            Self::SendSurfacePresentation(_)
            | Self::SendCanvasNotification(_)
            | Self::SendSession(_)
            | Self::SendSessionMedia(_) => ClosingAdmission::Drop,
            _ => ClosingAdmission::Refuse,
        }
    }
}
