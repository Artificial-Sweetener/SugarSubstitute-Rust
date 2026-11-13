// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real owned-input rejection keeps identical input and original typed cause with finite credit.
use super::*;
use crate::production::incident_host::tests::admitted_value;
use std::{error::Error, sync::Arc};
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
fn actual_owned_output_denial_preserves_input_and_first_typed_cause_until_error_disposal()
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
        let mut original = Some(42_u8);
        let cause = match client.submit_blocking_owned_box(
            &scope,
            requested,
            &mut original,
            |_, _| -> Result<(), MemoryClaimError> { Ok(()) },
        ) {
            Err(cause) => cause,
            Ok(_) => return Err("expired actual owned job was admitted".into()),
        };
        assert_eq!(original, Some(42));
        assert_eq!(scope.snapshot().reserved_bytes, 0);
        let error = OwnedOutputAdmissionFailure { cause, _original: context.memory().clone() };
        assert!(
            matches!(error.source().and_then(|cause| cause.source()).and_then(|cause| cause.downcast_ref::<AdmissionError>()), Some(AdmissionError::DeadlineExpired { deadline, now }) if *deadline == Moment::from_nanos(0) && *now == Moment::from_nanos(0))
        );
        drop(execution);
        Ok(error)
    })
}
