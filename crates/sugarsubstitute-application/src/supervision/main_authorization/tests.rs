// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual pure owner with deterministic input and completion facts.
mod ordering;
mod requests;
mod retirement;
use super::*;
use std::{error::Error, io, num::NonZeroU32};
use sugarsubstitute_contracts::{
    execution::Generation,
    lifecycle::{
        ConnectionId, CoordinatorEpoch, CoordinatorKey, InstanceId, LeaseId, LifecycleFailureCode,
        NamespaceId, ResourceGeneration, ResourceId, ResourceKey, RunGeneration,
    },
    startup::LaunchId,
};
type TestResult = Result<(), Box<dyn Error>>;
struct Fixture {
    session: MainAuthorizationSession,
    identity: MainIdentity,
    lease: LeaseKey,
    binding: ClientBinding,
}
impl Fixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        let owner = CoordinatorKey {
            namespace: NamespaceId::new([7; 32]),
            epoch: CoordinatorEpoch::new([9; 16])?,
        };
        let run =
            RunKey { owner, instance: InstanceId::new(1)?, generation: RunGeneration::new(1)? };
        let identity =
            MainIdentity { run, launch: LaunchId::new(1)?, navigation: Generation::new(1)? };
        let lease = LeaseKey {
            run,
            resource: ResourceKey {
                owner,
                id: ResourceId::new(1)?,
                generation: ResourceGeneration::new(1)?,
            },
            connection: ConnectionId::new(1)?,
            id: LeaseId::new(1)?,
        };
        let mut session = MainAuthorizationSession::new(run);
        session.begin(identity, lease, Moment::from_nanos(100))?;
        Ok(Self {
            session,
            identity,
            lease,
            binding: ClientBinding { run, connection: lease.connection },
        })
    }
    fn accept(
        &mut self,
        request: u64,
        observation: MainAuthorizationObservation,
    ) -> Result<Option<MainAuthorizationEffect>, Box<dyn Error>> {
        Ok(self.session.accept(
            self.binding,
            RequestId::new(request)?,
            observation,
            Moment::from_nanos(10),
        )?)
    }
    fn folder(&mut self) -> TestResult {
        self.accept(
            1,
            MainAuthorizationObservation::RequestedFolderAccepted {
                identity: self.identity,
                next_offset_units: 12,
            },
        )?;
        Ok(())
    }
    fn environment(&mut self) -> TestResult {
        self.folder()?;
        assert_eq!(
            self.accept(
                2,
                MainAuthorizationObservation::EnvironmentFolderComplete { identity: self.identity }
            )?,
            Some(MainAuthorizationEffect::ValidateEnvironment(self.identity))
        );
        Ok(())
    }
    fn controller(&mut self) -> TestResult {
        self.environment()?;
        self.accept(
            2,
            MainAuthorizationObservation::EnvironmentVerified { identity: self.identity },
        )?;
        self.accept(
            2,
            MainAuthorizationObservation::EnvironmentAuthorizationIssued {
                identity: self.identity,
            },
        )?;
        self.accept(
            3,
            MainAuthorizationObservation::ControllerObserved {
                identity: self.identity,
                browser_process_id: pid()?,
            },
        )?;
        Ok(())
    }
    fn rejected(
        &mut self,
        request: u64,
        observation: MainAuthorizationObservation,
        error: MainAuthorizationError,
    ) -> TestResult {
        let before = self.session.snapshot();
        let result = self.session.accept(
            self.binding,
            RequestId::new(request)?,
            observation,
            Moment::from_nanos(10),
        );
        assert!(matches!(result, Err(ref rejected) if rejected.error == error));
        assert_eq!(self.session.snapshot(), before);
        Ok(())
    }
}
fn pid() -> Result<NonZeroU32, Box<dyn Error>> {
    NonZeroU32::new(71).ok_or_else(|| io::Error::other("fixed positive PID rejected").into())
}
fn rejection(value: LifecycleRejection) -> MainAuthorizationError {
    MainAuthorizationError::Rejected(value)
}
fn failure(
    identity: MainIdentity,
    code: LifecycleFailureCode,
    message: &'static str,
) -> MainAuthorizationObservation {
    MainAuthorizationObservation::Failed {
        identity,
        failure: LifecycleFailure::new(code, Box::new(io::Error::other(message))),
    }
}
