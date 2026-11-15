// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove that update answers reach the current navigation in order, funded by its own lease.

use super::*;
use sugarsubstitute_application::updates_panel::IdleUpdateStatus;
use sugarsubstitute_contracts::{
    execution::{Generation, MemoryRelease},
    host_updates::{UpdatesProtocol, UpdatesRequestName},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    panel::PanelSession,
    startup::LaunchId,
};

type TestResult = Result<(), Box<dyn Error>>;

struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}

/// Whether the backend runs a workflow, as the host reads it on each call.
struct Workflow(bool);
impl WorkflowActivitySource for Workflow {
    fn workflow_running(&self) -> bool {
        self.0
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
    request_in(link, identity, message, UpdatesProtocol::V2)
}

/// Send one page request in the `protocol` version the page speaks.
fn request_in(
    link: &mut UpdatesPage,
    identity: MainIdentity,
    message: UpdatesClientMessage,
    protocol: UpdatesProtocol,
) -> TestResult {
    let lease = MemoryLease::new(1024 * 1024, Box::new(Release));
    let claim = lease.claim(PanelIngressReceipt::STORAGE_BYTES)?;
    let request = PageRequest { identity, message, protocol, lease, claim };
    link.observe(request, &mut IdleUpdateStatus, &Workflow(false))
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
    UpdatesPage::new()
}

/// Refresh the page's state with the backend running a workflow or not.
fn refresh(link: &mut UpdatesPage, running: bool) -> TestResult {
    link.refresh(&IdleUpdateStatus, &Workflow(running))
}

#[test]
fn a_watch_returns_its_credit_first_then_the_state_with_a_revision() -> TestResult {
    let page = identity(1)?;
    let mut link = idle()?;
    request(&mut link, page, watch(page)?)?;
    assert_eq!(drain(&mut link)?, [Sent::Receipt(page), Sent::Updates(page, Some(1))]);
    refresh(&mut link, false)?;
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
    let mut link = idle()?;
    request(&mut link, page, watch(page)?)?;
    refresh(&mut link, true)?;
    request(&mut link, page, check(page, "check-1")?)?;
    refresh(&mut link, false)?;
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
    let mut link = idle()?;
    request(&mut link, page, watch(page)?)?;
    link.retire(page);
    refresh(&mut link, true)?;
    assert_eq!(drain(&mut link)?, [Sent::Receipt(page)]);
    Ok(())
}

#[test]
fn the_page_is_told_this_builds_version() -> TestResult {
    assert_eq!(idle()?.current_version.as_str(), env!("CARGO_PKG_VERSION"));
    Ok(())
}

/// The update messages sent to the page, decoded.
fn sent_updates(link: &mut UpdatesPage) -> Result<Vec<serde_json::Value>, Box<dyn Error>> {
    let mut sent = Vec::new();
    while let Some(command) = link.take_command() {
        if let DesktopStartupCommand::SendUpdates(command) = command {
            sent.push(serde_json::from_slice(command.payload())?);
        }
    }
    Ok(sent)
}

#[test]
fn a_released_version_one_page_gets_version_one_answers_and_a_newer_page_the_named_state()
-> TestResult {
    for (protocol, version, product) in
        [(UpdatesProtocol::V1, 1, None), (UpdatesProtocol::V2, 2, Some("SugarSubstitute"))]
    {
        let mut link = idle()?;
        let page = identity(1)?;
        request_in(&mut link, page, watch(page)?, protocol)?;
        request_in(&mut link, page, check(page, "check-1")?, protocol)?;
        let sent = sent_updates(&mut link)?;
        let kinds: Vec<_> = sent.iter().map(|message| message["type"].clone()).collect();
        assert_eq!(kinds, ["state", "rejected"], "{protocol:?}");
        for message in &sent {
            assert_eq!(message["version"], version, "{protocol:?}");
        }
        assert_eq!(sent[0]["update"]["product"].as_str(), product, "{protocol:?}");
    }
    Ok(())
}
