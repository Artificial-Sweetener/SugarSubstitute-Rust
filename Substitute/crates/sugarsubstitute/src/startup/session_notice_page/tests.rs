// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove that the notice of unsaved work reaches a watching page, stays owed until a page
//! shows it, and is never invented.

use super::*;
use serde_json::{Value, json};
use sugarsubstitute_application::workspace_session::close_save::{
    CloseReason, SaveFailure, UnsavedCause,
};
use sugarsubstitute_contracts::{
    execution::{Generation, MemoryRelease},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    panel::PanelSession,
    startup::LaunchId,
};

type TestResult = Result<(), Box<dyn Error>>;

struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
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

fn send(
    page: &mut SessionNoticePage,
    identity: MainIdentity,
    message: SessionNoticeClientMessage,
) -> TestResult {
    let lease = MemoryLease::new(8 * 1024 * 1024, Box::new(Release));
    let claim = lease.claim(PanelIngressReceipt::STORAGE_BYTES)?;
    page.observe(identity, message, lease, claim)
}

fn watch(page: &mut SessionNoticePage, identity: MainIdentity) -> TestResult {
    send(page, identity, SessionNoticeClientMessage::Watch(PanelSession::for_main(identity)?))
}

fn shown(page: &mut SessionNoticePage, identity: MainIdentity, notice: u64) -> TestResult {
    let message = SessionNoticeClientMessage::Shown {
        session: PanelSession::for_main(identity)?,
        notice: NoticeNumber::new(notice)?,
    };
    send(page, identity, message)
}

/// Drain every owed command: how many receipts, and each answer as the page reads it.
fn drain(page: &mut SessionNoticePage) -> Result<(usize, Vec<Value>), Box<dyn Error>> {
    let (mut receipts, mut answers) = (0, Vec::new());
    while let Some(command) = page.take_command() {
        match command {
            DesktopStartupCommand::CompletePanelIngress(_) => receipts += 1,
            DesktopStartupCommand::SendSessionNotice(answer) => {
                let mut value: Value = serde_json::from_slice(answer.payload())?;
                value.as_object_mut().ok_or("Answer is not an object")?.remove("session");
                answers.push(value);
            }
            _ => return Err("Unexpected command".into()),
        }
    }
    Ok((receipts, answers))
}

fn lost_workflow_changes() -> NotSavedLastTime {
    NotSavedLastTime {
        reason: CloseReason::Close,
        items: vec![UnsavedItem::Workspace(UnsavedCause::Failed(SaveFailure::Storage))],
    }
}

fn not_saved(workspace: bool, inputs: bool) -> Value {
    json!({"protocol": "sugarcubes.session-notice", "version": 1, "type": "not-saved",
        "notice": 1, "workspace": workspace, "inputs": inputs})
}

fn none() -> Value {
    json!({"protocol": "sugarcubes.session-notice", "version": 1, "type": "none"})
}

#[test]
fn a_watching_page_is_told_what_the_previous_close_lost_until_a_page_shows_it() -> TestResult {
    let mut page = SessionNoticePage::new();
    page.settle(Some(&lost_workflow_changes()))?;
    watch(&mut page, identity(1)?)?;
    assert_eq!(drain(&mut page)?, (1, vec![not_saved(true, false)]));
    assert!(!page.take_shown(), "nothing is marked before a page shows it");

    // A reloaded page is owed the same notice.
    page.retire(identity(1)?);
    watch(&mut page, identity(2)?)?;
    assert_eq!(drain(&mut page)?, (1, vec![not_saved(true, false)]));

    shown(&mut page, identity(2)?, 1)?;
    assert_eq!(drain(&mut page)?, (1, vec![]));
    assert!(page.take_shown());
    assert!(!page.take_shown(), "reported once");

    watch(&mut page, identity(2)?)?;
    assert_eq!(drain(&mut page)?, (1, vec![none()]));
    Ok(())
}

#[test]
fn a_page_that_asks_before_the_record_is_read_is_answered_once_it_is() -> TestResult {
    let mut page = SessionNoticePage::new();
    watch(&mut page, identity(1)?)?;
    assert_eq!(drain(&mut page)?, (1, vec![]), "the receipt is owed at once, the answer later");
    let both = NotSavedLastTime {
        reason: CloseReason::RestartToApply,
        items: vec![
            UnsavedItem::Workspace(UnsavedCause::Failed(SaveFailure::Capture)),
            UnsavedItem::InputCanvas(UnsavedCause::Failed(SaveFailure::Storage)),
        ],
    };
    page.settle(Some(&both))?;
    assert_eq!(drain(&mut page)?, (0, vec![not_saved(true, true)]));
    // What the previous close left is learned once.
    page.settle(None)?;
    watch(&mut page, identity(1)?)?;
    assert_eq!(drain(&mut page)?, (1, vec![not_saved(true, true)]));
    Ok(())
}

#[test]
fn a_close_that_kept_everything_owes_nothing_and_a_stray_shown_marks_nothing() -> TestResult {
    let mut page = SessionNoticePage::new();
    page.settle(None)?;
    watch(&mut page, identity(1)?)?;
    assert_eq!(drain(&mut page)?, (1, vec![none()]));
    shown(&mut page, identity(1)?, 1)?;
    assert!(!page.take_shown());

    let mut owing = SessionNoticePage::new();
    owing.settle(Some(&lost_workflow_changes()))?;
    shown(&mut owing, identity(1)?, 2)?;
    assert!(!owing.take_shown(), "another notice number settles nothing");
    Ok(())
}

#[test]
fn a_retired_navigation_receives_nothing_more() -> TestResult {
    let mut page = SessionNoticePage::new();
    page.settle(Some(&lost_workflow_changes()))?;
    watch(&mut page, identity(1)?)?;
    page.retire(identity(1)?);
    assert_eq!(drain(&mut page)?, (1, vec![]));
    Ok(())
}
