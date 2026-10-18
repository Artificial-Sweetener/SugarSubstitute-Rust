// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Return obsolete Root rendering credit before actual source-close admission.
use super::*;
use sugarsubstitute_contracts::execution::{
    AdmissionError, ExecutionPort, JobClass, JobCompletion, JobOutcome, JobSpec, Operation,
    OwnedJob,
};
use sugarsubstitute_supervision::installed_frontend::FRONTEND_SOURCE_BYTES;

/// Route the real shared mailbox while retaining each tested job's own terminal.
fn terminal<T: Send + 'static, E: Send + 'static>(
    content: &mut EntryPresentation<Child>,
    wake: &Wake,
    job: &mut OwnedJob<T, E>,
) -> Result<JobCompletion<T, E>, Box<dyn Error>> {
    let until = Instant::now().checked_add(Duration::from_secs(30)).ok_or("Guard overflow")?;
    loop {
        let revision = wake.revision()?;
        content.poll_frame(NonZeroUsize::MIN.saturating_add(31))?;
        if let Some(terminal) = job.try_take_completion() {
            return Ok(terminal);
        }
        wake.wait(revision, until)?;
    }
}

#[test]
fn actual_hidden_root_returns_render_credit_before_original_source_close_and_keeps_entry_alive()
-> TestResult {
    let child = Child::default();
    let pending = child.pending.clone();
    let acknowledged = child.acknowledged.clone();
    let mut content = content(child)?;
    let wake = Arc::new(Wake::default());
    content.start(wake.clone())?;
    content.set_visible(true)?;
    content.request_frame(FrameShape::new(558, 558, 1.0)?, Duration::ZERO)?;
    let until = Instant::now().checked_add(Duration::from_secs(30)).ok_or("Guard overflow")?;
    let frame = loop {
        let revision = wake.revision()?;
        if let Some(frame) = content.poll_frame(NonZeroUsize::MIN.saturating_add(31))? {
            break frame;
        }
        wake.wait(revision, until)?;
    };
    drop(frame);
    // Native first submission is the external boundary; avoid unrelated invalid configuration.
    content.first_presented = true;
    let (client, cleanup, jobs) = content.entry_ref()?.presentation_test_admission()?;
    let spec = |bytes| -> Result<JobSpec, Box<dyn Error>> {
        Ok(JobSpec {
            id: jobs.allocate()?,
            operation: Operation::Readiness,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(sugarsubstitute_contracts::execution::Moment::from_nanos(
                30_000_000_000,
            )),
            reservation_bytes: bytes,
            replacement: None,
        })
    };
    let mut source = sugarsubstitute_supervision::installed_frontend::boundary_fixture::owner()?;
    let source_spec = spec(FRONTEND_SOURCE_BYTES)?;
    source.submit_inspection(&client, &cleanup, source_spec)?;
    let lease = loop {
        let revision = wake.revision()?;
        content.poll_frame(NonZeroUsize::MIN.saturating_add(31))?;
        if let Some(lease) = source.take_inspection()? {
            break lease;
        }
        wake.wait(revision, until)?;
    };
    let cache = sugarsubstitute_splash::SPLASH_CACHE_RESERVATION_BYTES;
    let pressure = 256 * 1024 * 1024 - cache - 2 * FRONTEND_SOURCE_BYTES + 1;
    let mut pressure_job = client.submit_blocking(&cleanup, spec(pressure)?, move |context| {
        context.memory().claim(pressure)
    })?;
    let pressure_terminal = terminal(&mut content, &wake, &mut pressure_job)?;
    assert!(matches!(pressure_terminal.outcome, JobOutcome::Success(_)));
    let rejected = source.submit_close(&client, &cleanup, spec(FRONTEND_SOURCE_BYTES)?);
    assert!(matches!(rejected, Err(AdmissionError::MemorySaturated { .. })));
    content.request_frame(FrameShape::new(558, 558, 1.0)?, Duration::from_millis(1))?;
    pending.set(Some(main_identity(1)?));
    content.observe_handoff()?;
    assert!(acknowledged.borrow().is_empty());
    content.presentation_hidden()?;
    assert_eq!(acknowledged.borrow().as_slice(), &[main_identity(1)?]);
    assert!(!content.closing && !content.entry_ref()?.closing());
    assert!(!content.preparation.as_mut().ok_or("Render driver absent")?.poll_retirement()?);
    loop {
        let revision = wake.revision()?;
        assert!(content.poll_frame(NonZeroUsize::MIN.saturating_add(31))?.is_none());
        if content.preparation.as_mut().ok_or("Render driver absent")?.poll_retirement()? {
            break;
        }
        wake.wait(revision, until)?;
    }
    let mut source_close = source.submit_close(&client, &cleanup, spec(FRONTEND_SOURCE_BYTES)?)?;
    let closed = terminal(&mut content, &wake, &mut source_close)?;
    assert!(matches!(closed.outcome, JobOutcome::Success(())));
    assert!(lease.validate_live().is_err());
    assert!(!content.presentation_visible());
    drop(closed);
    drop(source_close);
    drop(lease);
    drop(source);
    drop(pressure_terminal);
    drop(pressure_job);
    drop(cleanup);
    close(&mut content)
}
