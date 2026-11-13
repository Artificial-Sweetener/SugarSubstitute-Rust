// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove that update answers reach the current navigation in order, funded by its own lease.

use super::*;
use std::{cell::Cell, rc::Rc};
use sugarsubstitute_application::updates_panel::{IdleUpdateStatus, IdleWorkflowActivity};
use sugarsubstitute_contracts::{
    execution::{Generation, MemoryRelease},
    host_updates::UpdatesRequestName,
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    panel::PanelSession,
    startup::LaunchId,
};

type TestResult = Result<(), Box<dyn Error>>;

struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}

/// A workflow-activity source the test switches while the link owns it.
struct SharedWorkflow(Rc<Cell<bool>>);
impl WorkflowActivitySource for SharedWorkflow {
    fn workflow_running(&self) -> bool {
        self.0.get()
    }
}

fn identity(navigation: u64) -> Result<MainIdentity, Box<dyn Error>> {
    Ok(MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(1)?,
        navigation: Generation::new(navigation)?,
    })
}

fn request(
    link: &mut UpdatesPage,
    identity: MainIdentity,
    message: UpdatesClientMessage,
) -> TestResult {
    let lease = MemoryLease::new(1024 * 1024, Box::new(Release));
    let claim = lease.claim(PanelIngressReceipt::STORAGE_BYTES)?;
    link.observe(identity, message, lease, claim)
}

fn watch(identity: MainIdentity) -> Result<UpdatesClientMessage, Box<dyn Error>> {
    Ok(UpdatesClientMessage::Watch(PanelSession::for_main(identity)?))
}

fn check(identity: MainIdentity, name: &str) -> Result<UpdatesClientMessage, Box<dyn Error>> {
    Ok(UpdatesClientMessage::Check {
        session: PanelSession::for_main(identity)?,
        request: UpdatesRequestName::new(name)?,
    })
}

/// What one drained command is: a receipt, or an update message with its revision, if any.
#[derive(Debug, PartialEq)]
enum Sent {
    Receipt(MainIdentity),
    Updates(MainIdentity, Option<u64>),
}

fn drain(link: &mut UpdatesPage) -> Result<Vec<Sent>, Box<dyn Error>> {
    let mut sent = Vec::new();
    while let Some(command) = link.take_command() {
        sent.push(match command {
            DesktopStartupCommand::CompletePanelIngress(receipt) => {
                Sent::Receipt(receipt.identity())
            }
            DesktopStartupCommand::SendUpdates(command) => {
                Sent::Updates(command.identity(), command.revision())
            }
            _ => return Err("Unexpected command".into()),
        });
    }
    Ok(sent)
}

fn idle() -> Result<UpdatesPage, Box<dyn Error>> {
    UpdatesPage::new(Box::new(IdleUpdateStatus), Box::new(IdleWorkflowActivity))
}

#[test]
fn a_watch_returns_its_credit_first_then_the_state_with_a_revision() -> TestResult {
    let page = identity(1)?;
    let mut link = idle()?;
    request(&mut link, page, watch(page)?)?;
    assert_eq!(drain(&mut link)?, [Sent::Receipt(page), Sent::Updates(page, Some(1))]);
    link.refresh()?;
    assert_eq!(drain(&mut link)?, []);
    Ok(())
}

#[test]
fn every_check_is_answered_even_before_the_page_watches() -> TestResult {
    let page = identity(1)?;
    let mut link = idle()?;
    request(&mut link, page, check(page, "check-1")?)?;
    request(&mut link, page, check(page, "check-2")?)?;
    assert_eq!(
        drain(&mut link)?,
        [
            Sent::Receipt(page),
            Sent::Receipt(page),
            Sent::Updates(page, None),
            Sent::Updates(page, None)
        ]
    );
    Ok(())
}

#[test]
fn an_undelivered_state_is_superseded_by_a_newer_one_and_answers_are_kept() -> TestResult {
    let page = identity(1)?;
    let running = Rc::new(Cell::new(false));
    let mut link =
        UpdatesPage::new(Box::new(IdleUpdateStatus), Box::new(SharedWorkflow(running.clone())))?;
    request(&mut link, page, watch(page)?)?;
    running.set(true);
    link.refresh()?;
    request(&mut link, page, check(page, "check-1")?)?;
    running.set(false);
    link.refresh()?;
    assert_eq!(
        drain(&mut link)?,
        [
            Sent::Receipt(page),
            Sent::Receipt(page),
            Sent::Updates(page, None),
            Sent::Updates(page, Some(3))
        ]
    );
    Ok(())
}

#[test]
fn a_new_navigation_starts_a_new_session_and_drops_the_old_pages_answers() -> TestResult {
    let (first, second) = (identity(1)?, identity(2)?);
    let mut link = idle()?;
    request(&mut link, first, watch(first)?)?;
    request(&mut link, second, watch(second)?)?;
    assert_eq!(
        drain(&mut link)?,
        [Sent::Receipt(first), Sent::Receipt(second), Sent::Updates(second, Some(1))]
    );
    Ok(())
}

#[test]
fn a_retired_navigation_receives_nothing_more() -> TestResult {
    let page = identity(1)?;
    let running = Rc::new(Cell::new(false));
    let mut link =
        UpdatesPage::new(Box::new(IdleUpdateStatus), Box::new(SharedWorkflow(running.clone())))?;
    request(&mut link, page, watch(page)?)?;
    link.retire(page);
    running.set(true);
    link.refresh()?;
    assert_eq!(drain(&mut link)?, [Sent::Receipt(page)]);
    Ok(())
}

#[test]
fn the_page_is_told_this_builds_version() -> TestResult {
    assert_eq!(idle()?.current_version.as_str(), env!("CARGO_PKG_VERSION"));
    Ok(())
}
