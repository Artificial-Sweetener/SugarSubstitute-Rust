// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Show a crash report in the real window on a private desktop and prove its session settles
//! only after the window has actually retired.
use super::*;

/// A reporter session that acknowledges dismissal durably, and settles only after it has
/// observed the native window's actual retirement.
#[derive(Default)]
struct AcknowledgingSession {
    events: Vec<&'static str>,
    binding: Option<sugarsubstitute_contracts::incident::ReportBinding>,
    acknowledge: bool,
    /// The committed view the session replaces the first presented view with.
    committed: Option<(
        sugarsubstitute_contracts::incident::ReportBinding,
        crate::report::NativeReportView,
    )>,
}
impl crate::report::ReportSessionContent for AcknowledgingSession {
    fn start(
        &mut self,
        _: Arc<dyn CompletionWake>,
    ) -> std::result::Result<(), crate::report::ReportSurfaceError> {
        Ok(())
    }
    fn poll(
        &mut self,
        _: std::num::NonZeroUsize,
    ) -> std::result::Result<crate::report::ReportSurfaceUpdate, crate::report::ReportSurfaceError>
    {
        use sugarsubstitute_contracts::incident::{
            DurableAcknowledgement, IncidentRevision, IncidentUtc,
        };
        if self.events.contains(&"presented")
            && let Some((old, view)) = self.committed.take()
        {
            let current = view.binding();
            return Ok(crate::report::ReportSurfaceUpdate::ReplaceView { old, current, view });
        }
        let Some(binding) = self.binding.filter(|_| std::mem::take(&mut self.acknowledge)) else {
            return Ok(crate::report::ReportSurfaceUpdate::None);
        };
        let at = IncidentUtc::from_unix_seconds(1)
            .map_err(|_| crate::report::ReportSurfaceError::rejected("fixture_time"))?;
        let revision = IncidentRevision::new(binding.revision().get() + 1)
            .map_err(|_| crate::report::ReportSurfaceError::rejected("fixture_revision"))?;
        Ok(crate::report::ReportSurfaceUpdate::CloseAfterDurableAck {
            binding,
            acknowledgement: DurableAcknowledgement::new(binding.incident(), revision, at),
        })
    }
    fn action(
        &mut self,
        _: crate::report::ReportRequestId,
        command: sugarsubstitute_contracts::incident::ReportCommand,
    ) -> std::result::Result<(), crate::report::ReportSurfaceError> {
        self.events.push("dismiss");
        self.binding = Some(command.binding());
        self.acknowledge = true;
        Ok(())
    }
    fn observe(
        &mut self,
        observation: crate::report::ReportSurfaceObservation,
    ) -> std::result::Result<(), crate::report::ReportSurfaceError> {
        use crate::report::ReportSurfaceObservation as Seen;
        self.events.push(match observation {
            Seen::Presented(_) => "presented",
            Seen::ViewReplaced(_) => "view committed",
            Seen::NativeRetired(_) => "window retired",
            Seen::Failed { .. } => "failed",
            _ => "other",
        });
        Ok(())
    }
    fn request_shutdown(&mut self) -> std::result::Result<(), crate::report::ReportSurfaceError> {
        self.events.push("shutdown");
        Ok(())
    }
    fn poll_shutdown(&mut self) -> std::result::Result<bool, crate::report::ReportSurfaceError> {
        Ok(self.events.contains(&"window retired"))
    }
    fn finish_shutdown(&mut self) -> std::result::Result<(), crate::report::ReportSurfaceError> {
        self.events.push("finished");
        Ok(())
    }
}

fn crash_body() -> TestResult {
    use sugarsubstitute_contracts::{
        execution::MemoryLease,
        incident::{
            IncidentKey, IncidentRevision, IncidentSubject, ReportBinding,
            ReportRestartAvailability, ReportSessionId,
        },
        lifecycle::{CoordinatorEpoch, CoordinatorKey, NamespaceId},
    };
    struct Release;
    impl sugarsubstitute_contracts::execution::MemoryRelease for Release {
        fn release(&self, _: u64) {}
    }
    let copy = messages("en");
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let binding = ReportBinding::new(
        IncidentKey::new(IncidentSubject::Supervisor(owner)),
        IncidentRevision::new(1)?,
        ReportSessionId::new(1)?,
    );
    let lease = MemoryLease::new(
        2 * crate::report::ReportSurfaceOptions::KNOWN_STORAGE_BYTES,
        Box::new(Release),
    );
    let text = report_text(40);
    let view = crate::report::NativeReportView::from_claimed(
        binding,
        text.clone(),
        lease.claim(text.capacity() as u64 + 4096)?,
        ReportRestartAvailability::Available,
    )?;
    let committed = crate::report::NativeReportView::from_claimed(
        ReportBinding::new(binding.incident(), IncidentRevision::new(2)?, binding.session()),
        text.clone(),
        lease.claim(text.capacity() as u64 + 4096)?,
        ReportRestartAvailability::Available,
    )?;
    let options = crate::report::ReportSurfaceOptions::new(view, "en", lease)?;
    let session = Rc::new(RefCell::new(AcknowledgingSession {
        committed: Some((binding, committed)),
        ..AcknowledgingSession::default()
    }));
    let mut inner = crash::CrashReport::new(
        options,
        Shared(session.clone()),
        Box::new(std::time::Instant::now),
    );
    let mut script = Script::new(
        copy.report.title,
        &mut inner,
        vec![
            Step::Capture("crash-report"),
            Step::Click(ReportAction::IncludeDump),
            Step::Capture("crash-report-dump-consent"),
            Step::Click(ReportAction::ToggleDetails),
            Step::Capture("crash-report-details"),
            Step::Click(ReportAction::Close),
        ],
    );
    let captures = show(ReportPresentation::crash(copy, text), &mut script)?;
    assert_eq!(script.failure, None);
    assert_eq!(script.next, 6, "every step ran");
    assert_eq!(captures.len(), 3);
    assert!(captures[2].2 > captures[1].2 + 200, "details grew the window");
    assert_painted(&captures);
    drop(script);
    inner.receipt(Ok(()))?;
    assert_eq!(
        session.borrow().events,
        ["presented", "view committed", "dismiss", "shutdown", "window retired", "finished"],
        "the session settles only after the window has retired"
    );
    keep(&captures)
}

/// Shares the fixture session with the test after the window has closed.
struct Shared(Rc<RefCell<AcknowledgingSession>>);
impl crate::report::ReportSessionContent for Shared {
    fn start(
        &mut self,
        wake: Arc<dyn CompletionWake>,
    ) -> std::result::Result<(), crate::report::ReportSurfaceError> {
        self.0.borrow_mut().start(wake)
    }
    fn poll(
        &mut self,
        maximum: std::num::NonZeroUsize,
    ) -> std::result::Result<crate::report::ReportSurfaceUpdate, crate::report::ReportSurfaceError>
    {
        self.0.borrow_mut().poll(maximum)
    }
    fn action(
        &mut self,
        request: crate::report::ReportRequestId,
        command: sugarsubstitute_contracts::incident::ReportCommand,
    ) -> std::result::Result<(), crate::report::ReportSurfaceError> {
        self.0.borrow_mut().action(request, command)
    }
    fn observe(
        &mut self,
        observation: crate::report::ReportSurfaceObservation,
    ) -> std::result::Result<(), crate::report::ReportSurfaceError> {
        self.0.borrow_mut().observe(observation)
    }
    fn request_shutdown(&mut self) -> std::result::Result<(), crate::report::ReportSurfaceError> {
        self.0.borrow_mut().request_shutdown()
    }
    fn poll_shutdown(&mut self) -> std::result::Result<bool, crate::report::ReportSurfaceError> {
        self.0.borrow_mut().poll_shutdown()
    }
    fn finish_shutdown(&mut self) -> std::result::Result<(), crate::report::ReportSurfaceError> {
        self.0.borrow_mut().finish_shutdown()
    }
}

#[test]
fn crash_report_offers_dump_consent_and_settles_after_its_window_retires() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(
            module_path!(),
            "::crash_report_offers_dump_consent_and_settles_after_its_window_retires"
        ),
        crash_body,
    )
}
