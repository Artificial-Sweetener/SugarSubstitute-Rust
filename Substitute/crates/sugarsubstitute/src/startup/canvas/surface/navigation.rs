// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire only the closed native navigation's retained application work and replies.

use super::*;

impl NativeSurface {
    /// Remove old context waits even if the navigation never reached surface preparation.
    /// The desktop supplies actual main retirement; a newer navigation remains untouched.
    pub(in crate::startup::canvas) fn retire_navigation(
        &mut self,
        identity: MainIdentity,
        mut session: Option<&mut Session>,
    ) -> Result<(), Box<dyn Error>> {
        self.retire_preview_navigation(identity, session.as_deref_mut())?;
        self.editing.retire_navigation(identity);
        self.retire_deferred(identity);
        let retained = self.retained.as_ref().is_some_and(|binding| binding.identity == identity);
        let current = self.binding.as_ref().is_some_and(|binding| binding.identity == identity);
        if retained && self.binding.as_ref().is_some_and(|binding| binding.identity != identity) {
            return Err("A canvas transaction spans different native navigations".into());
        }
        if current || retained {
            self.discard_attachment(session)?;
        }
        if self.backend.is_some_and(|backend| backend.identity == identity) {
            self.backend = None;
        }
        if self.pending.as_ref().is_some_and(|pending| pending.identity == identity)
            && let Some(pending) = self.pending.take()
        {
            super::lifecycle::retired(&pending.request, pending.kind);
        }
        for slot in &mut self.contexts {
            if slot.as_ref().is_some_and(|context| context.identity == identity) {
                *slot = None;
            }
        }
        self.workflow.retire(identity);
        self.sources.retire(identity);
        let mut retained_count = 0;
        for index in 0..self.replies.len() {
            if self.replies[index].as_ref().is_some_and(|command| {
                let owner = match command {
                    DesktopStartupCommand::SendSurface(reply) => Some(reply.identity()),
                    DesktopStartupCommand::SendCanvasNative(reply) => Some(reply.identity()),
                    DesktopStartupCommand::SendCanvasNotification(notification) => {
                        Some(notification.identity())
                    }
                    DesktopStartupCommand::SendSurfacePresentation(reply) => Some(reply.identity()),
                    DesktopStartupCommand::CompletePanelIngress(receipt) => {
                        Some(receipt.identity())
                    }
                    _ => None,
                };
                owner == Some(identity)
            }) {
                self.replies[index] = None;
            } else if self.replies[index].is_some() {
                self.replies.swap(retained_count, index);
                retained_count += 1;
            }
        }
        Ok(())
    }
}
