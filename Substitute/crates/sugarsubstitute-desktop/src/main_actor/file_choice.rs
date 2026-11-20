// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hold this Main's page request for a file dialog until the application shows it, and send
//! the page what the person chose.
use super::*;
use sugarsubstitute_contracts::host_file_choice::{
    FileChoiceClientMessage, FileChoiceFailure, FileChoiceHostMessage,
};

impl MainActor {
    /// Take the page's waiting request for a file dialog, to show it. `None` while closing.
    pub(crate) fn take_file_choice_request(&mut self) -> Option<FileChoiceClientMessage> {
        if self.closing() { None } else { self.file_choice.take() }
    }

    /// Send the page the outcome of the dialog it asked for; another request may follow.
    pub(crate) fn answer_file_choice(
        &mut self,
        message: &FileChoiceHostMessage,
    ) -> Result<(), DesktopError> {
        self.file_choice.finished();
        self.tell_file_choice_page(message)
    }

    /// Tell the current document. A page that is not revealed, or is closing, hears nothing:
    /// the document that asked is gone or going.
    fn tell_file_choice_page(
        &mut self,
        message: &FileChoiceHostMessage,
    ) -> Result<(), DesktopError> {
        if self.closing()
            || !self.revealed
            || self.readiness.as_ref().and_then(MainReadiness::panel_context).is_none()
        {
            return Ok(());
        }
        let command = crate::main_file_choice_page::FileChoiceCommand::new(
            self.identity(),
            message,
            self.request()?.storage_lease(),
        )
        .map_err(|cause| DesktopError::boxed("encode_file_choice_answer", cause))?;
        self.send_file_choice_page(command)
    }

    /// Hold the page's request for the application, or answer at once that a dialog is already
    /// waiting or showing.
    pub(super) fn answer_file_choice_page(
        &mut self,
        message: FileChoiceClientMessage,
        claim: sugarsubstitute_contracts::execution::MemoryClaim,
    ) -> Result<(), DesktopError> {
        let receipt = crate::PanelIngressReceipt::control(self.identity(), claim)
            .map_err(|cause| DesktopError::new("acknowledge_file_choice_message", cause))?;
        self.complete_panel_ingress(receipt)?;
        match self.file_choice.ask(message) {
            Ok(()) => Ok(()),
            Err(refused) => self.tell_file_choice_page(&FileChoiceHostMessage::Failed {
                request: refused.request,
                reason: FileChoiceFailure::Busy,
            }),
        }
    }
}
