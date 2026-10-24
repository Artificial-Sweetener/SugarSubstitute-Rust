// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Run successive storage phases under one actual execution admission ledger.
use super::*;
use sugarsubstitute_contracts::execution::{AdmissionError, Scope};
use sugarsubstitute_execution::ExecutionClient;
type Failure = Box<dyn Error + Send + Sync>;

/// Rejection tests observe admission itself; successive-phase tests await retiring capacity.
enum Capacity {
    Wait,
    Once,
}

/// Keep the original execution owner alive while phase outputs retain admitted byte reservations.
pub(crate) struct Pipeline {
    client: ExecutionClient,
    scope: Scope,
    wake: Arc<Wake>,
    next: u64,
    _owner: ExecutionOwner,
}
impl Pipeline {
    pub(crate) fn new() -> Result<Self, Failure> {
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(Arc::new(TestClock::default()), wake.clone())?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        Ok(Self { client, scope, wake, next: 0, _owner: owner })
    }
    /// Admit another real worker without resetting capacity held by earlier phase results.
    pub(crate) fn run<T: Send + 'static>(
        &mut self,
        bytes: u64,
        body: impl FnOnce(JobContext) -> Result<T, Failure> + Send + 'static,
    ) -> Result<T, Failure> {
        self.execute(bytes, body, Capacity::Wait)
    }

    /// Observe immediate admission rejection without waiting for custody retained by this test.
    /// Successful admission still consumes the actual worker terminal before returning its value.
    pub(crate) fn run_once<T: Send + 'static>(
        &mut self,
        bytes: u64,
        body: impl FnOnce(JobContext) -> Result<T, Failure> + Send + 'static,
    ) -> Result<T, Failure> {
        self.execute(bytes, body, Capacity::Once)
    }

    /// Keep job identity, factory ownership and terminal observation identical for both policies.
    fn execute<T: Send + 'static>(
        &mut self,
        bytes: u64,
        body: impl FnOnce(JobContext) -> Result<T, Failure> + Send + 'static,
        capacity: Capacity,
    ) -> Result<T, Failure> {
        let id = self.admit(|_, _, id| Ok(id))?;
        let body = Arc::new(Mutex::new(Some(body)));
        let submit = || {
            let body = body.clone();
            self.client.submit_blocking(
                &self.scope,
                JobSpec {
                    id,
                    operation: Operation::NativeLifecycle,
                    class: JobClass::InfrastructureLifecycle,
                    deadline: Some(Moment::from_nanos(2_000_000_000)),
                    reservation_bytes: bytes,
                    replacement: None,
                },
                move |context| {
                    let body = crate::registry::lock(&body)
                        .take()
                        .ok_or("Storage factory already consumed")?;
                    body(context)
                },
            )
        };
        let mut job = match capacity {
            Capacity::Wait => self.wait_capacity(submit)?,
            Capacity::Once => submit()?,
        };
        match self.observe(|| job.try_take_completion())?.outcome {
            JobOutcome::Success(value) => Ok(value),
            JobOutcome::Failure(cause) => Err(cause),
            other => Err(format!("Pipeline worker stopped: {:?}", other.kind()).into()),
        }
    }

    /// A terminal value can precede disposal of the adapter's last context reference.
    /// Memory rejection arms the adapter's capacity notification. Keep the factory unconsumed
    /// and retry only when the original observer receives activity; other rejections are final.
    fn wait_capacity<T>(
        &self,
        mut submit: impl FnMut() -> Result<T, AdmissionError>,
    ) -> Result<T, Failure> {
        self.observe(|| match submit() {
            Ok(value) => Some(Ok(value)),
            Err(AdmissionError::MemorySaturated { .. }) => None,
            Err(cause) => Some(Err(cause)),
        })?
        .map_err(|cause| Box::new(cause) as Failure)
    }

    /// Submit a production owner using this same ledger and increasing job identities.
    pub(crate) fn admit<T>(
        &mut self,
        submit: impl FnOnce(&ExecutionClient, &Scope, JobId) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        self.next = self.next.checked_add(1).ok_or("Pipeline job identity exhausted")?;
        submit(&self.client, &self.scope, JobId::new(self.next)?)
    }

    /// Admit a phase after actual capacity notifications retire earlier reservations. Keep one
    /// job identity and the original observer bound; permanent or non-memory rejection is final.
    pub(crate) fn admit_available<T>(
        &mut self,
        mut submit: impl FnMut(&ExecutionClient, &Scope, JobId) -> io::Result<T>,
    ) -> Result<T, Failure> {
        let id = self.admit(|_, _, id| Ok(id))?;
        self.observe(|| match submit(&self.client, &self.scope, id) {
            Ok(value) => Some(Ok(value)),
            Err(cause)
                if matches!(
                    cause.get_ref().and_then(|cause| cause.downcast_ref::<AdmissionError>()),
                    Some(AdmissionError::MemorySaturated { .. })
                ) =>
            {
                None
            }
            Err(cause) => Some(Err(cause)),
        })?
        .map_err(|cause| Box::new(cause) as Failure)
    }

    /// Observe the real owner's terminal delivery with a finite observer bound and native wake.
    pub(crate) fn observe<T>(&self, mut poll: impl FnMut() -> Option<T>) -> Result<T, Failure> {
        let until = Instant::now() + Duration::from_secs(20);
        loop {
            let revision = *crate::registry::lock(&self.wake.0);
            self.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
            if let Some(completion) = poll() {
                return Ok(completion);
            }
            let remaining =
                until.checked_duration_since(Instant::now()).ok_or("Pipeline observer expired")?;
            let (_guard, timeout) = self
                .wake
                .1
                .wait_timeout_while(crate::registry::lock(&self.wake.0), remaining, |current| {
                    *current == revision
                })
                .map_err(|_| "Pipeline observer poisoned")?;
            if timeout.timed_out() {
                return Err("Pipeline observer expired".into());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_admission_observes_actual_capacity_release_before_reusing_its_job_id() -> TestResult {
        let mut pipeline = Pipeline::new()?;
        let mut retained =
            Some(pipeline.run_once(240 * 1024 * 1024, |context| Ok(context.memory().clone()))?);
        let mut attempts = 0;
        let mut job = pipeline.admit_available(|client, scope, id| {
            attempts += 1;
            let result = client.submit_blocking(
                scope,
                JobSpec {
                    id,
                    operation: Operation::Infrastructure,
                    class: JobClass::InfrastructureLifecycle,
                    deadline: Some(Moment::from_nanos(2_000_000_000)),
                    reservation_bytes: 32 * 1024 * 1024,
                    replacement: None,
                },
                |_| Ok::<_, io::Error>(7),
            );
            if matches!(&result, Err(AdmissionError::MemorySaturated { .. })) {
                drop(retained.take());
            }
            result.map_err(io::Error::other)
        })?;
        assert!(attempts >= 2);
        assert_eq!(job.spec().id.get(), 2);
        assert_eq!(job.spec().reservation_bytes, 32 * 1024 * 1024);
        assert!(matches!(
            pipeline.observe(|| job.try_take_completion())?.outcome,
            JobOutcome::Success(7)
        ));
        Ok(())
    }

    #[test]
    fn single_attempt_reports_retained_memory_rejection_without_running_the_factory() -> TestResult
    {
        let mut pipeline = Pipeline::new()?;
        let retained = pipeline.run_once(200 * 1024 * 1024, |context| {
            Ok(context.memory().claim(200 * 1024 * 1024)?)
        })?;
        let calls = Arc::new(AtomicU64::new(0));
        let called = calls.clone();
        let rejected = pipeline.run_once(128 * 1024 * 1024, move |_| {
            called.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });
        let cause = rejected.err().ok_or("Retained memory did not exclude admission")?;
        assert!(matches!(
            cause.downcast_ref::<AdmissionError>(),
            Some(AdmissionError::MemorySaturated { .. })
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        drop(retained);
        assert_eq!(pipeline.run(128 * 1024 * 1024, |_| Ok(7))?, 7);
        Ok(())
    }

    #[test]
    fn storage_admission_waits_for_signalled_capacity_without_changing_its_request() -> TestResult {
        let pipeline = Pipeline::new()?;
        let mut attempts = 0;
        let value = pipeline.wait_capacity(|| {
            attempts += 1;
            if attempts == 1 {
                pipeline.wake.wake().map_err(AdmissionError::Unexpected)?;
                Err(AdmissionError::MemorySaturated { requested_bytes: 136, available_bytes: 72 })
            } else {
                Ok(136)
            }
        })?;
        assert_eq!(attempts, 2);
        assert_eq!(value, 136);
        Ok(())
    }
}
