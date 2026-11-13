// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The window's start-up update messages and the splash's "Updating to version X…" line come
//! from the coordinator's status as the child received it.
use super::*;
use sugarsubstitute_contracts::update::status::{LaunchVersion, StartupNotice, UpdateStatus};
use sugarsubstitute_desktop::{StartupWindowContent, UpdateNotice};
use sugarsubstitute_splash::{StartupStatus, UpdateVersion};

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
