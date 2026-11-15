// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The SugarCubes update report's bytes and the words each outcome shows.
use super::*;
use crate::localization::pack_updates::EN;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn reports_round_trip_and_unknown_or_oversized_bytes_are_refused() -> TestResult {
    let report = PackUpdateReport {
        from: Some("1.0.0".into()),
        to: Some("1.1.0".into()),
        source: Some("registry".into()),
        ..PackUpdateReport::new(PackOutcome::Updated)
    };
    let bytes = report.render()?;
    assert_eq!(
        String::from_utf8(bytes.clone())?,
        r#"{"schema":1,"outcome":"updated","from":"1.0.0","to":"1.1.0","source":"registry"}"#
    );
    assert_eq!(PackUpdateReport::parse(&bytes)?, report);
    for invalid in [
        br#"{"schema":2,"outcome":"updated"}"#.to_vec(),
        br#"{"schema":1,"outcome":"exploded"}"#.to_vec(),
        br#"{"schema":1,"outcome":"updated","extra":1}"#.to_vec(),
        vec![b' '; MAX_PACK_REPORT_BYTES + 1],
    ] {
        assert_eq!(PackUpdateReport::parse(&invalid), Err(PackReportError));
    }
    Ok(())
}

#[test]
fn each_outcome_shows_at_most_one_plain_line_and_its_report_lines() {
    let with = |outcome, to: Option<&str>| PackUpdateReport {
        to: to.map(str::to_owned),
        ..PackUpdateReport::new(outcome)
    };
    assert_eq!(
        with(PackOutcome::Updated, Some("1.1.0")).notice(&EN).as_deref(),
        Some("SugarCubes updated to version 1.1.0")
    );
    assert_eq!(
        with(PackOutcome::Available, Some("1.2.0")).notice(&EN).as_deref(),
        Some("SugarCubes version 1.2.0 is available.")
    );
    assert_eq!(with(PackOutcome::Restored, None).notice(&EN).as_deref(), Some(EN.previous_running));
    let restored = PackUpdateReport {
        from: Some("1.1.0".into()),
        to: Some("1.0.0".into()),
        reason: Some("did-not-start".into()),
        ..PackUpdateReport::new(PackOutcome::Restored)
    };
    assert_eq!(
        restored.notice(&EN).as_deref(),
        Some("The new SugarCubes version didn't start, so the previous version was put back.")
    );
    for quiet in [PackOutcome::UpToDate, PackOutcome::Judging, PackOutcome::Busy] {
        assert_eq!(with(quiet, None).notice(&EN), None);
    }
    let failed = PackUpdateReport {
        step: Some("downloading".into()),
        unchecked: vec!["github-release".into()],
        ..PackUpdateReport::new(PackOutcome::Failed)
    };
    assert_eq!(failed.notice(&EN).as_deref(), Some(EN.could_not_update));
    assert_eq!(failed.lines(&EN), [EN.could_not_update, EN.steps[1], EN.github_unchecked]);
    let kept = PackUpdateReport {
        reason: Some("changed-files".into()),
        ..PackUpdateReport::new(PackOutcome::NotManaged)
    };
    assert_eq!(kept.lines(&EN), [EN.kept_edited]);
}

#[test]
fn the_splash_names_the_update_only_while_it_runs() {
    assert_eq!(PackUpdateStatus::Updating.splash_line(&EN), Some("Updating SugarCubes…"));
    assert_eq!(
        PackUpdateStatus::Restoring.splash_line(&EN),
        Some("Putting the previous SugarCubes version back…")
    );
    assert_eq!(PackUpdateStatus::Restoring.report(), None);
    assert_eq!(PackUpdateStatus::Idle.splash_line(&EN), None);
    let finished = PackUpdateStatus::Finished(PackUpdateReport::new(PackOutcome::CouldNotCheck));
    assert_eq!(finished.splash_line(&EN), None);
    assert_eq!(finished.notice(&EN).as_deref(), Some(EN.could_not_check));
}

#[test]
fn a_report_from_another_process_is_read_only_within_its_bounds() -> Result<(), PackReportError> {
    let report = PackUpdateReport {
        to: Some("1.1.0".into()),
        unchecked: vec!["registry".into(), "github-release".into()],
        ..PackUpdateReport::new(PackOutcome::Available)
    };
    assert!(report.valid());
    assert_eq!(PackUpdateReport::parse(&report.render()?)?, report);
    let long = PackUpdateReport { to: Some("9".repeat(65)), ..report.clone() };
    assert_eq!(PackUpdateReport::parse(&long.render()?), Err(PackReportError));
    let many = PackUpdateReport { unchecked: vec!["registry".into(); 3], ..report.clone() };
    assert!(!many.valid());
    let empty = PackUpdateReport { code: Some(String::new()), ..report };
    assert!(!empty.valid());
    Ok(())
}

#[test]
fn the_window_names_the_one_line_by_its_words_and_offers_a_report_only_for_problems() {
    let updated = PackUpdateReport {
        to: Some("1.1.0".into()),
        ..PackUpdateReport::new(PackOutcome::Updated)
    };
    assert_eq!(updated.notice_template(&EN), Some(EN.updated));
    assert!(!updated.needs_attention());
    assert_eq!(PackUpdateReport::new(PackOutcome::Updated).notice_template(&EN), None);
    for outcome in [
        PackOutcome::Restored,
        PackOutcome::CouldNotCheck,
        PackOutcome::Failed,
        PackOutcome::Stopped,
    ] {
        assert!(PackUpdateReport::new(outcome).needs_attention(), "{outcome:?}");
    }
    for outcome in [PackOutcome::UpToDate, PackOutcome::Judging, PackOutcome::NotManaged] {
        assert!(!PackUpdateReport::new(outcome).needs_attention(), "{outcome:?}");
    }
}
