// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Update failure reports: Copy and Export without Restart, export outcomes, and report text.
use super::*;

/// A fake export that finishes when told to.
#[derive(Default)]
struct Exports {
    begun: Vec<PathBuf>,
    outcome: Option<std::result::Result<(), Box<dyn Error>>>,
    closed: bool,
}
impl UpdateExport for Exports {
    fn start(&mut self, _: Arc<dyn CompletionWake>) -> TestResult {
        Ok(())
    }
    fn begin(&mut self, destination: PathBuf) -> TestResult {
        self.begun.push(destination);
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
    {
        let mut owner = UpdateReport::new(Some(&mut exports), copy.report);
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
    }
    assert_eq!(exports.begun, vec![PathBuf::from(r"D:\reports\update.txt")]);
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
    let mut owner = UpdateReport::<Exports>::new(None, copy.report);
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
    let mut owner = UpdateReport::new(Some(&mut exports), copy.report);
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

#[test]
fn an_update_attempt_report_names_its_version_step_and_causes_in_the_users_language() {
    use sugarsubstitute_contracts::update::attempt::{AttemptPhase, UpdateReport as Attempt};
    let attempt = Attempt {
        version: Some("2.0.0".into()),
        phase: AttemptPhase::Downloading,
        causes: vec!["The download stopped".into(), "The connection was reset".into()],
    };
    let report = ReportPresentation::update_attempt(messages("en"), "SugarSubstitute", &attempt);
    assert_eq!(
        report.details.as_deref(),
        Some(
            "Update failed\nVersion: 2.0.0\nStep: Downloading the update\n\n\
             The download stopped\nThe connection was reset\n"
        )
    );
    assert_eq!(report.content.summary[1].value, "Downloading the update");
    let unknown = Attempt { version: None, ..attempt };
    let es = messages("es");
    let report = ReportPresentation::update_attempt(es, "SugarSubstitute", &unknown);
    let text = report.details.unwrap_or_default();
    assert!(text.contains(&format!("Versión: {}", es.report.unavailable)));
    assert!(text.contains("Descargando la actualización"));
    assert_eq!(report.content.summary[0].value, es.report.unavailable);
}
