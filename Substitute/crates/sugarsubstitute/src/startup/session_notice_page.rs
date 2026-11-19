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
//!
//! The host's own lines (the close save, a ready or failed update, the start-up update
//! messages) take this road too, so nothing is ever written in the title bar. A page that
//! speaks version 2 is sent each line once it watches, and every later line as it arises; a
//! line stays owed, and is sent again to the next navigation, until a page says it showed it.

use std::{collections::VecDeque, error::Error};
use sugarsubstitute_application::workspace_session::close_save::{NotSavedLastTime, UnsavedItem};
use sugarsubstitute_contracts::{
    execution::{MemoryClaim, MemoryLease},
    host_session_notice::{
        MAX_LINE_CHARACTERS, NoticeNumber, NoticeTone, SessionNoticeClientMessage,
        SessionNoticeHostMessage, SessionNoticeVersion,
    },
    startup::MainIdentity,
};
use sugarsubstitute_desktop::{DesktopStartupCommand, PanelIngressReceipt, SessionNoticeCommand};

/// The notice of what the previous close did not keep; lines are numbered after it.
const NOT_SAVED: u64 = 1;
/// The most lines kept for a page that has not shown them; the oldest gives way to a newer.
const MAX_LINES: usize = 16;

/// One sentence the host has for the user, in the user's language.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostLine {
    /// How the line reads.
    pub(crate) tone: NoticeTone,
    /// The sentence.
    pub(crate) text: String,
    /// Whether the page keeps it on screen until the user closes it.
    pub(crate) lasting: bool,
}

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
    /// The version the page's watch spoke; `None` until it watches.
    version: Option<SessionNoticeVersion>,
    /// The highest line number already sent to this page.
    sent: u64,
}

/// One host's session notice for whichever navigation is current.
pub(crate) struct SessionNoticePage {
    owed: Owed,
    page: Option<Page>,
    receipts: VecDeque<PanelIngressReceipt>,
    outbox: VecDeque<SessionNoticeCommand>,
    shown: bool,
    /// Lines no page has shown yet, oldest first, each under its notice number.
    lines: VecDeque<(u64, HostLine)>,
    last_line: u64,
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
            lines: VecDeque::new(),
            last_line: NOT_SAVED,
        }
    }

    /// Owe the user `line`, and send it now to a page that already watches in version 2. A
    /// line already owed in the same words is not owed twice.
    ///
    /// # Errors
    /// Rejects storage the page's lease cannot fund.
    pub(crate) fn tell(&mut self, mut line: HostLine) -> Result<(), Box<dyn Error>> {
        if line.text.trim().is_empty() || self.lines.iter().any(|(_, owed)| owed.text == line.text)
        {
            return Ok(());
        }
        if line.text.chars().count() > MAX_LINE_CHARACTERS {
            line.text = line.text.chars().take(MAX_LINE_CHARACTERS).collect();
        }
        if self.lines.len() == MAX_LINES {
            self.lines.pop_front();
        }
        self.last_line += 1;
        self.lines.push_back((self.last_line, line));
        self.send_lines()
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
            self.page = Some(Page { identity, lease, waiting: false, version: None, sent: 0 });
        }
        match message {
            SessionNoticeClientMessage::Watch { version, .. } => {
                if let Some(page) = &mut self.page {
                    page.version = Some(version);
                    // A page that watches again is told every line it has not shown.
                    page.sent = 0;
                }
                self.answer()?;
                self.send_lines()
            }
            SessionNoticeClientMessage::Shown { notice, .. } => {
                if notice.get() == NOT_SAVED {
                    if matches!(self.owed, Owed::NotSaved { .. }) {
                        self.owed = Owed::Nothing;
                        self.shown = true;
                    }
                } else {
                    self.lines.retain(|(number, _)| *number != notice.get());
                }
                Ok(())
            }
        }
    }

    /// The lines no page has shown yet, oldest first.
    #[cfg(test)]
    pub(crate) fn owed_lines(&self) -> Vec<&HostLine> {
        self.lines.iter().map(|(_, line)| line).collect()
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

    /// Send the watching version 2 page every owed line it has not been sent yet.
    fn send_lines(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(page) = &mut self.page else { return Ok(()) };
        if page.version != Some(SessionNoticeVersion::Two) {
            return Ok(());
        }
        for (number, line) in &self.lines {
            if *number <= page.sent {
                continue;
            }
            let message = SessionNoticeHostMessage::Line {
                notice: NoticeNumber::new(*number)?,
                tone: line.tone,
                text: line.text.clone(),
                lasting: line.lasting,
            };
            self.outbox.push_back(SessionNoticeCommand::new(
                page.identity,
                &message,
                SessionNoticeVersion::Two,
                &page.lease,
            )?);
            page.sent = *number;
        }
        Ok(())
    }

    /// Answer the current page's watch, or keep it waiting until the record has been read.
    fn answer(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(page) = &mut self.page else { return Ok(()) };
        let Some(version) = page.version else { return Ok(()) };
        // A version 2 page that is owed lines is answered with them rather than with "none".
        let lined = version == SessionNoticeVersion::Two && !self.lines.is_empty();
        let message = match self.owed {
            Owed::Unknown => {
                page.waiting = true;
                return Ok(());
            }
            Owed::Nothing if lined => {
                page.waiting = false;
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
        self.outbox.push_back(SessionNoticeCommand::new(
            page.identity,
            &message,
            version,
            &page.lease,
        )?);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
