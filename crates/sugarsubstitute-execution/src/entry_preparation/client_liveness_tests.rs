// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Revalidate real client admission without consulting substituted public context ports.

use super::core_binding::PREPARATION_BYTES;
use crate::{
    admission::Core,
    runtime_test_support::{ClockPort, ManualClock, TestResult, Wake, spec},
    scope::lock,
    task::Stop,
};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::{coordinator_entry::EntrySetupBudget, execution::*};

struct Ports(AtomicUsize);
impl CancelControl for Ports {
    fn is_requested(&self) -> bool {
        self.0.fetch_add(1, Ordering::AcqRel);
        false
    }
    fn cancelled(&self) -> WaitFuture {
        self.0.fetch_add(1, Ordering::AcqRel);
        Box::pin(std::future::pending())
    }
}
impl Clock for Ports {
    fn now(&self) -> Moment {
        self.0.fetch_add(1, Ordering::AcqRel);
        Moment::from_nanos(u64::MAX)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        self.0.fetch_add(1, Ordering::AcqRel);
        Box::pin(std::future::pending())
    }
}

#[test]
fn actual_client_cancellation_cannot_be_overridden_by_matching_context_ports() -> TestResult {
    let core = Core::new(
        ExecutionId::new(1)?,
        Arc::new(ClockPort(Arc::new(ManualClock::default()))),
        Arc::new(Wake::default()),
        crate::runtime::DEFAULT_MEMORY_BYTES,
    );
    let client = crate::ExecutionClient { core: core.clone() };
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let setup = EntrySetupBudget::new(Moment::from_nanos(0))?;
    let mut request = spec(1, JobClass::InfrastructureLifecycle, PREPARATION_BYTES)?;
    request.operation = Operation::NativeLifecycle;
    request.deadline = Some(setup.until());
    let mut job = client
        .submit_entry_preparation(&scope, request, setup, |_| Ok::<_, io::Error>(()))
        .map_err(|rejected| rejected.error)?;
    let queued = core
        .take(|class| class == JobClass::InfrastructureLifecycle)
        .ok_or("Actual admitted client did not enter Running")?;
    let ports = Arc::new(Ports(AtomicUsize::new(0)));
    let observed = JobContext::new(
        queued.context.owner_id(),
        queued.context.scope_id(),
        queued.context.spec(),
        queued.context.deadline(),
        ports.clone(),
        ports.clone(),
        AdmittedJobResources::new(queued.context.memory().clone(), None),
    );
    core.validate_preparation_client(
        &lock(&core.state),
        &observed,
        Moment::from_nanos(0),
        setup.until(),
    )?;
    core.cancel_job(queued.context.job_id());
    assert!(matches!(
        core.validate_preparation_client(
            &lock(&core.state),
            &observed,
            Moment::from_nanos(0),
            setup.until()
        ),
        Err(super::EntryPreparationInputError::Control(
            super::EntryPreparationControlCause::Cancelled
        ))
    ));
    assert_eq!(ports.0.load(Ordering::Acquire), 0);
    assert_eq!(scope.snapshot().reserved_bytes, PREPARATION_BYTES);
    core.flush_notifications();
    queued.job.stop(&queued.context, Some(Moment::from_nanos(0)), Stop::Cancelled);
    let completion = job.try_take_completion().ok_or("Actual cancelled job completion absent")?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    drop(completion);
    drop(observed);
    drop(queued);
    drop(job);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
