// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise shutdown causality after partial native initialization without opening a GUI.
use super::*;
use crate::report::ReportRequestId;
use std::{error::Error, sync::Arc};
use sugarsubstitute_contracts::{
    execution::{CompletionWake, MemoryRelease},
    incident::*,
    lifecycle::*,
};
struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}
#[derive(Default)]
struct Content {
    shutdowns: usize,
    polls: usize,
    retired: usize,
    actual_settled: bool,
    reject_retirement: bool,
}
impl ReportSessionContent for Content {
    fn start(&mut self, _: Arc<dyn CompletionWake>) -> Result<(), ReportSurfaceError> {
        Ok(())
    }
    fn poll(&mut self, _: NonZeroUsize) -> Result<ReportSurfaceUpdate, ReportSurfaceError> {
        Ok(ReportSurfaceUpdate::None)
    }
    fn action(&mut self, _: ReportRequestId, _: ReportCommand) -> Result<(), ReportSurfaceError> {
        Ok(())
    }
    fn observe(&mut self, observation: ReportSurfaceObservation) -> Result<(), ReportSurfaceError> {
        if matches!(observation, ReportSurfaceObservation::NativeRetired(_)) {
            self.retired += 1;
            if self.reject_retirement {
                return Err(ReportSurfaceError::rejected("actual_host_observer_failure"));
            }
        }
        Ok(())
    }
    fn request_shutdown(&mut self) -> Result<(), ReportSurfaceError> {
        self.shutdowns += 1;
        Ok(())
    }
    fn poll_shutdown(&mut self) -> Result<bool, ReportSurfaceError> {
        self.polls += 1;
        Ok(self.actual_settled)
    }
    fn finish_shutdown(&mut self) -> Result<(), ReportSurfaceError> {
        Ok(())
    }
}
fn session() -> Result<Session, Box<dyn Error>> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let binding = ReportBinding::new(
        IncidentKey::new(IncidentSubject::Supervisor(owner)),
        IncidentRevision::new(1)?,
        ReportSessionId::new(1)?,
    );
    let lease = MemoryLease::new(ReportSurfaceOptions::KNOWN_STORAGE_BYTES, Box::new(Release));
    let view = NativeReportView::from_claimed(binding, "report\n".into(), lease.claim(512)?)?;
    Ok(Session::new(ReportSurfaceOptions::new(view, "en", false, 960, 740, lease)?))
}
#[test]
fn report_partial_initialization_close_is_once_and_keeps_original_shutdown_deadline()
-> Result<(), Box<dyn Error>> {
    let mut session = session()?;
    let mut content = Content::default();
    session.fail(
        &mut content,
        ReportSurfaceError::new("actual_native_initialization", std::io::Error::other("original")),
    );
    let deadline = session.close_deadline;
    session.close(&mut content);
    assert_eq!(session.close_deadline, deadline);
    assert_eq!(content.shutdowns, 1);
    assert_eq!(
        session.failure.as_ref().ok_or("lost cause")?.operation(),
        "actual_native_initialization"
    );
    assert!(!session.native_retired);
    assert!(!session.settled);
    assert!(!session.teardown_ready());
    session.shutdown(&mut content);
    session.observe_retirement(&mut content);
    assert!(session.native_retired);
    assert!(!session.settled);
    assert!(!session.teardown_ready());
    assert_eq!(content.retired, 1);
    content.actual_settled = true;
    session.shutdown(&mut content);
    session.observe_retirement(&mut content);
    assert!(session.settled);
    assert!(session.teardown_ready());
    assert_eq!(content.retired, 1);
    assert_eq!(content.polls, 2);
    Ok(())
}
#[test]
fn report_actual_native_retirement_is_not_reissued_when_host_observer_rejects()
-> Result<(), Box<dyn Error>> {
    let mut session = session()?;
    let mut content = Content { reject_retirement: true, ..Default::default() };
    session.close(&mut content);
    session.shutdown(&mut content);
    session.observe_retirement(&mut content);
    session.observe_retirement(&mut content);
    assert_eq!(content.retired, 1);
    assert!(session.native_retired);
    assert!(!session.settled);
    assert_eq!(
        session.failure.as_ref().ok_or("lost observer cause")?.operation(),
        "actual_host_observer_failure"
    );
    Ok(())
}
#[test]
fn report_content_settlement_cannot_authorize_native_close_or_retirement()
-> Result<(), Box<dyn Error>> {
    let mut session = session()?;
    let mut content = Content { actual_settled: true, ..Default::default() };
    session.shutdown(&mut content);
    session.observe_retirement(&mut content);
    assert!(session.settled);
    assert!(!session.state.closing());
    assert!(!session.native_retired);
    assert!(!session.teardown_ready());
    assert!(session.failure.is_some());
    assert_eq!(content.retired, 0);
    Ok(())
}
