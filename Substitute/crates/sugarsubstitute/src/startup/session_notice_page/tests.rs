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
    watch_in(page, identity, SessionNoticeVersion::One)
}

fn watch_in(
    page: &mut SessionNoticePage,
    identity: MainIdentity,
    version: SessionNoticeVersion,
) -> TestResult {
    let session = PanelSession::for_main(identity)?;
    send(page, identity, SessionNoticeClientMessage::Watch { session, version })
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

fn saving() -> HostLine {
    HostLine { tone: NoticeTone::Info, text: "Saving your workspace…".into(), lasting: true }
}

fn line(notice: u64, text: &str, tone: &str, lasting: bool) -> Value {
    json!({"protocol": "sugarcubes.session-notice", "version": 2, "type": "line",
        "notice": notice, "tone": tone, "text": text, "lasting": lasting})
}

#[test]
fn a_version_two_page_is_sent_each_line_once_and_later_lines_as_they_arise() -> TestResult {
    let mut page = SessionNoticePage::new();
    page.settle(None)?;
    page.tell(saving())?;
    assert_eq!(drain(&mut page)?, (0, vec![]), "no page watches yet");
    watch_in(&mut page, identity(1)?, SessionNoticeVersion::Two)?;
    // A page that is owed a line is answered with it rather than with "none".
    let first = line(2, "Saving your workspace…", "info", true);
    assert_eq!(drain(&mut page)?, (1, vec![first.clone()]));
    page.tell(saving())?;
    assert_eq!(drain(&mut page)?, (0, vec![]), "the same words are owed once");
    let warning = HostLine { tone: NoticeTone::Warn, text: "Later".into(), lasting: false };
    page.tell(warning)?;
    assert_eq!(drain(&mut page)?, (0, vec![line(3, "Later", "warn", false)]));
    // A line stays owed, and is sent to the next navigation, until a page shows it.
    shown(&mut page, identity(1)?, 2)?;
    assert_eq!(drain(&mut page)?, (1, vec![]));
    page.retire(identity(1)?);
    watch_in(&mut page, identity(2)?, SessionNoticeVersion::Two)?;
    assert_eq!(drain(&mut page)?, (1, vec![line(3, "Later", "warn", false)]));
    shown(&mut page, identity(2)?, 3)?;
    watch_in(&mut page, identity(2)?, SessionNoticeVersion::Two)?;
    let none = json!({"protocol": "sugarcubes.session-notice", "version": 2, "type": "none"});
    assert_eq!(drain(&mut page)?, (2, vec![none]));
    Ok(())
}

#[test]
fn a_version_one_page_is_never_sent_a_line_and_the_line_stays_owed() -> TestResult {
    let mut page = SessionNoticePage::new();
    page.settle(None)?;
    page.tell(saving())?;
    watch(&mut page, identity(1)?)?;
    assert_eq!(drain(&mut page)?, (1, vec![none()]));
    assert_eq!(page.owed_lines(), vec![&saving()]);
    Ok(())
}

#[test]
fn unsaved_work_and_lines_are_both_told_to_a_version_two_page() -> TestResult {
    let mut page = SessionNoticePage::new();
    page.settle(Some(&lost_workflow_changes()))?;
    page.tell(saving())?;
    watch_in(&mut page, identity(1)?, SessionNoticeVersion::Two)?;
    let mut lost = not_saved(true, false);
    lost["version"] = json!(2);
    assert_eq!(drain(&mut page)?, (1, vec![lost, line(2, "Saving your workspace…", "info", true)]));
    Ok(())
}

#[test]
fn an_overlong_line_is_cut_to_what_the_page_admits_and_blank_ones_are_not_owed() -> TestResult {
    let mut page = SessionNoticePage::new();
    page.tell(HostLine { tone: NoticeTone::Info, text: "  ".into(), lasting: false })?;
    assert!(page.owed_lines().is_empty());
    page.tell(HostLine { tone: NoticeTone::Info, text: "é".repeat(900), lasting: false })?;
    assert_eq!(page.owed_lines()[0].text.chars().count(), MAX_LINE_CHARACTERS);
    for index in 0..MAX_LINES + 3 {
        page.tell(HostLine {
            tone: NoticeTone::Info,
            text: format!("line {index}"),
            lasting: false,
        })?;
    }
    assert_eq!(page.owed_lines().len(), MAX_LINES, "the oldest give way");
    Ok(())
}
