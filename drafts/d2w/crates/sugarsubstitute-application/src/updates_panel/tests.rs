// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove the updates answers against a fake updater.

use super::*;
use std::error::Error;
use sugarsubstitute_contracts::{
    host_updates::UpdatesRequestName,
    panel::PanelSession,
    update::{
        attempt::{AttemptOutcome, AttemptProgress},
        status::RecordedUpdates,
    },
};

type TestResult = Result<(), Box<dyn Error>>;

/// A fake updater: its facts are set by the test, and it records what the page asked for.
struct FakeUpdater {
    facts: Option<UpdateFacts>,
    check: Result<(), UpdatesRejection>,
    restart: Result<(), UpdatesRejection>,
    checks: usize,
    restarts: Vec<UpdatesRestartTiming>,
}

impl FakeUpdater {
    fn new() -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            facts: Some(UpdateFacts {
                current_version: UpdatesVersion::new("0.4.0")?,
                channel: UpdatesChannel::Beta,
                status: UpdateStatus::default(),
                workflow_running: false,
                restart_after_workflow: false,
            }),
            check: Ok(()),
            restart: Ok(()),
            checks: 0,
            restarts: Vec::new(),
        })
    }

    fn status(&mut self) -> Result<&mut UpdateStatus, Box<dyn Error>> {
        Ok(&mut self.facts.as_mut().ok_or("Facts missing")?.status)
    }
}

impl UpdateControls for FakeUpdater {
    fn facts(&self) -> Option<UpdateFacts> {
        self.facts.clone()
    }
    fn check_now(&mut self) -> Result<(), UpdatesRejection> {
        self.checks += 1;
        if self.check.is_ok()
            && let Some(facts) = &mut self.facts
        {
            facts.status.begin();
        }
        self.check
    }
    fn restart(&mut self, when: UpdatesRestartTiming) -> Result<(), UpdatesRejection> {
        self.restarts.push(when);
        if self.restart.is_ok()
            && when == UpdatesRestartTiming::AfterWorkflow
            && let Some(facts) = &mut self.facts
        {
            facts.restart_after_workflow = true;
        }
        self.restart
    }
}

fn session() -> Result<PanelSession, Box<dyn Error>> {
    Ok(PanelSession::new("session-12345678".into())?)
}

fn watch() -> Result<UpdatesClientMessage, Box<dyn Error>> {
    Ok(UpdatesClientMessage::Watch(session()?))
}

fn request(name: &str) -> Result<UpdatesRequestName, Box<dyn Error>> {
    Ok(UpdatesRequestName::new(name)?)
}

fn revision(message: &UpdatesHostMessage) -> Option<u64> {
    match message {
        UpdatesHostMessage::State { revision, .. } => Some(*revision),
        _ => None,
    }
}

fn status_of(message: &UpdatesHostMessage) -> Option<&UpdatesStatus> {
    match message {
        UpdatesHostMessage::State { update, .. } => Some(&update.status),
        _ => None,
    }
}

#[test]
fn watch_is_answered_with_a_state_and_changes_raise_the_revision() -> TestResult {
    let mut updater = FakeUpdater::new()?;
    let mut panel = UpdatesPanelSession::default();
    assert_eq!(panel.refresh(&updater), None);
    let answers = panel.receive(watch()?, &mut updater);
    assert_eq!(answers.len(), 1);
    assert_eq!(revision(&answers[0]), Some(1));
    assert_eq!(status_of(&answers[0]), Some(&UpdatesStatus::UpToDate));
    assert_eq!(panel.refresh(&updater), None);
    updater.status()?.reconcile(RecordedUpdates {
        prepared: Some("0.5.0".into()),
        previous_version_running: false,
    });
    let changed = panel.refresh(&updater).ok_or("Changed state was not sent")?;
    assert_eq!(revision(&changed), Some(2));
    assert_eq!(status_of(&changed), Some(&UpdatesStatus::Ready(UpdatesVersion::new("0.5.0")?)));
    let facts = updater.facts.as_mut().ok_or("Facts missing")?;
    facts.workflow_running = true;
    let running = panel.refresh(&updater).ok_or("Workflow change was not sent")?;
    let UpdatesHostMessage::State { revision: 3, update } = running else {
        return Err("Expected the third state".into());
    };
    assert!(update.workflow_running);
    Ok(())
}

#[test]
fn a_watch_before_the_host_has_facts_is_answered_once_they_arrive() -> TestResult {
    let mut updater = FakeUpdater::new()?;
    let facts = updater.facts.take();
    let mut panel = UpdatesPanelSession::default();
    assert!(panel.receive(watch()?, &mut updater).is_empty());
    updater.facts = facts;
    assert_eq!(panel.refresh(&updater).as_ref().and_then(revision), Some(1));
    Ok(())
}

#[test]
fn a_repeated_watch_is_answered_again_with_a_newer_revision() -> TestResult {
    let mut updater = FakeUpdater::new()?;
    let mut panel = UpdatesPanelSession::default();
    panel.receive(watch()?, &mut updater);
    let again = panel.receive(watch()?, &mut updater);
    assert_eq!(again.iter().filter_map(revision).collect::<Vec<_>>(), [2]);
    Ok(())
}

#[test]
fn every_check_and_restart_is_answered_by_name() -> TestResult {
    let mut updater = FakeUpdater::new()?;
    let mut panel = UpdatesPanelSession::default();
    panel.receive(watch()?, &mut updater);
    let check = UpdatesClientMessage::Check { session: session()?, request: request("check-1")? };
    let answers = panel.receive(check, &mut updater);
    assert_eq!(answers[0], UpdatesHostMessage::Accepted(request("check-1")?));
    assert_eq!(status_of(&answers[1]), Some(&UpdatesStatus::Checking));
    assert_eq!(updater.checks, 1);

    updater.check = Err(UpdatesRejection::Busy);
    let check = UpdatesClientMessage::Check { session: session()?, request: request("check-2")? };
    assert_eq!(
        panel.receive(check, &mut updater),
        [UpdatesHostMessage::Rejected {
            request: request("check-2")?,
            reason: UpdatesRejection::Busy
        }]
    );

    updater.restart = Err(UpdatesRejection::NotReady);
    let restart = UpdatesClientMessage::Restart {
        session: session()?,
        request: request("restart-1")?,
        when: UpdatesRestartTiming::Now,
    };
    assert_eq!(
        panel.receive(restart, &mut updater),
        [UpdatesHostMessage::Rejected {
            request: request("restart-1")?,
            reason: UpdatesRejection::NotReady
        }]
    );
    updater.restart = Ok(());
    let restart = UpdatesClientMessage::Restart {
        session: session()?,
        request: request("restart-2")?,
        when: UpdatesRestartTiming::AfterWorkflow,
    };
    let answers = panel.receive(restart, &mut updater);
    assert_eq!(answers[0], UpdatesHostMessage::Accepted(request("restart-2")?));
    let UpdatesHostMessage::State { update, .. } = &answers[1] else {
        return Err("The scheduled restart was not shown".into());
    };
    assert_eq!(update.scheduled_restart, UpdatesScheduledRestart::AfterWorkflow);
    assert_eq!(updater.restarts, [UpdatesRestartTiming::Now, UpdatesRestartTiming::AfterWorkflow]);
    Ok(())
}

#[test]
fn requests_before_watch_are_answered_without_a_state() -> TestResult {
    let mut updater = FakeUpdater::new()?;
    let mut panel = UpdatesPanelSession::default();
    let check = UpdatesClientMessage::Check { session: session()?, request: request("early")? };
    assert_eq!(panel.receive(check, &mut updater), [UpdatesHostMessage::Accepted(request("early")?)]);
    Ok(())
}

#[test]
fn the_page_state_follows_the_updater_status() -> TestResult {
    let mut updater = FakeUpdater::new()?;
    let facts = |updater: &FakeUpdater| updater.facts.clone().ok_or("Facts missing");
    updater.status()?.begin();
    updater.status()?.observe(AttemptProgress { phase: AttemptPhase::Downloading, bytes: None });
    assert_eq!(page_state(&facts(&updater)?).status, UpdatesStatus::Checking);

    let report = UpdateReport {
        version: Some("0.5.0".into()),
        phase: AttemptPhase::Downloading,
        causes: (0..12).map(|index| format!("Cause {index}")).collect(),
    };
    updater.status()?.finish(Some(AttemptOutcome::Failed(report)), 7);
    let failed = page_state(&facts(&updater)?);
    assert_eq!(failed.status, UpdatesStatus::Failed(UpdatesFailure::Update));
    assert_eq!(failed.last_successful_check_ms, None);
    let shown = failed.report.ok_or("Report missing")?;
    assert_eq!(
        shown,
        UpdatesReport::new(
            UpdatesVersion::new("0.5.0")?,
            UpdatesPhase::Download,
            (0..8).map(|index| format!("Cause {index}")).collect(),
        )?
    );

    updater.status()?.begin();
    updater.status()?.finish(None, 8);
    let unreported = page_state(&facts(&updater)?);
    assert_eq!(unreported.status, UpdatesStatus::Failed(UpdatesFailure::Check));
    assert_eq!(unreported.report, None);

    updater.status()?.begin();
    updater.status()?.finish(Some(AttemptOutcome::Current), 9);
    updater.status()?.reconcile(RecordedUpdates {
        prepared: None,
        previous_version_running: true,
    });
    let current = page_state(&facts(&updater)?);
    assert_eq!(current.status, UpdatesStatus::UpToDate);
    assert_eq!(current.last_successful_check_ms, Some(9000));
    assert!(current.previous_version_running);
    assert_eq!(current.report, None);
    Ok(())
}
