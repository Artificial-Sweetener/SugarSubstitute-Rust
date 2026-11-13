// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Update failure reports: Copy and Export without Restart, export outcomes, and report text for
//! application and SugarCubes failures, with nothing private in what is copied or exported.
use super::*;
use sugarsubstitute_contracts::update::{
    attempt::{AttemptPhase, UpdateReport as Attempt},
    pack_failure::PackUpdateFailure,
};

/// A fake export that records what it was asked to write and finishes when told to.
#[derive(Default)]
struct Exports {
    begun: Vec<(PathBuf, String)>,
    outcome: Option<std::result::Result<(), Box<dyn Error>>>,
    closed: bool,
}
impl UpdateExport for Exports {
    fn start(&mut self, _: Arc<dyn CompletionWake>) -> TestResult {
        Ok(())
    }
    fn begin(&mut self, destination: PathBuf, report: &str) -> TestResult {
        self.begun.push((destination, report.to_owned()));
        Ok(())
    }
    fn finished(&mut self) -> Option<std::result::Result<(), Box<dyn Error>>> {
        self.outcome.take()
    }
    fn close(&mut self) {
        self.closed = true;
    }
    fn settled(&mut self) -> bool {
        true
    }
}

#[test]
fn update_failure_report_offers_copy_and_export_without_restart() -> TestResult {
    let copy = messages("en");
    let mut exports = Exports::default();
    let presentation = ReportPresentation::update_failure(
        copy,
        "SugarSubstitute",
        "0.5.0",
        "Downloading",
        "update 0.5.0 failed while downloading\ncaused by: connection reset\n".into(),
    );
    assert_eq!(presentation.content.kind, sugarsubstitute_ui::report::ReportKind::UpdateFailure);
    let shown_text = presentation.details.clone().ok_or("details")?;
    {
        let mut owner = UpdateReport::new(Some(&mut exports), &presentation);
        let mut lifetime = ReportLifetime::new(presentation, &mut owner)?;
        assert!(lifetime.inventory().iter().all(|c| c.id != ReportAction::Restart.id()));
        started(&mut lifetime)?;
        next_frame(&mut lifetime, Duration::ZERO)?;
        let shown = visible(&lifetime);
        for action in [ReportAction::Copy, ReportAction::Export, ReportAction::Close] {
            assert!(shown.contains(&action), "{action:?}");
        }
        invoke(&mut lifetime, ReportAction::Export)?;
        let picker = lifetime.picker().ok_or("export asks for a destination")?;
        assert!(matches!(picker.kind, crate::SelectionKind::Save { .. }));
        lifetime.input(Event::Picked(picker, Ok(Some(r"D:\reports\update.txt".into()))))?;
        next_frame(&mut lifetime, Duration::ZERO)?;
        assert!(!visible(&lifetime).is_empty());
        invoke(&mut lifetime, ReportAction::Copy)?;
        let copied = lifetime.clipboard().map(|copy| copy.text);
        assert_eq!(copied.as_deref(), Some(shown_text.as_str()), "Copy takes the whole report");
    }
    assert_eq!(
        exports.begun,
        vec![(PathBuf::from(r"D:\reports\update.txt"), shown_text)],
        "Export writes exactly what Copy copies"
    );
    Ok(())
}

#[test]
fn an_update_report_without_a_writer_keeps_copy_and_hides_export() -> TestResult {
    let copy = messages("en");
    let presentation = ReportPresentation::update_failure(
        copy,
        "SugarSubstitute",
        "0.5.0",
        "Downloading",
        "update 0.5.0 failed while downloading
"
        .into(),
    );
    let mut owner = UpdateReport::<Exports>::new(None, &presentation);
    let mut lifetime = ReportLifetime::new(presentation, &mut owner)?;
    started(&mut lifetime)?;
    next_frame(&mut lifetime, Duration::ZERO)?;
    let shown = visible(&lifetime);
    assert!(shown.contains(&ReportAction::Copy) && shown.contains(&ReportAction::Close));
    assert!(!shown.contains(&ReportAction::Export));
    assert!(!shown.contains(&ReportAction::Restart));
    Ok(())
}

#[test]
fn update_export_outcome_becomes_the_footer_status() -> TestResult {
    let copy = messages("en");
    let mut exports = Exports { outcome: Some(Err("disk full".into())), ..Exports::default() };
    let presentation =
        ReportPresentation::update_failure(copy, "SugarSubstitute", "1", "x", "".into());
    let mut owner = UpdateReport::new(Some(&mut exports), &presentation);
    let (mut message, mut state, mut closed, mut changed) = (
        String::new(),
        ReportState::new(sugarsubstitute_ui::report::ReportKind::UpdateFailure),
        false,
        false,
    );
    let mut control = ReportControl {
        message: &mut message,
        state: &mut state,
        closed: &mut closed,
        changed: &mut changed,
        requests: &mut ControlRequests::default(),
    };
    owner.command(
        ReportCommand::Export { destination: "a.txt".into(), include_dump: false },
        &mut control,
    )?;
    owner.poll(&mut control)?;
    assert_eq!(
        control.state.status,
        Some(ReportStatus { text: copy.report.export_failed.into(), error: true })
    );
    owner.command(ReportCommand::Action(ReportAction::Close), &mut control)?;
    assert!(*control.closed);
    Ok(())
}

fn attempt() -> Attempt {
    Attempt {
        version: Some("2.0.0".into()),
        phase: AttemptPhase::Downloading,
        causes: vec!["The download stopped".into(), "The connection was reset".into()],
    }
}

fn pack() -> PackUpdateFailure {
    PackUpdateFailure::new(
        Some("0.16.0"),
        Some("Downloading the update"),
        ["SugarCubes couldn't be updated.", "The registry couldn't be checked."],
        ["digest-mismatch"],
    )
}

#[test]
fn an_update_attempt_report_names_its_version_step_and_causes_in_the_users_language() -> TestResult
{
    let en = messages("en");
    let report = ReportPresentation::failed_updates(en, "SugarSubstitute", Some(&attempt()), None)
        .ok_or("report")?;
    assert_eq!(
        report.details.as_deref(),
        Some(
            "Update failed\nVersion: 2.0.0\nStep: Downloading the update\n\n\
             The download stopped\nThe connection was reset\n"
        )
    );
    assert_eq!(report.content.title, en.report.update_title);
    assert_eq!(report.content.summary[1].value, "Downloading the update");
    let unknown = Attempt { version: None, ..attempt() };
    let es = messages("es");
    let report = ReportPresentation::failed_updates(es, "SugarSubstitute", Some(&unknown), None)
        .ok_or("report")?;
    let text = report.details.unwrap_or_default();
    assert!(text.contains(&format!("Versión: {}", es.report.unavailable)));
    assert!(text.contains("Descargando la actualización"));
    assert_eq!(report.content.summary[0].value, es.report.unavailable);
    Ok(())
}

#[test]
fn a_failed_sugarcubes_update_has_its_own_report_in_the_users_language() -> TestResult {
    let en = messages("en");
    let report = ReportPresentation::failed_updates(en, "SugarSubstitute", None, Some(&pack()))
        .ok_or("report")?;
    assert_eq!(report.content.kind, sugarsubstitute_ui::report::ReportKind::UpdateFailure);
    assert_eq!(report.content.title, "SugarCubes update failed");
    assert_eq!(report.content.message, en.report.pack_update_message);
    assert_eq!(report.content.summary[0].value, "0.16.0");
    assert_eq!(report.content.summary[1].value, "Downloading the update");
    assert_eq!(
        report.details.as_deref(),
        Some(
            "SugarCubes update failed\nVersion: 0.16.0\nStep: Downloading the update\n\n\
             SugarCubes couldn't be updated.\nThe registry couldn't be checked.\ndigest-mismatch\n"
        )
    );
    let ja = messages("ja");
    let report = ReportPresentation::failed_updates(
        ja,
        "SugarSubstitute",
        None,
        Some(&PackUpdateFailure::new(None, None, ["x"], [""; 0])),
    )
    .ok_or("report")?;
    assert_eq!(report.content.title, ja.report.pack_update_title);
    assert_eq!(report.content.summary[0].value, ja.report.unavailable);
    assert_eq!(report.content.summary[1].value, ja.report.unavailable);
    Ok(())
}

#[test]
fn both_failures_share_one_report_with_the_application_first() -> TestResult {
    let en = messages("en");
    let report =
        ReportPresentation::failed_updates(en, "SugarSubstitute", Some(&attempt()), Some(&pack()))
            .ok_or("report")?;
    assert_eq!(report.content.title, en.report.update_title);
    assert_eq!(report.content.summary[0].value, "2.0.0");
    let text = report.details.ok_or("details")?;
    let app = text.find("Update failed\n").ok_or("application section")?;
    let pack = text.find("\n\nSugarCubes update failed\nVersion: 0.16.0").ok_or("pack section")?;
    assert!(app < pack);
    Ok(())
}

#[test]
fn nothing_failed_means_no_report() {
    assert!(
        ReportPresentation::failed_updates(messages("en"), "SugarSubstitute", None, None).is_none()
    );
}

#[test]
fn a_report_from_any_source_shows_and_exports_no_path_address_or_credential() -> TestResult {
    let copy = messages("en");
    // Built directly, not through `from_error`, as a source that skipped redaction would.
    let leaky = Attempt {
        version: Some("2.0.0".into()),
        phase: AttemptPhase::Preparing,
        causes: vec![
            r#"Cannot stage "C:\Users\Jo Ann\SugarSubstitute\releases\2""#.into(),
            "proxy 10.0.0.7:3128 sent Authorization: Bearer abc".into(),
        ],
    };
    let pack = PackUpdateFailure::new(
        Some("0.16.0"),
        None,
        [r"Kept E:\ComfyUI\custom_nodes\SugarCubes"],
        ["https://api.comfy.org/x?token=abc"],
    );
    let presentation =
        ReportPresentation::failed_updates(copy, "SugarSubstitute", Some(&leaky), Some(&pack))
            .ok_or("report")?;
    let mut exports = Exports::default();
    {
        let mut owner = UpdateReport::new(Some(&mut exports), &presentation);
        let mut lifetime = ReportLifetime::new(presentation, &mut owner)?;
        started(&mut lifetime)?;
        next_frame(&mut lifetime, Duration::ZERO)?;
        invoke(&mut lifetime, ReportAction::Copy)?;
        let copied = lifetime.clipboard().map(|copy| copy.text).ok_or("copy")?;
        invoke(&mut lifetime, ReportAction::Export)?;
        let picker = lifetime.picker().ok_or("export asks for a destination")?;
        lifetime.input(Event::Picked(picker, Ok(Some("update.txt".into()))))?;
        for private in ["Jo", "Ann", "Users", "ComfyUI", "10.0.0.7", "abc", "comfy.org", "token"] {
            assert!(!copied.contains(private), "{private} in {copied}");
        }
        assert!(copied.contains("Cannot stage <path>"));
        assert!(copied.contains("proxy <address> sent <redacted>"));
    }
    let (_, exported) = exports.begun.first().ok_or("export began")?;
    for private in ["Jo", "Ann", "ComfyUI", "10.0.0.7", "abc", "token"] {
        assert!(!exported.contains(private), "{private} in {exported}");
    }
    Ok(())
}
