// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The update report exports through the startup host's real execution owner and receiving
//! files, and every export closes its files before it reports an outcome.
use super::*;
use crate::{
    completion_wake::Wake,
    startup::{StartupHost, platform_tests::Platform},
};
use std::num::NonZeroUsize;
use sugarsubstitute_contracts::{
    execution::ScopeId,
    localization::messages,
    update::{
        attempt::{AttemptOutcome, AttemptPhase, UpdateReport},
        status::UpdateStatus,
    },
};
use sugarsubstitute_desktop::{
    StartupWindowContent, WindowContent, report::surface::ReportPresentation,
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn failure() -> UpdateReport {
    UpdateReport {
        version: Some("2.0.0".into()),
        phase: AttemptPhase::Downloading,
        causes: vec!["The download stopped".into(), "The connection was reset".into()],
    }
}

fn spec(id: u64) -> TestResult<ScopeSpec> {
    Ok(ScopeSpec { id: ScopeId::new(id)?, deadline: None })
}

/// A started host whose coordinator reported `failure`, with export scopes when `exports`.
fn started(
    failure: Option<UpdateReport>,
    exports: bool,
) -> TestResult<(StartupHost<Platform>, Arc<Wake>)> {
    let mut host = crate::startup::platform_tests::host()?;
    host.child.start_allowed = true;
    if exports {
        host.configure_update_reports(spec(3)?, spec(4)?)?;
    }
    if let Some(report) = failure {
        let mut status = UpdateStatus::default();
        status.begin();
        status.finish(Some(AttemptOutcome::Failed(report)), 1);
        host.child.update_status = Some(status);
    }
    let wake = Arc::new(Wake::default());
    host.start(wake.clone())?;
    Ok((host, wake))
}

/// Deliver completions the way the host's turn does until the writer reports an outcome.
fn outcome(
    host: &StartupHost<Platform>,
    wake: &Wake,
    writer: &mut dyn UpdateExport,
) -> TestResult<Result<(), Box<dyn Error>>> {
    let bound = NonZeroUsize::MIN.saturating_add(31);
    wake.until(|| match host.execution.drain(bound) {
        Err(cause) => Some(Err(cause)),
        Ok(_) => writer.finished().map(Ok),
    })
}

#[test]
fn an_exported_update_report_holds_exactly_the_text_the_window_shows() -> TestResult {
    let (mut host, wake) = started(Some(failure()), true)?;
    let report = shown_report(&host)?;
    let mut writer = host.update_report_export().ok_or("Export hidden")?;
    writer.start(wake.clone())?;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Update report.txt");
    writer.begin(path.clone(), &report)?;
    assert!(writer.begin(path.clone(), &report).is_err(), "one write at a time");
    assert!(!writer.settled());
    outcome(&host, &wake, &mut *writer)??;
    assert_eq!(std::fs::read_to_string(&path)?, report);
    assert!(report.contains("2.0.0") && report.contains("The connection was reset"));
    assert!(writer.settled());
    // The save dialog already confirmed replacing an existing file.
    std::fs::write(&path, "an older report")?;
    writer.begin(path.clone(), &report)?;
    outcome(&host, &wake, &mut *writer)??;
    assert_eq!(std::fs::read_to_string(&path)?, report);
    assert_eq!(std::fs::read_dir(directory.path())?.count(), 1, "no stage or backup remains");
    assert!(host.update_reports.as_ref().is_some_and(UpdateReports::settled));
    Ok(())
}

/// The full text of the update report the window shows for `host`'s status.
fn shown_report(host: &StartupHost<Platform>) -> TestResult<String> {
    let status = host.update_status().ok_or("no update status")?;
    let report = ReportPresentation::updates(messages("en"), "SugarSubstitute", &status);
    let details = report.as_ref().and_then(ReportPresentation::details);
    Ok(details.ok_or("no update report")?.to_owned())
}

/// A start whose only failure is its SugarCubes update offers Export too, and the file holds
/// the SugarCubes section the window shows.
#[test]
fn a_failed_sugarcubes_update_alone_exports_the_report_the_window_shows() -> TestResult {
    use sugarsubstitute_contracts::update::pack::{
        PackOutcome, PackUpdateReport, PackUpdateStatus,
    };
    let (mut host, wake) = started(None, true)?;
    assert!(host.update_report_export().is_none(), "nothing to report yet");
    let mut status = UpdateStatus::default();
    status.observe_sugarcubes(PackUpdateStatus::Finished(PackUpdateReport {
        to: Some("0.16.0".into()),
        step: Some("installing".into()),
        code: Some("digest-mismatch".into()),
        ..PackUpdateReport::new(PackOutcome::Failed)
    }));
    host.child.update_status = Some(status);
    let report = shown_report(&host)?;
    let mut writer = host.update_report_export().ok_or("Export hidden")?;
    writer.start(wake.clone())?;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Update report.txt");
    writer.begin(path.clone(), &report)?;
    outcome(&host, &wake, &mut *writer)??;
    assert_eq!(std::fs::read_to_string(&path)?, report);
    let words = messages("en");
    assert!(report.starts_with(&format!("{}\n", words.report.pack_update_title)), "{report}");
    assert!(report.contains("0.16.0") && report.contains("digest-mismatch"), "{report}");
    Ok(())
}

#[test]
fn a_destination_the_receiving_owner_refuses_fails_and_leaves_nothing_behind() -> TestResult {
    let (mut host, wake) = started(Some(failure()), true)?;
    let report = shown_report(&host)?;
    let mut writer = host.update_report_export().ok_or("Export hidden")?;
    writer.start(wake.clone())?;
    for refused in [r"report.txt", r"\\server\share\report.txt", r"C:\reports\con.txt"] {
        assert!(writer.begin(PathBuf::from(refused), &report).is_err(), "{refused}");
        assert!(writer.settled());
    }
    let directory = tempfile::tempdir()?;
    let missing = directory.path().join("missing").join("report.txt");
    writer.begin(missing.clone(), &report)?;
    assert!(outcome(&host, &wake, &mut *writer)?.is_err());
    assert!(!missing.exists());
    assert_eq!(std::fs::read_dir(directory.path())?.count(), 0);
    assert!(writer.settled());
    assert!(host.update_reports.as_ref().is_some_and(UpdateReports::settled));
    Ok(())
}

#[test]
fn a_closed_report_refuses_new_writes_and_its_outstanding_write_finishes() -> TestResult {
    let (mut host, wake) = started(Some(failure()), true)?;
    let report = shown_report(&host)?;
    let mut writer = host.update_report_export().ok_or("Export hidden")?;
    writer.start(wake.clone())?;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("report.txt");
    writer.begin(path.clone(), &report)?;
    writer.close();
    assert!(writer.begin(directory.path().join("other.txt"), &report).is_err());
    let bound = NonZeroUsize::MIN.saturating_add(31);
    wake.until(|| (host.execution.drain(bound).is_err() || writer.settled()).then_some(()));
    assert!(path.exists());
    assert!(!directory.path().join("other.txt").exists());
    assert!(host.update_reports.as_ref().is_some_and(UpdateReports::settled));
    Ok(())
}

#[test]
fn export_is_offered_only_for_a_failed_attempt_on_a_composed_running_host() -> TestResult {
    let (mut host, _wake) = started(None, true)?;
    assert!(host.update_report_export().is_none(), "no failed attempt to report");
    let (mut host, _wake) = started(Some(failure()), false)?;
    assert!(host.update_report_export().is_none(), "no export scopes composed");
    let (mut host, _wake) = started(Some(failure()), true)?;
    host.closing = true;
    assert!(host.update_report_export().is_none(), "the application is closing");
    let mut idle = crate::startup::platform_tests::host()?;
    idle.configure_update_reports(spec(3)?, spec(4)?)?;
    let mut status = UpdateStatus::default();
    status.finish(Some(AttemptOutcome::Failed(failure())), 1);
    idle.child.update_status = Some(status);
    assert!(idle.update_report_export().is_none(), "execution has not started");
    Ok(())
}

#[test]
fn export_scopes_must_be_distinct_unbounded_and_follow_the_startup_scopes() -> TestResult {
    let mut host = crate::startup::platform_tests::host()?;
    for (main, cleanup) in [
        (spec(3)?, spec(3)?),
        (spec(2)?, spec(4)?),
        (spec(3)?, spec(1)?),
        (
            ScopeSpec {
                id: ScopeId::new(3)?,
                deadline: Some(sugarsubstitute_contracts::execution::Moment::from_nanos(1)),
            },
            spec(4)?,
        ),
    ] {
        assert!(host.configure_update_reports(main, cleanup).is_err());
        assert!(host.update_reports.is_none());
    }
    host.configure_update_reports(spec(3)?, spec(4)?)?;
    assert!(host.configure_update_reports(spec(5)?, spec(6)?).is_err(), "configured once");
    Ok(())
}
