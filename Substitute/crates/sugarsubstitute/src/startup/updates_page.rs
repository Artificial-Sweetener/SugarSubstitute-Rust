// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Answer the embedded page's `sugarcubes.updates` requests for the current navigation.
//!
//! The page's requests arrive as observations on the control lane; consuming one returns its
//! credit to the page. The application owner decides every answer and state from the host's
//! update-status port and workflow-activity source; this link binds those decisions to the
//! current navigation, funds each encoded message from that navigation's own lease and hands
//! them to the desktop in order. A newer navigation starts a new updates session.

use std::{collections::VecDeque, error::Error};
use sugarsubstitute_application::updates_panel::{
    UpdateStatusPort, UpdatesPanelSession, WorkflowActivitySource,
};
use sugarsubstitute_contracts::{
    execution::{MemoryClaim, MemoryLease},
    host_updates::{UpdatesClientMessage, UpdatesHostMessage, UpdatesProtocol, UpdatesVersion},
    startup::MainIdentity,
};
use sugarsubstitute_desktop::{DesktopStartupCommand, PanelIngressReceipt, UpdatesCommand};

/// The running release as the page shows it: this build's workspace version.
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The current navigation's updates session and the lease that funds its messages.
struct Page {
    identity: MainIdentity,
    /// The `sugarcubes.updates` version this page speaks, from its latest message.
    protocol: UpdatesProtocol,
    lease: MemoryLease,
    session: UpdatesPanelSession,
}

/// One page update request: the navigation that sent it, the `protocol` version it spoke, and
/// the page's `lease` and the `claim` that admitted it.
pub(crate) struct PageRequest {
    pub(crate) identity: MainIdentity,
    pub(crate) message: UpdatesClientMessage,
    pub(crate) protocol: UpdatesProtocol,
    pub(crate) lease: MemoryLease,
    pub(crate) claim: MemoryClaim,
}

/// One host's updates answers for whichever navigation is current.
pub(crate) struct UpdatesPage {
    current_version: UpdatesVersion,
    page: Option<Page>,
    receipts: VecDeque<PanelIngressReceipt>,
    outbox: VecDeque<UpdatesCommand>,
}

impl UpdatesPage {
    /// Answers reporting this build's version.
    ///
    /// # Errors
    /// Rejects a build version the updates schema cannot carry.
    pub(crate) fn new() -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            current_version: UpdatesVersion::new(CURRENT_VERSION)?,
            page: None,
            receipts: VecDeque::new(),
            outbox: VecDeque::new(),
        })
    }

    /// Consume one page `request`, decide it with the host's `updater` and `workflow`, and queue
    /// its answers, in the version the page spoke, in the order the page must see them.
    ///
    /// # Errors
    /// Rejects an allocation too small for the control-lane receipt, or answer storage the
    /// page's lease cannot fund.
    pub(crate) fn observe(
        &mut self,
        request: PageRequest,
        updater: &mut dyn UpdateStatusPort,
        workflow: &dyn WorkflowActivitySource,
    ) -> Result<(), Box<dyn Error>> {
        let PageRequest { identity, message, protocol, lease, claim } = request;
        // Consuming the request returns its control-lane credit to the page.
        self.receipts.push_back(PanelIngressReceipt::control(identity, claim)?);
        if self.page.as_ref().is_none_or(|page| page.identity != identity) {
            self.outbox.retain(|command| command.identity() == identity);
            self.page = Some(Page {
                identity,
                protocol,
                lease,
                session: UpdatesPanelSession::new(self.current_version.clone()),
            });
        }
        let Some(page) = &mut self.page else {
            return Ok(());
        };
        page.protocol = protocol;
        let answers = page.session.receive(message, updater, workflow);
        for answer in answers {
            self.queue(&answer)?;
        }
        Ok(())
    }

    /// Queue a newer state when what the current page shows from `updater` and `workflow` has
    /// changed.
    ///
    /// # Errors
    /// Rejects state storage the page's lease cannot fund.
    pub(crate) fn refresh(
        &mut self,
        updater: &dyn UpdateStatusPort,
        workflow: &dyn WorkflowActivitySource,
    ) -> Result<(), Box<dyn Error>> {
        let Some(page) = &mut self.page else {
            return Ok(());
        };
        if let Some(state) = page.session.refresh(updater, workflow) {
            self.queue(&state)?;
        }
        Ok(())
    }

    /// Forget a page whose navigation retired; its pending answers can no longer be delivered.
    pub(crate) fn retire(&mut self, identity: MainIdentity) {
        if self.page.as_ref().is_some_and(|page| page.identity == identity) {
            self.page = None;
        }
        self.outbox.retain(|command| command.identity() != identity);
    }

    /// Move the next owed host-direction command: receipts first, then answers and states in
    /// the order they were decided.
    pub(crate) fn take_command(&mut self) -> Option<DesktopStartupCommand> {
        if let Some(receipt) = self.receipts.pop_front() {
            return Some(DesktopStartupCommand::CompletePanelIngress(receipt));
        }
        self.outbox.pop_front().map(DesktopStartupCommand::SendUpdates)
    }

    /// Encode `message` for the current page from its own lease.
    fn queue(&mut self, message: &UpdatesHostMessage) -> Result<(), Box<dyn Error>> {
        let Some(page) = &self.page else {
            return Ok(());
        };
        let claim = page.lease.claim(UpdatesCommand::STORAGE_BYTES)?;
        let command = UpdatesCommand::new(page.identity, message, page.protocol, claim)?;
        // An undelivered older state is superseded; answers are never dropped.
        if command.revision().is_some()
            && let Some(index) = self.outbox.iter().position(|queued| queued.revision().is_some())
        {
            self.outbox.remove(index);
        }
        self.outbox.push_back(command);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
