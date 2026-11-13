// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The window's start-up update messages and the splash's "Updating to version X…" line come
//! from the coordinator's status as the child received it.
use super::*;
use sugarsubstitute_contracts::update::status::{LaunchVersion, StartupNotice, UpdateStatus};
use sugarsubstitute_desktop::{StartupWindowContent, UpdateNotice};
use sugarsubstitute_splash::{StartupStatus, UpdateVersion};

fn en() -> &'static sugarsubstitute_contracts::localization::Messages {
    sugarsubstitute_contracts::localization::messages("en")
}

#[test]
fn every_start_up_message_reaches_the_window_once_in_its_own_form() -> BackendResult<()> {
    let mut host = host()?;
    host.child.notices.extend([
        StartupNotice::Updated("2.4.1".into()),
        StartupNotice::Repairing,
        StartupNotice::PreviousVersionRunning,
    ]);
    // A build without update configuration names no release notes, so no link is offered.
    assert_eq!(
        host.take_update_notice(),
        Some(UpdateNotice::Updated { version: "2.4.1".into(), changelog: None })
    );
    assert_eq!(host.take_update_notice(), Some(UpdateNotice::Repairing));
    assert_eq!(host.take_update_notice(), Some(UpdateNotice::PreviousVersionRunning));
    assert_eq!(host.take_update_notice(), None);
    Ok(())
}

#[test]
fn the_splash_says_updating_only_while_the_new_version_is_unconfirmed() -> BackendResult<()> {
    let mut host = host()?;
    assert_eq!(window_content::updating(&host.child, StartupStatus::Loading, en()), None);
    let mut status = UpdateStatus::default();
    status.observe_launch(LaunchVersion::Applying("2.4.1".into()));
    host.child.update_status = Some(status.clone());
    let updating = UpdateVersion::new("2.4.1").map(StartupStatus::Updating);
    for loading in
        [StartupStatus::Loading, StartupStatus::LoadingLong, StartupStatus::LoadingExtended]
    {
        assert_eq!(window_content::updating(&host.child, loading, en()), updating);
    }
    for other in [StartupStatus::Settling, StartupStatus::RetryReady, StartupStatus::CleanupFailed]
    {
        assert_eq!(window_content::updating(&host.child, other, en()), None, "{other:?} wins");
    }
    status.observe_launch(LaunchVersion::Updated("2.4.1".into()));
    host.child.update_status = Some(status);
    assert_eq!(window_content::updating(&host.child, StartupStatus::Loading, en()), None);
    let mut unshowable = UpdateStatus::default();
    unshowable.observe_launch(LaunchVersion::Applying("9".repeat(40)));
    host.child.update_status = Some(unshowable);
    assert_eq!(window_content::updating(&host.child, StartupStatus::Loading, en()), None);
    Ok(())
}

#[test]
fn a_host_step_holds_the_splash_line_after_the_applying_version() -> BackendResult<()> {
    let mut host = host()?;
    host.child.step = Some("Updating SugarCubes…");
    let step = Some(StartupStatus::Step("Updating SugarCubes…"));
    for loading in
        [StartupStatus::Loading, StartupStatus::LoadingLong, StartupStatus::LoadingExtended]
    {
        assert_eq!(window_content::updating(&host.child, loading, en()), step);
    }
    for other in [StartupStatus::Failed, StartupStatus::Settling, StartupStatus::CleanupFailed] {
        assert_eq!(window_content::updating(&host.child, other, en()), None, "{other:?} wins");
    }
    let mut status = UpdateStatus::default();
    status.observe_launch(LaunchVersion::Applying("2.4.1".into()));
    host.child.update_status = Some(status);
    assert_eq!(
        window_content::updating(&host.child, StartupStatus::Loading, en()),
        UpdateVersion::new("2.4.1").map(StartupStatus::Updating),
        "the application's own update line wins"
    );
    Ok(())
}

#[test]
fn a_host_steps_lines_follow_the_applications_own_messages_once_each() -> BackendResult<()> {
    let mut host = host()?;
    host.child.notices.push_back(StartupNotice::Repairing);
    host.child.step_notices.extend([
        ("SugarCubes update", "SugarCubes was updated to version 3.1.0.".to_owned()),
        ("SugarCubes update", "SugarCubes couldn't be updated.".to_owned()),
    ]);
    assert_eq!(host.take_update_notice(), Some(UpdateNotice::Repairing));
    for text in ["SugarCubes was updated to version 3.1.0.", "SugarCubes couldn't be updated."] {
        assert_eq!(
            host.take_update_notice(),
            Some(UpdateNotice::Line { label: "SugarCubes update", text: text.into() })
        );
    }
    assert_eq!(host.take_update_notice(), None);
    Ok(())
}

#[test]
fn a_failed_host_step_is_reported_only_while_the_applications_updater_has_no_failure()
-> BackendResult<()> {
    use sugarsubstitute_contracts::update::attempt::{
        AttemptOutcome, AttemptPhase, AttemptProgress, UpdateReport,
    };
    let step = UpdateReport {
        version: Some("3.1.0".into()),
        phase: AttemptPhase::Downloading,
        causes: vec!["SugarCubes couldn't be updated.".into()],
    };
    let mut host = host()?;
    assert_eq!(host.update_status(), None, "nothing to report");
    host.child.step_failure = Some(step.clone());
    assert_eq!(host.update_status().as_ref().and_then(|status| status.failure()), Some(&step));
    let mut status = UpdateStatus::default();
    status.begin();
    status.observe(AttemptProgress { phase: AttemptPhase::Checking, bytes: None });
    host.child.update_status = Some(status.clone());
    let shown = host.update_status().ok_or("status")?;
    assert_eq!(shown.failure(), None, "a running check keeps its progress");
    let own = UpdateReport {
        version: Some("2.4.1".into()),
        phase: AttemptPhase::Checking,
        causes: vec!["The update server could not be reached".into()],
    };
    status.finish(Some(AttemptOutcome::Failed(own.clone())), 5);
    host.child.update_status = Some(status);
    assert_eq!(host.update_status().as_ref().and_then(|status| status.failure()), Some(&own));
    Ok(())
}
