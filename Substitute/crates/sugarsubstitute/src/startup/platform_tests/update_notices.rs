// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The window's start-up update messages and the splash's "Updating SugarCubes…" and
//! "Updating to version X…" lines come from the coordinator's status as the child received it.
use super::*;
use sugarsubstitute_contracts::update::{
    pack::{PackOutcome, PackUpdateReport, PackUpdateStatus},
    status::{LaunchVersion, StartupNotice, UpdateStatus},
};
use sugarsubstitute_desktop::{StartupWindowContent, UpdateNotice};
use sugarsubstitute_splash::{StartupStatus, UpdateVersion};

fn could_not_check() -> PackUpdateReport {
    PackUpdateReport::new(PackOutcome::CouldNotCheck)
}

#[test]
fn every_start_up_message_reaches_the_window_once_in_its_own_form() -> BackendResult<()> {
    let mut host = host()?;
    host.child.notices.extend([
        StartupNotice::Updated("2.4.1".into()),
        StartupNotice::Repairing,
        StartupNotice::PreviousVersionRunning,
        StartupNotice::Sugarcubes(could_not_check()),
    ]);
    // A build without update configuration names no release notes, so no link is offered.
    assert_eq!(
        host.take_update_notice(),
        Some(UpdateNotice::Updated { version: "2.4.1".into(), changelog: None })
    );
    assert_eq!(host.take_update_notice(), Some(UpdateNotice::Repairing));
    assert_eq!(host.take_update_notice(), Some(UpdateNotice::PreviousVersionRunning));
    assert_eq!(host.take_update_notice(), Some(UpdateNotice::Sugarcubes(could_not_check())));
    assert_eq!(host.take_update_notice(), None);
    Ok(())
}

#[test]
fn the_splash_says_updating_only_while_the_new_version_is_unconfirmed() -> BackendResult<()> {
    let mut host = host()?;
    assert_eq!(window_content::updating(&host.child, StartupStatus::Loading), None);
    let mut status = UpdateStatus::default();
    status.observe_launch(LaunchVersion::Applying("2.4.1".into()));
    host.child.update_status = Some(status.clone());
    let updating = UpdateVersion::new("2.4.1").map(StartupStatus::Updating);
    for loading in
        [StartupStatus::Loading, StartupStatus::LoadingLong, StartupStatus::LoadingExtended]
    {
        assert_eq!(window_content::updating(&host.child, loading), updating);
    }
    for other in [StartupStatus::Settling, StartupStatus::RetryReady, StartupStatus::CleanupFailed]
    {
        assert_eq!(window_content::updating(&host.child, other), None, "{other:?} wins");
    }
    status.observe_launch(LaunchVersion::Updated("2.4.1".into()));
    host.child.update_status = Some(status);
    assert_eq!(window_content::updating(&host.child, StartupStatus::Loading), None);
    let mut unshowable = UpdateStatus::default();
    unshowable.observe_launch(LaunchVersion::Applying("9".repeat(40)));
    host.child.update_status = Some(unshowable);
    assert_eq!(window_content::updating(&host.child, StartupStatus::Loading), None);
    Ok(())
}

/// While the SugarCubes update holds the start the splash says so, ahead of a new version's
/// line; once it finished the version line returns, and failure lines always win.
#[test]
fn the_splash_says_updating_sugarcubes_while_its_update_holds_the_start() -> BackendResult<()> {
    let mut host = host()?;
    let mut status = UpdateStatus::default();
    status.observe_launch(LaunchVersion::Applying("2.4.1".into()));
    status.observe_sugarcubes(PackUpdateStatus::Updating);
    host.child.update_status = Some(status.clone());
    for loading in
        [StartupStatus::Loading, StartupStatus::LoadingLong, StartupStatus::LoadingExtended]
    {
        assert_eq!(
            window_content::updating(&host.child, loading),
            Some(StartupStatus::UpdatingSugarcubes)
        );
    }
    for other in [StartupStatus::Failed, StartupStatus::Settling, StartupStatus::CleanupFailed] {
        assert_eq!(window_content::updating(&host.child, other), None, "{other:?} wins");
    }
    status.observe_sugarcubes(PackUpdateStatus::Finished(could_not_check()));
    host.child.update_status = Some(status);
    assert_eq!(
        window_content::updating(&host.child, StartupStatus::Loading),
        UpdateVersion::new("2.4.1").map(StartupStatus::Updating)
    );
    Ok(())
}

/// While the coordinator restarts into an update prepared before the window opened, the splash
/// stays and the window is never revealed; when it withdraws the restart, the held step runs.
#[test]
fn the_window_stays_hidden_while_the_app_restarts_into_its_update() -> BackendResult<()> {
    use sugarsubstitute_contracts::startup::StartupEffect;
    use sugarsubstitute_desktop::DesktopStartupCommand;
    let mut host = host()?;
    let launch = host.launch_id();
    let mut status = UpdateStatus::default();
    status.observe_launch(LaunchVersion::Restarting("2.4.1".into()));
    host.child.update_status = Some(status.clone());
    assert_eq!(
        window_content::updating(&host.child, StartupStatus::Loading),
        UpdateVersion::new("2.4.1").map(StartupStatus::Updating),
        "the splash says which version it updates to"
    );
    host.route(super::policy::StartupRoute::Effect(StartupEffect::RetireSplash { launch }))?;
    assert!(host.commands.take().is_none(), "the splash stays");
    host.release_held_reveal()?;
    assert!(host.commands.take().is_none(), "still restarting");
    status.withdraw_restart();
    host.child.update_status = Some(status);
    host.release_held_reveal()?;
    assert!(
        matches!(host.commands.take(), Some(DesktopStartupCommand::RetireSplash(held)) if held == launch),
        "the withdrawn restart lets the window open"
    );
    assert!(host.held_reveal.is_none());
    Ok(())
}
