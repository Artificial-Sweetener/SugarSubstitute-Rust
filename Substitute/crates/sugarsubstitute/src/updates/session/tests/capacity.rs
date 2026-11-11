// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Deferral does not turn transient reservation overlap into failure or extend the campaign.
use super::*;
use crate::updates::test_support::retain_capacity;

#[test]
fn deferred_phase_keeps_its_original_deadline_and_can_retire_on_cleanup() -> Result<()> {
    let wake = Arc::new(Wake::default());
    let clock = Arc::new(Manual::default());
    let mut session = Session::new(1024 * 1024, Box::new(|_| Ok(None)));
    session.clock = clock.clone();
    session.start(wake.clone())?;
    observe(&wake, || {
        Ok(session.activity(NonZeroUsize::MIN.saturating_add(31))? == HeadlessStep::Shutdown)
    })?;
    let bytes = serde_json::to_vec(&serde_json::json!({
        "schema_version":1,"repository":vec![92;32],
        "feed":{"transport":"https","base":"https://updates.example.test/","redirects":[]}
    }))?;
    session.worker = crate::updates::configuration(
        &bytes,
        r"E:\unopened\app".into(),
        r"E:\unopened\workspace".into(),
        Duration::from_secs(30),
    )?
    .map(Worker::new)
    .transpose()?;
    let held = retain_capacity(
        session.client.as_ref().ok_or("Client absent")?,
        session.work.as_ref().ok_or("Scope absent")?,
        session.jobs.allocate()?,
        &wake,
    )?;
    let until = session.until;
    assert_eq!(session.activity(NonZeroUsize::MIN)?, HeadlessStep::Active);
    assert!(session.receipt.is_none());
    let worker = session.worker.as_ref().ok_or("Worker absent")?;
    assert!(!worker.pending());
    assert_eq!(worker.progress(), Progress::Opening);
    assert_eq!(session.until, until);
    clock.0.store(901_000_000_000, Ordering::SeqCst);
    let error = session.activity(NonZeroUsize::MIN).err().ok_or("Deadline was renewed")?;
    assert_eq!(
        error.downcast_ref::<io::Error>().map(io::Error::kind),
        Some(io::ErrorKind::TimedOut)
    );
    drop(held);
    finish(&mut session, &wake)
}
