// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Answer the embedded page's `sugarcubes.release-notes` requests for the current navigation,
//! and carry "What's new" from the caption to the page.
//!
//! The page's requests arrive as observations on the control lane; consuming one returns its
//! credit to the page. A watching page learns the host's product and running version, and is
//! told to show what is new when the user asks the host. A fetch runs on the host's release
//! notes fetch owner, one at a time; requests for the same range share it, and the last notes
//! read are kept for later requests of that range. Each encoded message is funded from its
//! navigation's own lease and handed to the desktop in order. A newer navigation starts a new
//! release notes session.

use super::release_notes_fetch::NotesFetches;
use std::{collections::VecDeque, error::Error};
use sugarsubstitute_contracts::{
    execution::{MemoryClaim, MemoryLease},
    host_release_notes::{
        ReleaseNotesClientMessage, ReleaseNotesFailure, ReleaseNotesHostMessage,
        ReleaseNotesRequestName, ReleaseNotesVersion,
    },
    startup::MainIdentity,
};
use sugarsubstitute_desktop::{DesktopStartupCommand, PanelIngressReceipt, ReleaseNotesCommand};
use sugarsubstitute_supervision::release_notes::Selection;

/// The running release as the page shows it: this build's workspace version.
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The current navigation's release notes session and the lease that funds its messages.
struct Page {
    identity: MainIdentity,
    lease: MemoryLease,
    revision: u64,
    watching: bool,
}

/// One page request waiting for the fetch of its range.
struct Waiting {
    identity: MainIdentity,
    request: ReleaseNotesRequestName,
    since: ReleaseNotesVersion,
}

/// One host's release notes answers for whichever navigation is current.
pub(crate) struct ReleaseNotesPage {
    current_version: ReleaseNotesVersion,
    page: Option<Page>,
    receipts: VecDeque<PanelIngressReceipt>,
    outbox: VecDeque<ReleaseNotesCommand>,
    waiting: VecDeque<Waiting>,
    /// The range the running fetch reads.
    fetching: Option<ReleaseNotesVersion>,
    /// The last notes read, by the range they answer.
    read: Option<(ReleaseNotesVersion, Selection)>,
}

impl ReleaseNotesPage {
    /// Answers naming this build's version.
    ///
    /// # Errors
    /// Rejects a build version the release notes schema cannot carry.
    pub(crate) fn new() -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            current_version: ReleaseNotesVersion::new(CURRENT_VERSION)?,
            page: None,
            receipts: VecDeque::new(),
            outbox: VecDeque::new(),
            waiting: VecDeque::new(),
            fetching: None,
            read: None,
        })
    }

    /// Consume one page request, attributed to `identity` and admitted with `claim`, and queue
    /// its answers; a fetch starts on `fetches` unless one for its range already runs.
    ///
    /// # Errors
    /// Rejects an allocation too small for the control-lane receipt, or answer storage the
    /// page's lease cannot fund.
    pub(crate) fn observe(
        &mut self,
        identity: MainIdentity,
        message: ReleaseNotesClientMessage,
        lease: MemoryLease,
        claim: MemoryClaim,
        fetches: &mut dyn NotesFetches,
    ) -> Result<(), Box<dyn Error>> {
        // Consuming the request returns its control-lane credit to the page.
        self.receipts.push_back(PanelIngressReceipt::control(identity, claim)?);
        if self.page.as_ref().is_none_or(|page| page.identity != identity) {
            self.retire_others(identity);
            self.page = Some(Page { identity, lease, revision: 0, watching: false });
        }
        match message {
            ReleaseNotesClientMessage::Watch(_) => {
                let Some(page) = &mut self.page else { return Ok(()) };
                page.watching = true;
                page.revision += 1;
                let state = ReleaseNotesHostMessage::State {
                    revision: page.revision,
                    product: crate::APPLICATION_NAME.into(),
                    current_version: self.current_version.clone(),
                };
                self.queue(&state)
            }
            ReleaseNotesClientMessage::Load { request, since, .. } => {
                if let Some((range, selection)) = &self.read
                    && *range == since
                {
                    let notes = notes(request, selection);
                    return self.queue(&notes);
                }
                self.waiting.push_back(Waiting { identity, request, since });
                self.pump(fetches)
            }
        }
    }

    /// Answer the requests the ended fetch on `fetches` read for, and start the next range.
    ///
    /// # Errors
    /// Rejects answer storage the page's lease cannot fund.
    pub(crate) fn advance(&mut self, fetches: &mut dyn NotesFetches) -> Result<(), Box<dyn Error>> {
        let Some(outcome) = fetches.take() else { return Ok(()) };
        let Some(range) = self.fetching.take() else { return self.pump(fetches) };
        self.answer(&range, outcome)?;
        self.pump(fetches)
    }

    /// Whether a page of the current navigation follows the host's release notes, so it can
    /// show what is new.
    pub(crate) fn watching(&self) -> bool {
        self.page.as_ref().is_some_and(|page| page.watching)
    }

    /// Ask the watching page to show what is new.
    ///
    /// # Errors
    /// Refuses when no page follows the host's release notes, and rejects storage the page's
    /// lease cannot fund.
    pub(crate) fn open(&mut self) -> Result<(), Box<dyn Error>> {
        if !self.watching() {
            return Err("No page shows the release notes".into());
        }
        self.queue(&ReleaseNotesHostMessage::Open)
    }

    /// Forget a page whose navigation retired; its pending answers can no longer be delivered.
    pub(crate) fn retire(&mut self, identity: MainIdentity) {
        if self.page.as_ref().is_some_and(|page| page.identity == identity) {
            self.page = None;
        }
        self.outbox.retain(|command| command.identity() != identity);
        self.waiting.retain(|waiting| waiting.identity != identity);
    }

    /// Move the next owed host-direction command: receipts first, then states and answers in
    /// the order they were decided.
    pub(crate) fn take_command(&mut self) -> Option<DesktopStartupCommand> {
        if let Some(receipt) = self.receipts.pop_front() {
            return Some(DesktopStartupCommand::CompletePanelIngress(receipt));
        }
        self.outbox.pop_front().map(DesktopStartupCommand::SendReleaseNotes)
    }

    /// Drop every queued answer and waiting request of navigations other than `identity`.
    fn retire_others(&mut self, identity: MainIdentity) {
        self.outbox.retain(|command| command.identity() == identity);
        self.waiting.retain(|waiting| waiting.identity == identity);
    }

    /// Start the oldest waiting range when no fetch runs; a fetch that cannot start fails its
    /// requests as unavailable at once.
    fn pump(&mut self, fetches: &mut dyn NotesFetches) -> Result<(), Box<dyn Error>> {
        while self.fetching.is_none()
            && let Some(next) = self.waiting.front()
        {
            let range = next.since.clone();
            match fetches.start(&self.current_version, &range) {
                Ok(()) => self.fetching = Some(range),
                Err(cause) => {
                    tracing::warn!(operation = "release_notes_fetch", %cause,
                        "The release notes fetch could not start");
                    self.answer(&range, Err(ReleaseNotesFailure::Unavailable))?;
                }
            }
        }
        Ok(())
    }

    /// Answer every request waiting for `range` with `outcome`, keeping notes for later ones.
    fn answer(
        &mut self,
        range: &ReleaseNotesVersion,
        outcome: Result<Selection, ReleaseNotesFailure>,
    ) -> Result<(), Box<dyn Error>> {
        let (answered, waiting): (VecDeque<Waiting>, VecDeque<Waiting>) =
            std::mem::take(&mut self.waiting).into_iter().partition(|item| item.since == *range);
        self.waiting = waiting;
        for item in answered {
            let message = match &outcome {
                Ok(selection) => notes(item.request, selection),
                Err(reason) => {
                    ReleaseNotesHostMessage::Failed { request: item.request, reason: *reason }
                }
            };
            self.queue(&message)?;
        }
        if let Ok(selection) = outcome {
            self.read = Some((range.clone(), selection));
        }
        Ok(())
    }

    /// Encode `message` for the current page from its own lease.
    fn queue(&mut self, message: &ReleaseNotesHostMessage) -> Result<(), Box<dyn Error>> {
        let Some(page) = &self.page else {
            return Ok(());
        };
        let command = ReleaseNotesCommand::new(page.identity, message, &page.lease)?;
        // An undelivered older state is superseded; nothing else is ever dropped.
        if command.revision().is_some()
            && let Some(index) = self.outbox.iter().position(|queued| queued.revision().is_some())
        {
            self.outbox.remove(index);
        }
        self.outbox.push_back(command);
        Ok(())
    }
}

/// The notes answer to `request`.
fn notes(request: ReleaseNotesRequestName, selection: &Selection) -> ReleaseNotesHostMessage {
    ReleaseNotesHostMessage::Notes {
        request,
        releases: selection.releases.clone(),
        omitted: selection.omitted,
    }
}

#[cfg(test)]
mod tests;
