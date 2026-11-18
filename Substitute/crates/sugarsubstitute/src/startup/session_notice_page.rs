// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Tell the embedded page what the host owes the user about their session
//! (`sugarcubes.session-notice`), and learn when the page has shown it.
//!
//! The page's messages arrive as observations on the control lane; consuming one returns its
//! credit to the page. A watching page is answered once the host knows what the previous close
//! left: the notice of unsaved work, or none. The notice stays owed, and is offered to every
//! later navigation, until a page says it showed it; the owner of the close record then marks
//! it. Each encoded message is funded from its navigation's own lease.

use std::{collections::VecDeque, error::Error};
use sugarsubstitute_application::workspace_session::close_save::{NotSavedLastTime, UnsavedItem};
use sugarsubstitute_contracts::{
    execution::{MemoryClaim, MemoryLease},
    host_session_notice::{NoticeNumber, SessionNoticeClientMessage, SessionNoticeHostMessage},
    startup::MainIdentity,
};
use sugarsubstitute_desktop::{DesktopStartupCommand, PanelIngressReceipt, SessionNoticeCommand};

/// The one notice a run can owe: what the previous close did not keep.
const NOT_SAVED: u64 = 1;

/// What the host owes the user about the previous close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Owed {
    /// The previous close's record has not been read yet.
    Unknown,
    /// Nothing, or a notice a page has already shown.
    Nothing,
    /// The previous close did not keep these.
    NotSaved { workspace: bool, inputs: bool },
}

/// The current navigation's session and the lease that funds its messages.
struct Page {
    identity: MainIdentity,
    lease: MemoryLease,
    /// The page asked and has not been answered, because the record was not read yet.
    waiting: bool,
}

/// One host's session notice for whichever navigation is current.
pub(crate) struct SessionNoticePage {
    owed: Owed,
    page: Option<Page>,
    receipts: VecDeque<PanelIngressReceipt>,
    outbox: VecDeque<SessionNoticeCommand>,
    shown: bool,
}

impl SessionNoticePage {
    /// A host that does not yet know what the previous close left.
    pub(crate) const fn new() -> Self {
        Self {
            owed: Owed::Unknown,
            page: None,
            receipts: VecDeque::new(),
            outbox: VecDeque::new(),
            shown: false,
        }
    }

    /// Learn what the previous close left, once, and answer a page that already asked.
    ///
    /// # Errors
    /// Rejects answer storage the page's lease cannot fund.
    pub(crate) fn settle(
        &mut self,
        notice: Option<&NotSavedLastTime>,
    ) -> Result<(), Box<dyn Error>> {
        if self.owed != Owed::Unknown {
            return Ok(());
        }
        let lost = |wanted: fn(&UnsavedItem) -> bool| {
            notice.is_some_and(|notice| notice.items.iter().any(wanted))
        };
        let workspace = lost(|item| matches!(item, UnsavedItem::Workspace(_)));
        let inputs = lost(|item| matches!(item, UnsavedItem::InputCanvas(_)));
        self.owed =
            if workspace || inputs { Owed::NotSaved { workspace, inputs } } else { Owed::Nothing };
        if self.page.as_ref().is_some_and(|page| page.waiting) {
            self.answer()?;
        }
        Ok(())
    }

    /// Consume one page message, attributed to `identity` and admitted with `claim`, and queue
    /// its answer.
    ///
    /// # Errors
    /// Rejects an allocation too small for the control-lane receipt, or answer storage the
    /// page's lease cannot fund.
    pub(crate) fn observe(
        &mut self,
        identity: MainIdentity,
        message: SessionNoticeClientMessage,
        lease: MemoryLease,
        claim: MemoryClaim,
    ) -> Result<(), Box<dyn Error>> {
        // Consuming the message returns its control-lane credit to the page.
        self.receipts.push_back(PanelIngressReceipt::control(identity, claim)?);
        if self.page.as_ref().is_none_or(|page| page.identity != identity) {
            self.outbox.retain(|command| command.identity() == identity);
            self.page = Some(Page { identity, lease, waiting: false });
        }
        match message {
            SessionNoticeClientMessage::Watch(_) => self.answer(),
            SessionNoticeClientMessage::Shown { notice, .. } => {
                if notice.get() == NOT_SAVED && matches!(self.owed, Owed::NotSaved { .. }) {
                    self.owed = Owed::Nothing;
                    self.shown = true;
                }
                Ok(())
            }
        }
    }

    /// Whether a page showed the notice since this was last asked; the close record's owner
    /// marks it so the notice is not owed to a later run.
    pub(crate) fn take_shown(&mut self) -> bool {
        std::mem::take(&mut self.shown)
    }

    /// Forget a page whose navigation retired; its pending answers can no longer be delivered.
    /// A notice it did not show stays owed to the next page.
    pub(crate) fn retire(&mut self, identity: MainIdentity) {
        if self.page.as_ref().is_some_and(|page| page.identity == identity) {
            self.page = None;
        }
        self.outbox.retain(|command| command.identity() != identity);
    }

    /// Move the next owed host-direction command: receipts first, then answers in order.
    pub(crate) fn take_command(&mut self) -> Option<DesktopStartupCommand> {
        if let Some(receipt) = self.receipts.pop_front() {
            return Some(DesktopStartupCommand::CompletePanelIngress(receipt));
        }
        self.outbox.pop_front().map(DesktopStartupCommand::SendSessionNotice)
    }

    /// Answer the current page's watch, or keep it waiting until the record has been read.
    fn answer(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(page) = &mut self.page else { return Ok(()) };
        let message = match self.owed {
            Owed::Unknown => {
                page.waiting = true;
                return Ok(());
            }
            Owed::Nothing => SessionNoticeHostMessage::None,
            Owed::NotSaved { workspace, inputs } => SessionNoticeHostMessage::NotSaved {
                notice: NoticeNumber::new(NOT_SAVED)?,
                workspace,
                inputs,
            },
        };
        page.waiting = false;
        self.outbox.push_back(SessionNoticeCommand::new(page.identity, &message, &page.lease)?);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
