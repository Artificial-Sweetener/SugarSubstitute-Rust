// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The start-up update messages the page is told, and the splash's "Updating SugarCubes…" and
//! "Updating to version X…" lines come from the coordinator's status as the child received it.
use super::*;
use sugarsubstitute_contracts::update::{
    pack::{PackOutcome, PackUpdateReport, PackUpdateStatus},
    status::{LaunchVersion, UpdateStatus},
};
use sugarsubstitute_desktop::StartupWindowContent;
use sugarsubstitute_splash::{StartupStatus, UpdateVersion};

fn could_not_check() -> PackUpdateReport {
    PackUpdateReport::new(PackOutcome::CouldNotCheck)
}

#[cfg(windows)]
fn start_up_messages() -> [sugarsubstitute_contracts::update::status::StartupNotice; 4] {
    use sugarsubstitute_contracts::update::status::StartupNotice;
    [
        StartupNotice::Updated("2.4.1".into()),
        StartupNotice::Repairing,
        StartupNotice::PreviousVersionRunning,
        StartupNotice::Sugarcubes(could_not_check()),
    ]
}

#[cfg(windows)]
#[test]
fn every_start_up_message_is_owed_to_the_page_once() -> BackendResult<()> {
    let mut host = host()?;
    host.child.notices.extend(start_up_messages());
    // Before the window is up nothing is said, and nothing is lost.
    host.tell_host_lines(false)?;
    assert!(host.session_notice.owed_lines().is_empty());
    assert_eq!(host.child.notices.len(), 4);
    host.tell_host_lines(true)?;
    let words = host.locale_messages();
    let owed: Vec<String> =
        host.session_notice.owed_lines().into_iter().map(|line| line.text.clone()).collect();
    assert_eq!(owed.len(), 4);
    assert!(owed[0].contains("2.4.1"));
    assert_eq!(owed[1], words.updates.repairing);
    assert_eq!(owed[2], words.updates.previous_running);
    assert_eq!(Some(owed[3].clone()), could_not_check().notice(words.pack_updates));
    // A later turn has nothing more to say.
    host.tell_host_lines(true)?;
    assert_eq!(host.session_notice.owed_lines().len(), 4);
    Ok(())
}

#[cfg(windows)]
#[test]
fn the_release_notes_scope_is_unbounded_follows_the_startup_scopes_and_is_configured_once()
-> BackendResult<()> {
    use sugarsubstitute_contracts::execution::{Moment, ScopeId, ScopeSpec};
    let spec = |id, deadline| -> BackendResult<ScopeSpec> {
        Ok(ScopeSpec { id: ScopeId::new(id)?, deadline })
    };
    let mut host = host()?;
    for refused in [spec(1, None)?, spec(2, None)?, spec(3, Some(Moment::from_nanos(1)))?] {
        assert!(host.configure_release_notes(refused).is_err());
        assert!(host.release_notes_jobs.is_none());
    }
    host.configure_release_notes(spec(3, None)?)?;
    assert!(host.release_notes_jobs.as_ref().is_some_and(|jobs| jobs.settled()));
    assert!(host.configure_release_notes(spec(4, None)?).is_err(), "configured once");
    Ok(())
}
#[test]
fn the_splash_says_updating_only_while_the_new_version_is_unconfirmed() -> BackendResult<()> {
    let mut host = host()?;
    assert_eq!(window_content::updating(&host.child, StartupStatus::Loading, false), None);
    let mut status = UpdateStatus::default();
    status.observe_launch(LaunchVersion::Applying("2.4.1".into()));
    host.child.update_status = Some(status.clone());
    let updating = UpdateVersion::new("2.4.1").map(StartupStatus::Updating);
    for loading in
        [StartupStatus::Loading, StartupStatus::LoadingLong, StartupStatus::LoadingExtended]
    {
        assert_eq!(window_content::updating(&host.child, loading, false), updating);
    }
    for other in [StartupStatus::Settling, StartupStatus::RetryReady, StartupStatus::CleanupFailed]
    {
        assert_eq!(window_content::updating(&host.child, other, false), None, "{other:?} wins");
    }
    status.observe_launch(LaunchVersion::Updated("2.4.1".into()));
    host.child.update_status = Some(status);
    assert_eq!(window_content::updating(&host.child, StartupStatus::Loading, false), None);
    let mut unshowable = UpdateStatus::default();
    unshowable.observe_launch(LaunchVersion::Applying("9".repeat(40)));
    host.child.update_status = Some(unshowable);
    assert_eq!(window_content::updating(&host.child, StartupStatus::Loading, false), None);
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
            window_content::updating(&host.child, loading, false),
            Some(StartupStatus::UpdatingSugarcubes)
        );
    }
    for other in [StartupStatus::Failed, StartupStatus::Settling, StartupStatus::CleanupFailed] {
        assert_eq!(window_content::updating(&host.child, other, false), None, "{other:?} wins");
    }
    status.observe_sugarcubes(PackUpdateStatus::Finished(could_not_check()));
    host.child.update_status = Some(status);
    assert_eq!(
        window_content::updating(&host.child, StartupStatus::Loading, false),
        UpdateVersion::new("2.4.1").map(StartupStatus::Updating)
    );
    Ok(())
}

fn restarting() -> UpdateStatus {
    let mut status = UpdateStatus::default();
    status.observe_launch(LaunchVersion::Restarting("2.4.1".into()));
    status
}

/// A window that was never revealed, told the app restarts into an update prepared before it
/// opened, says which version it updates to and keeps its splash until the coordinator's own
/// notice covers the screen; it then closes itself to restart, through the same close as every
/// other restart; a close already under way is left alone.
#[test]
fn a_window_never_revealed_closes_itself_to_restart_into_the_update() -> BackendResult<()> {
    use sugarsubstitute_application::workspace_session::close_save::CloseReason;
    let mut host = host()?;
    host.child.update_status = Some(restarting());
    assert_eq!(
        window_content::updating(&host.child, StartupStatus::Loading, false),
        UpdateVersion::new("2.4.1").map(StartupStatus::Updating),
        "the splash says which version it updates to"
    );
    host.advance_restart_hold()?;
    assert!(!host.closing, "the splash stays until the coordinator's notice covers the screen");
    let mut covered = restarting();
    covered.observe_restart_cover(true);
    host.child.update_status = Some(covered);
    host.advance_restart_hold()?;
    assert!(host.closing, "the hidden window closes itself");
    assert_eq!(host.close_reason, CloseReason::RestartToApply);
    host.advance_restart_hold()?;
    assert_eq!(host.close_reason, CloseReason::RestartToApply, "one close, once");
    Ok(())
}

/// A window revealed before it applied the restarting status stays open and holds its next step
/// while the restart stands; once the coordinator withdraws it, the held step runs.
#[test]
fn a_window_revealed_before_the_restart_stays_open_until_the_restart_is_withdrawn()
-> BackendResult<()> {
    use sugarsubstitute_contracts::startup::StartupEffect;
    use sugarsubstitute_desktop::DesktopStartupCommand;
    let mut host = host()?;
    let launch = host.launch_id();
    let identity = sugarsubstitute_contracts::startup::MainIdentity {
        run: host.run_key(),
        launch,
        navigation: sugarsubstitute_contracts::execution::Generation::new(1)?,
    };
    host.commands.reveal_main(identity)?;
    assert!(host.commands.take().is_some(), "the reveal went to the window");
    host.child.update_status = Some(restarting());
    host.advance_restart_hold()?;
    assert!(!host.closing, "a seen window is never closed for the restart");
    host.route(super::policy::StartupRoute::Effect(StartupEffect::RetireSplash { launch }))?;
    assert!(host.commands.take().is_none(), "held while the restart stands");
    let mut withdrawn = restarting();
    withdrawn.withdraw_restart();
    host.child.update_status = Some(withdrawn);
    host.advance_restart_hold()?;
    assert!(
        matches!(host.commands.take(), Some(DesktopStartupCommand::RetireSplash(held)) if held == launch),
        "the withdrawn restart lets the held step run"
    );
    assert!(!host.closing);
    Ok(())
}

/// The coordinator's close request closes the window through its ordinary close, once.
#[test]
fn a_coordinator_close_request_closes_the_window_normally() -> BackendResult<()> {
    use sugarsubstitute_application::workspace_session::close_save::CloseReason;
    let mut host = host()?;
    host.advance_coordinator_close()?;
    assert!(!host.closing);
    host.child.close_requested_by_coordinator = true;
    host.advance_coordinator_close()?;
    assert!(host.closing);
    assert_eq!(host.close_reason, CloseReason::Close);
    host.advance_coordinator_close()?;
    assert_eq!(host.close_reason, CloseReason::Close, "one close");
    Ok(())
}

/// A session whose page reported `code` and whose cleanup settled, ready for retry.
fn failed_page(code: &str) -> BackendResult<StartupSession> {
    use sugarsubstitute_contracts::{
        configuration::{
            ConnectionConfiguration, LoopbackOrigin, Presentation, StartupConfiguration,
        },
        execution::Generation,
        startup::{LaunchId, ReadinessReceipt, StartupEvent, StartupObservation},
    };
    let launch = LaunchId::new(1)?;
    let origin = LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?;
    let mut session = StartupSession::new(
        launch,
        Generation::new(1)?,
        StartupConfiguration::new(
            ConnectionConfiguration::Attach(origin),
            sugarsubstitute_contracts::configuration::ProfileId::new("retry-line".into())?,
            Presentation::new("en", sugarsubstitute_contracts::configuration::Theme::Dark, true)?,
        ),
    );
    let receipt = ReadinessReceipt::parse(format!(r#"{{"schema_version":1,"launch_id":1,"navigation_generation":1,"outcome":{{"status":"failed","failure":{{"code":"{code}","detail":"fixture"}}}}}}"#).as_bytes())?;
    for observation in [
        StartupObservation::SplashPresented,
        StartupObservation::ConnectionReady(origin),
        StartupObservation::FrontendNavigationStarted(Generation::new(1)?),
        StartupObservation::FrontendReceipt(receipt),
        StartupObservation::ConnectionSettled,
    ] {
        session.observe(StartupEvent::new(launch, observation))?;
    }
    Ok(session)
}

/// When SugarCubes could not load, the retry line says so; after this start updated SugarCubes
/// it also says the retry starts the previous version. Any other failure keeps the ordinary
/// line, and lines other than the retry are left alone.
#[test]
fn the_retry_line_names_sugarcubes_when_it_could_not_load() -> BackendResult<()> {
    let mut host = host()?;
    let sugarcubes = failed_page("sugarcubes_unavailable")?;
    assert_eq!(
        window_content::sugarcubes_retry(&host.child, &sugarcubes, StartupStatus::RetryReady),
        Some(StartupStatus::SugarcubesRetryReady)
    );
    let mut status = UpdateStatus::default();
    status.observe_sugarcubes(PackUpdateStatus::Finished(PackUpdateReport {
        from: Some("0.15.3".into()),
        to: Some("0.15.7".into()),
        ..PackUpdateReport::new(PackOutcome::Updated)
    }));
    host.child.update_status = Some(status);
    for code in ["sugarcubes_unavailable", "sugarcubes_failed"] {
        assert_eq!(
            window_content::sugarcubes_retry(
                &host.child,
                &failed_page(code)?,
                StartupStatus::RetryReady
            ),
            Some(StartupStatus::SugarcubesRetryPrevious),
            "{code}"
        );
    }
    let other = failed_page("frontend_unavailable")?;
    assert_eq!(
        window_content::sugarcubes_retry(&host.child, &other, StartupStatus::RetryReady),
        None
    );
    for current in [StartupStatus::Settling, StartupStatus::CleanupFailed, StartupStatus::Loading] {
        assert_eq!(window_content::sugarcubes_retry(&host.child, &sugarcubes, current), None);
    }
    assert!(StartupStatus::SugarcubesRetryPrevious.offers_retry());
    assert!(StartupStatus::SugarcubesRetryReady.offers_retry());
    assert!(!StartupStatus::Settling.offers_retry());
    Ok(())
}

/// A SugarCubes that predates the application does not describe its release sources, so the
/// host cannot update it, and its page cannot load in this host. When such a start fails,
/// whatever the page reported, the retry line says SugarCubes is out of date instead of
/// "Connection failed". The same update failure with a start that did not fail, or another
/// update failure, changes nothing.
#[test]
fn the_retry_line_says_an_outdated_sugarcubes_is_out_of_date() -> BackendResult<()> {
    let mut host = host()?;
    let failed = |code: &str| PackUpdateReport {
        step: Some("checking".into()),
        code: Some(code.into()),
        ..PackUpdateReport::new(PackOutcome::Failed)
    };
    let mut status = UpdateStatus::default();
    status.observe_sugarcubes(PackUpdateStatus::Finished(failed("release-source")));
    host.child.update_status = Some(status);
    for code in ["frontend_unavailable", "sugarcubes_unavailable", "timed_out"] {
        assert_eq!(
            window_content::sugarcubes_retry(
                &host.child,
                &failed_page(code)?,
                StartupStatus::RetryReady
            ),
            Some(StartupStatus::SugarcubesOutdated),
            "{code}"
        );
    }
    assert!(StartupStatus::SugarcubesOutdated.offers_retry());
    let other = failed_page("frontend_unavailable")?;
    for current in [StartupStatus::Settling, StartupStatus::Loading, StartupStatus::Failed] {
        assert_eq!(window_content::sugarcubes_retry(&host.child, &other, current), None);
    }
    let mut status = UpdateStatus::default();
    status.observe_sugarcubes(PackUpdateStatus::Finished(failed("io")));
    host.child.update_status = Some(status);
    assert_eq!(
        window_content::sugarcubes_retry(&host.child, &other, StartupStatus::RetryReady),
        None,
        "another update failure says nothing about the pack's age"
    );
    Ok(())
}

/// While the previous SugarCubes version is put back the splash says so instead of offering
/// Retry, and the ordinary lines return once it finished.
#[test]
fn no_retry_is_offered_while_the_previous_sugarcubes_is_put_back() -> BackendResult<()> {
    let mut host = host()?;
    let mut status = UpdateStatus::default();
    status.observe_sugarcubes(PackUpdateStatus::Restoring);
    host.child.update_status = Some(status.clone());
    for current in [
        StartupStatus::RetryReady,
        StartupStatus::SugarcubesRetryReady,
        StartupStatus::SugarcubesRetryPrevious,
        StartupStatus::Loading,
        StartupStatus::LoadingLong,
        StartupStatus::UpdatingSugarcubes,
    ] {
        assert_eq!(
            window_content::restoring(&host.child, current),
            Some(StartupStatus::RestoringSugarcubes),
            "{current:?}"
        );
    }
    for current in [StartupStatus::Settling, StartupStatus::Failed] {
        assert_eq!(
            window_content::restoring(&host.child, current),
            Some(StartupStatus::RestoringSugarcubes),
            "cleanup in progress: {current:?}"
        );
    }
    assert_eq!(
        window_content::restoring(&host.child, StartupStatus::CleanupFailed),
        None,
        "a failed cleanup wins"
    );
    assert!(!StartupStatus::RestoringSugarcubes.offers_retry());
    status.observe_sugarcubes(PackUpdateStatus::Finished(could_not_check()));
    host.child.update_status = Some(status);
    assert_eq!(window_content::restoring(&host.child, StartupStatus::RetryReady), None);
    Ok(())
}

/// A retry starts the version this launch already updated to, so the splash no longer says it
/// is updating to it; the SugarCubes lines still show, and a restart into an update keeps its
/// line.
#[test]
fn a_retry_drops_the_app_update_line_that_was_already_applied() -> BackendResult<()> {
    let mut host = host()?;
    let mut status = UpdateStatus::default();
    status.observe_launch(LaunchVersion::Applying("0.5.0".into()));
    host.child.update_status = Some(status.clone());
    assert_eq!(
        window_content::updating(&host.child, StartupStatus::Loading, false),
        UpdateVersion::new("0.5.0").map(StartupStatus::Updating)
    );
    assert_eq!(window_content::updating(&host.child, StartupStatus::Loading, true), None);
    status.observe_sugarcubes(PackUpdateStatus::Updating);
    host.child.update_status = Some(status);
    assert_eq!(
        window_content::updating(&host.child, StartupStatus::Loading, true),
        Some(StartupStatus::UpdatingSugarcubes)
    );
    host.child.update_status = Some(restarting());
    assert_eq!(
        window_content::updating(&host.child, StartupStatus::Loading, true),
        UpdateVersion::new("2.4.1").map(StartupStatus::Updating)
    );
    Ok(())
}

/// The user's own close, "Cancel loading" on the splash or the window's close button, arrives as
/// a close request from the window. While SugarCubes files are installed it waits like every
/// other close: the window stays open and says "Finishing the SugarCubes update…", and it
/// closes once the coordinator says the install ended.
#[test]
fn the_users_close_during_a_sugarcubes_install_keeps_the_window_until_it_finishes()
-> BackendResult<()> {
    use sugarsubstitute_application::workspace_session::close_save::CloseReason;
    let mut host = host()?;
    let mut installing = UpdateStatus::default();
    installing.observe_sugarcubes(PackUpdateStatus::Updating);
    host.child.update_status = Some(installing.clone());
    host.observe(DesktopStartupObservation::CloseRequested)?;
    assert!(!host.closing, "the user's close waits for the install");
    assert_eq!(host.deferred_close, Some(CloseReason::Close));
    host.observe(DesktopStartupObservation::CloseRequested)?;
    assert!(!host.closing, "pressing close again forces nothing");
    installing.observe_sugarcubes(PackUpdateStatus::Finished(could_not_check()));
    host.child.update_status = Some(installing);
    host.advance_deferred_close()?;
    assert!(host.closing, "the install ended: the close begins");

    let mut idle = super::host()?;
    idle.observe(DesktopStartupObservation::CloseRequested)?;
    assert!(idle.closing, "with nothing installing the user's close begins at once");
    Ok(())
}

/// A start that failed and that the coordinator has not settled yet.
fn failed_session(
    launch: sugarsubstitute_contracts::startup::LaunchId,
) -> BackendResult<StartupSession> {
    let mut session = StartupSession::new(
        launch,
        Generation::new(1)?,
        StartupConfiguration::new(
            ConnectionConfiguration::Attach(LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?),
            ProfileId::new("failed-start-close-test".into())?,
            Presentation::new("en", Theme::Dark, true)?,
        ),
    );
    session.observe(StartupEvent::new(launch, StartupObservation::SplashPresented))?;
    session.observe(StartupEvent::new(
        launch,
        StartupObservation::Failed(StartupFailure::new(
            StartupFailureCode::ProcessExited,
            Box::new(std::io::Error::other("the backend did not start")),
        )),
    ))?;
    assert_eq!(session.phase(), StartupPhase::Failed);
    Ok(session)
}

/// A close pressed in the moment between a start failing and the coordinator settling it
/// waits for the settlement, with no SugarCubes install or restore running: a window that left
/// in that moment was reported as a failed start. A second request forces nothing, and a
/// session end, which the system will not wait for, still closes at once.
#[test]
fn a_close_before_a_failed_start_settled_waits_for_it_with_nothing_installing() -> BackendResult<()>
{
    use sugarsubstitute_application::workspace_session::close_save::CloseReason;
    let mut host = host()?;
    let launch = host.launch_id();
    host.startup = Some(Prepared { session: failed_session(launch)?, released: false });
    for _ in 0..2 {
        host.observe(DesktopStartupObservation::CloseRequested)?;
        assert!(!host.closing, "the failed start is not settled yet");
        assert_eq!(host.deferred_close, Some(CloseReason::Close));
    }
    host.advance_deferred_close()?;
    assert!(!host.closing);
    let mut prepared = host.startup.take().ok_or("Startup absent")?;
    prepared.session.observe(StartupEvent::new(launch, StartupObservation::ConnectionSettled))?;
    host.startup = Some(prepared);
    host.advance_deferred_close()?;
    assert!(host.closing, "the failed start settled: the close begins");
    assert_eq!(host.child.close_phase, Some(StartupPhase::Stopped));

    let mut ending = super::host()?;
    let launch = ending.launch_id();
    ending.startup = Some(Prepared { session: failed_session(launch)?, released: false });
    ending.request_close(CloseReason::SessionEnding { critical: false })?;
    assert!(ending.closing, "a session end does not wait");

    // A start whose cleanup failed never settles; its close is not held for it.
    let mut stuck = super::host()?;
    let launch = stuck.launch_id();
    stuck.startup = Some(Prepared { session: failed_session(launch)?, released: false });
    stuck.child.cleanup_failed = true;
    stuck.request_close(CloseReason::Close)?;
    assert!(stuck.closing, "nothing will settle this start");
    Ok(())
}
/// A close during a restore waits for the failed start to settle as well. The previous files
/// are back a moment before the coordinator has retired the start that failed; a window that
/// left in between was reported as a failed start.
#[test]
fn a_close_during_a_restore_also_waits_for_the_failed_start_to_settle() -> BackendResult<()> {
    let mut host = host()?;
    let launch = host.launch_id();
    host.startup = Some(Prepared { session: failed_session(launch)?, released: false });
    let mut restoring = UpdateStatus::default();
    restoring.observe_sugarcubes(PackUpdateStatus::Restoring);
    host.child.update_status = Some(restoring.clone());
    host.observe(DesktopStartupObservation::CloseRequested)?;
    assert!(!host.closing, "the close waits for the restore");
    restoring.observe_sugarcubes(PackUpdateStatus::Finished(could_not_check()));
    host.child.update_status = Some(restoring);
    host.advance_deferred_close()?;
    assert!(!host.closing, "the previous files are back, the failed start is not settled yet");
    let mut prepared = host.startup.take().ok_or("Startup absent")?;
    prepared.session.observe(StartupEvent::new(launch, StartupObservation::ConnectionSettled))?;
    assert_eq!(prepared.session.phase(), StartupPhase::Stopped);
    host.startup = Some(prepared);
    host.advance_deferred_close()?;
    assert!(host.closing, "the failed start settled: the close begins");
    assert_eq!(
        host.child.close_phase,
        Some(StartupPhase::Stopped),
        "the child classifies the exit by the settled start, not by the phase it last sampled"
    );
    Ok(())
}

/// A close asked for while SugarCubes files are installed or put back waits for that to end:
/// the splash says "Finishing the SugarCubes update…", a second close request forces nothing,
/// and the ordinary close begins once the coordinator says the install ended. A session end
/// still closes at once.
#[test]
fn a_close_during_a_sugarcubes_install_waits_for_it_to_finish() -> BackendResult<()> {
    use sugarsubstitute_application::workspace_session::close_save::CloseReason;
    let mut host = host()?;
    let mut installing = UpdateStatus::default();
    installing.observe_sugarcubes(PackUpdateStatus::Updating);
    host.child.update_status = Some(installing.clone());
    host.request_close(CloseReason::Close)?;
    assert!(!host.closing, "the close waits for the install");
    assert_eq!(host.deferred_close, Some(CloseReason::Close));
    host.request_close(CloseReason::Close)?;
    assert!(!host.closing, "a second request forces nothing");
    host.advance_deferred_close()?;
    assert!(!host.closing, "still installing");
    installing.observe_sugarcubes(PackUpdateStatus::Finished(could_not_check()));
    host.child.update_status = Some(installing);
    host.advance_deferred_close()?;
    assert!(host.closing, "the install ended: the ordinary close begins");
    assert_eq!(host.deferred_close, None);

    let mut ending = super::host()?;
    let mut restoring = UpdateStatus::default();
    restoring.observe_sugarcubes(PackUpdateStatus::Restoring);
    ending.child.update_status = Some(restoring);
    ending.request_close(CloseReason::SessionEnding { critical: false })?;
    assert!(ending.closing, "a session end does not wait");
    Ok(())
}
