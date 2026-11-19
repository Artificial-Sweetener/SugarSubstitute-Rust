// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove that release notes answers and "What's new" reach the current navigation in order,
//! that fetches of one range are shared, and that every failure is answered.

use super::*;
use serde_json::Value;
use sugarsubstitute_contracts::{
    execution::{Generation, MemoryRelease},
    host_release_notes::ReleaseNotesRelease,
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    panel::PanelSession,
    startup::LaunchId,
};

type TestResult = Result<(), Box<dyn Error>>;

struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}

/// Fetches whose outcomes the test decides, recording every range started.
#[derive(Default)]
struct Fetches {
    started: Vec<String>,
    refuse: bool,
    ended: Option<Result<Selection, ReleaseNotesFailure>>,
}

impl NotesFetches for Fetches {
    fn start(
        &mut self,
        running: &ReleaseNotesVersion,
        since: &ReleaseNotesVersion,
    ) -> Result<(), Box<dyn Error>> {
        assert_eq!(running.as_str(), CURRENT_VERSION);
        if self.refuse {
            return Err("Execution is closing".into());
        }
        self.started.push(since.as_str().to_owned());
        Ok(())
    }

    fn take(&mut self) -> Option<Result<Selection, ReleaseNotesFailure>> {
        self.ended.take()
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
    link: &mut ReleaseNotesPage,
    identity: MainIdentity,
    message: ReleaseNotesClientMessage,
    fetches: &mut Fetches,
) -> TestResult {
    let lease = MemoryLease::new(8 * 1024 * 1024, Box::new(Release));
    let claim = lease.claim(PanelIngressReceipt::STORAGE_BYTES)?;
    link.observe(identity, message, lease, claim, fetches)
}

fn watch(identity: MainIdentity) -> Result<ReleaseNotesClientMessage, Box<dyn Error>> {
    Ok(ReleaseNotesClientMessage::Watch(PanelSession::for_main(identity)?))
}

fn fetch(
    identity: MainIdentity,
    name: &str,
    since: &str,
) -> Result<ReleaseNotesClientMessage, Box<dyn Error>> {
    Ok(ReleaseNotesClientMessage::Load {
        session: PanelSession::for_main(identity)?,
        request: ReleaseNotesRequestName::new(name)?,
        since: ReleaseNotesVersion::new(since)?,
    })
}

fn selection() -> Result<Selection, Box<dyn Error>> {
    Ok(Selection {
        releases: vec![ReleaseNotesRelease::new(
            ReleaseNotesVersion::new(CURRENT_VERSION)?,
            "## notes\n\n* a change".into(),
        )?],
        omitted: 0,
    })
}

/// One drained command: a receipt, or a release notes message's type and request.
#[derive(Debug, PartialEq)]
enum Sent {
    Receipt(MainIdentity),
    Notes(MainIdentity, String, Option<String>),
}

fn drain(link: &mut ReleaseNotesPage) -> Result<Vec<Sent>, Box<dyn Error>> {
    let mut sent = Vec::new();
    while let Some(command) = link.take_command() {
        sent.push(match command {
            DesktopStartupCommand::CompletePanelIngress(receipt) => {
                Sent::Receipt(receipt.identity())
            }
            DesktopStartupCommand::SendReleaseNotes(command) => {
                let value: Value = serde_json::from_slice(command.payload())?;
                let text = |field: &str| value[field].as_str().map(str::to_owned);
                let kind = text("type").ok_or("Message has no type")?;
                let detail = text("reason").or_else(|| text("request"));
                Sent::Notes(command.identity(), kind, detail)
            }
            _ => return Err("Unexpected command".into()),
        });
    }
    Ok(sent)
}

fn notes(page: MainIdentity, kind: &str, detail: Option<&str>) -> Sent {
    Sent::Notes(page, kind.into(), detail.map(str::to_owned))
}

#[test]
fn a_watch_returns_its_credit_then_names_the_product_and_running_version() -> TestResult {
    let page = identity(1)?;
    let mut link = ReleaseNotesPage::new()?;
    let mut fetches = Fetches::default();
    request(&mut link, page, watch(page)?, &mut fetches)?;
    let command = link.take_command();
    assert!(matches!(command, Some(DesktopStartupCommand::CompletePanelIngress(_))));
    let Some(DesktopStartupCommand::SendReleaseNotes(state)) = link.take_command() else {
        return Err("State missing".into());
    };
    let value: Value = serde_json::from_slice(state.payload())?;
    assert_eq!(value["type"], "state");
    assert_eq!(value["product"], crate::APPLICATION_NAME);
    assert_eq!(value["currentVersion"], CURRENT_VERSION);
    assert_eq!(state.revision(), Some(1));
    Ok(())
}

#[test]
fn requests_for_one_range_share_a_fetch_and_a_later_request_reuses_its_notes() -> TestResult {
    let page = identity(1)?;
    let mut link = ReleaseNotesPage::new()?;
    let mut fetches = Fetches::default();
    request(&mut link, page, fetch(page, "load-1", "0.0.1")?, &mut fetches)?;
    request(&mut link, page, fetch(page, "load-2", "0.0.1")?, &mut fetches)?;
    assert_eq!(fetches.started, ["0.0.1"]);
    assert_eq!(drain(&mut link)?, [Sent::Receipt(page), Sent::Receipt(page)]);
    fetches.ended = Some(Ok(selection()?));
    link.advance(&mut fetches)?;
    assert_eq!(
        drain(&mut link)?,
        [notes(page, "notes", Some("load-1")), notes(page, "notes", Some("load-2"))]
    );
    request(&mut link, page, fetch(page, "load-3", "0.0.1")?, &mut fetches)?;
    assert_eq!(fetches.started, ["0.0.1"], "the notes already read answer it");
    assert_eq!(drain(&mut link)?, [Sent::Receipt(page), notes(page, "notes", Some("load-3"))]);
    Ok(())
}

#[test]
fn a_failed_fetch_answers_each_request_and_a_retry_fetches_again() -> TestResult {
    let page = identity(1)?;
    let mut link = ReleaseNotesPage::new()?;
    let mut fetches = Fetches::default();
    request(&mut link, page, fetch(page, "load-1", "0.0.1")?, &mut fetches)?;
    fetches.ended = Some(Err(ReleaseNotesFailure::Unavailable));
    link.advance(&mut fetches)?;
    assert_eq!(
        drain(&mut link)?,
        [Sent::Receipt(page), notes(page, "failed", Some("unavailable"))]
    );
    request(&mut link, page, fetch(page, "load-2", "0.0.1")?, &mut fetches)?;
    assert_eq!(fetches.started, ["0.0.1", "0.0.1"], "a failure is never kept");
    fetches.ended = Some(Err(ReleaseNotesFailure::NotPublished));
    link.advance(&mut fetches)?;
    assert_eq!(
        drain(&mut link)?,
        [Sent::Receipt(page), notes(page, "failed", Some("not-published"))]
    );
    Ok(())
}

#[test]
fn a_fetch_that_cannot_start_is_answered_as_unavailable_at_once() -> TestResult {
    let page = identity(1)?;
    let mut link = ReleaseNotesPage::new()?;
    let mut fetches = Fetches { refuse: true, ..Fetches::default() };
    request(&mut link, page, fetch(page, "load-1", "0.0.1")?, &mut fetches)?;
    assert_eq!(
        drain(&mut link)?,
        [Sent::Receipt(page), notes(page, "failed", Some("unavailable"))]
    );
    Ok(())
}

#[test]
fn another_range_waits_for_the_running_fetch_then_starts() -> TestResult {
    let page = identity(1)?;
    let mut link = ReleaseNotesPage::new()?;
    let mut fetches = Fetches::default();
    request(&mut link, page, fetch(page, "load-1", "0.0.1")?, &mut fetches)?;
    request(&mut link, page, fetch(page, "load-2", "0.0.2")?, &mut fetches)?;
    assert_eq!(fetches.started, ["0.0.1"]);
    fetches.ended = Some(Ok(selection()?));
    link.advance(&mut fetches)?;
    assert_eq!(fetches.started, ["0.0.1", "0.0.2"]);
    assert_eq!(
        drain(&mut link)?,
        [Sent::Receipt(page), Sent::Receipt(page), notes(page, "notes", Some("load-1"))]
    );
    Ok(())
}

#[test]
fn a_new_navigation_drops_the_old_pages_requests() -> TestResult {
    let (first, second) = (identity(1)?, identity(2)?);
    let mut link = ReleaseNotesPage::new()?;
    let mut fetches = Fetches::default();
    request(&mut link, first, fetch(first, "load-1", "0.0.1")?, &mut fetches)?;
    request(&mut link, second, watch(second)?, &mut fetches)?;
    fetches.ended = Some(Ok(selection()?));
    link.advance(&mut fetches)?;
    assert_eq!(
        drain(&mut link)?,
        [Sent::Receipt(first), Sent::Receipt(second), notes(second, "state", None)]
    );
    Ok(())
}
