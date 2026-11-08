// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise immutable native custody across actual finite jobs and execution-owner boundaries.
use super::*;
use std::io::{Read, Seek, SeekFrom};
use sugarsubstitute_execution::ExecutionClient;
struct Fixture {
    client: ExecutionClient,
    scope: Scope,
    wake: Arc<Wake>,
    clock: Arc<TestClock>,
    next: u64,
    _owner: ExecutionOwner,
}
impl Fixture {
    fn new() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let wake = Arc::new(Wake::default());
        let clock = Arc::new(TestClock::default());
        let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        Ok(Self { client, scope, wake, clock, next: 0, _owner: owner })
    }
    fn run<T: Send + 'static>(
        &mut self,
        deadline: u64,
        body: impl FnOnce(JobContext) -> Result<T, io::Error> + Send + 'static,
    ) -> Result<T, Box<dyn Error + Send + Sync>> {
        self.next += 1;
        let mut job = self.client.submit_blocking(
            &self.scope,
            JobSpec {
                id: JobId::new(self.next)?,
                operation: Operation::Infrastructure,
                class: JobClass::InfrastructureOrdinary,
                deadline: Some(Moment::from_nanos(deadline)),
                reservation_bytes: 2 * 1024 * 1024,
                replacement: None,
            },
            body,
        )?;
        let cutoff = Instant::now() + Duration::from_secs(20);
        loop {
            let revision = *self.wake.0.lock().map_err(|_| "Retained fixture wake poisoned")?;
            self.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
            if let Some(completion) = job.try_take_completion() {
                return match completion.outcome {
                    JobOutcome::Success(value) => Ok(value),
                    JobOutcome::Failure(cause) => Err(cause.into()),
                    outcome => {
                        Err(format!("Retained fixture stopped: {:?}", outcome.kind()).into())
                    }
                };
            }
            let remaining =
                cutoff.checked_duration_since(Instant::now()).ok_or("Retained fixture expired")?;
            let (_guard, timeout) = self
                .wake
                .1
                .wait_timeout_while(
                    self.wake.0.lock().map_err(|_| "Retained fixture wake poisoned")?,
                    remaining,
                    |count| *count == revision,
                )
                .map_err(|_| "Retained fixture wake poisoned")?;
            if timeout.timed_out() {
                return Err("Retained artifact worker did not stop".into());
            }
        }
    }
}
#[test]
fn retained_reader_uses_fresh_job_controls_and_keeps_exact_immutable_native_file() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("original.bin");
    std::fs::write(&path, b"0123456789")?;
    let mut fixture = Fixture::new()?;
    let selected = path.clone();
    let reader = fixture.run(1_000_000_000, move |context| {
        RetainedArtifactReader::open(&selected, 10, context.memory(), &context)
    })?;
    assert!(std::fs::write(&path, b"changed").is_err());
    assert!(std::fs::rename(&path, directory.path().join("replacement.bin")).is_err());
    fixture.clock.expire();
    let reader = Arc::new(Mutex::new(reader));
    let source = reader.clone();
    let bytes = fixture.run(3_000_000_000, move |context| {
        source
            .lock()
            .map_err(|_| io::Error::other("Retained reader poisoned"))?
            .with_reader(&context, |input| {
                input.seek(SeekFrom::Start(3))?;
                let mut bytes = [0; 4];
                input.read_exact(&mut bytes)?;
                assert!(input.seek(SeekFrom::Start(11)).is_err());
                Ok::<_, io::Error>(bytes)
            })
            .map_err(io::Error::other)
    })?;
    assert_eq!(&bytes, b"3456");
    let source = reader.clone();
    fixture.run(3_000_000_000, move |context| {
        let mut reader = source.lock().map_err(|_| io::Error::other("Retained reader poisoned"))?;
        reader.close(&context)?;
        assert!(reader.closed());
        assert!(reader.with_reader(&context, |_| Ok::<_, io::Error>(())).is_err());
        Ok(())
    })?;
    std::fs::write(&path, b"replacement after checked close")?;
    Ok(())
}
#[test]
fn retained_reader_refuses_foreign_execution_owner_without_releasing_original_custody() -> TestResult
{
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("original.bin");
    std::fs::write(&path, b"original")?;
    let mut owner = Fixture::new()?;
    let selected = path.clone();
    let reader = owner.run(1_000_000_000, move |context| {
        RetainedArtifactReader::open(&selected, 8, context.memory(), &context)
    })?;
    let reader = Arc::new(Mutex::new(reader));
    let source = reader.clone();
    let mut foreign = Fixture::new()?;
    foreign.run(1_000_000_000, move |context| {
        let mut reader = source.lock().map_err(|_| io::Error::other("Retained reader poisoned"))?;
        assert!(reader.with_reader(&context, |_| Ok::<_, io::Error>(())).is_err());
        assert!(reader.close(&context).is_err());
        assert!(!reader.closed());
        Ok(())
    })?;
    assert!(std::fs::write(&path, b"changed").is_err());
    owner.run(1_000_000_000, move |context| {
        reader.lock().map_err(|_| io::Error::other("Retained reader poisoned"))?.close(&context)
    })?;
    assert_eq!(std::fs::read(&path)?, b"original");
    Ok(())
}
