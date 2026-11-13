// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real central rejection preserves typed cause and original funding without native frame authority.
use super::*;
use crate::production::incident_host::tests::admitted_value;
use std::sync::Arc;
use sugarsubstitute_execution::ExecutionOwner;
struct Clock;
impl sugarsubstitute_contracts::execution::Clock for Clock {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
struct Wake;
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        Ok(())
    }
}
#[test]
fn actual_admission_rejection_keeps_exact_cause_and_original_reservation_credit()
-> Result<(), Box<dyn Error + Send + Sync>> {
    admitted_value(crate::production::incident_host::CONTROLLER_BYTES, |context| {
        let execution = ExecutionOwner::new(Arc::new(Clock), Arc::new(Wake))?;
        let client = execution.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let requested = JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(0)),
            reservation_bytes: 64 * 1024,
            replacement: None,
        };
        let cause =
            match client
                .submit_blocking(&scope, requested, |_| -> Result<(), MemoryClaimError> { Ok(()) })
            {
                Err(cause) => cause,
                Ok(_) => return Err("expired actual job was admitted".into()),
            };
        assert!(
            matches!(cause, AdmissionError::DeadlineExpired { deadline, now } if deadline == Moment::from_nanos(0) && now == Moment::from_nanos(0))
        );
        assert_eq!(scope.snapshot().reserved_bytes, 0);
        let error = OwnedViewAdmissionFailure {
            cause: Cause::Admission(cause),
            _original: context.memory().clone(),
        };
        assert!(
            matches!(error.source().and_then(|cause| cause.downcast_ref::<AdmissionError>()), Some(AdmissionError::DeadlineExpired { deadline, now }) if *deadline == Moment::from_nanos(0) && *now == Moment::from_nanos(0))
        );
        let mut untouched = ViewService::new(&context)?;
        assert!(untouched.take_admission_failure().is_none());
        assert!(!untouched.admission_transferred);
        drop(execution);
        Ok(error)
    })
}
