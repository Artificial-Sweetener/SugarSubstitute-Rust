// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Build a synthetic native source only through genuine admitted preparation ownership.

use super::{
    EntryPreparationContext, EntryPreparationFailure, EntryPreparationSuccess, EntryRequestInput,
    core_binding::{OriginalPreparation, PREPARATION_BYTES},
    owner::EntryPreparationOwner,
    port::{AllocationState, EnvironmentPort},
};
use crate::{
    ExecutionClient,
    admission::Core,
    runtime_test_support::{TestResult, spec},
    task::QueuedJob,
};
use std::{io, path::PathBuf, sync::Arc};
use sugarsubstitute_contracts::{
    configuration::*, coordinator_entry::EntrySetupBudget, execution::*,
};

pub(crate) struct Prepared {
    pub(crate) input: Option<EntryRequestInput>,
    pub(crate) owner: EntryPreparationOwner,
    pub(crate) queued: QueuedJob,
    pub(crate) job: OwnedJob<EntryPreparationSuccess<()>, EntryPreparationFailure<io::Error>>,
}
pub(crate) fn prepare(core: &Arc<Core>, scope: &Scope) -> TestResult<Prepared> {
    let client = ExecutionClient { core: core.clone() };
    let setup = EntrySetupBudget::new(Moment::from_nanos(0))?;
    let mut specification = spec(1, JobClass::InfrastructureLifecycle, PREPARATION_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    specification.deadline = Some(setup.until());
    let job = client
        .submit_entry_preparation(scope, specification, setup, |_| Ok::<_, io::Error>(()))
        .map_err(|rejection| rejection.error)?;
    let queued = core
        .take(|class| class == JobClass::InfrastructureLifecycle)
        .ok_or("Actual preparation was not dequeued")?;
    let (binding, memory) = OriginalPreparation::mint(core, &queued.context, setup)?;
    let owner =
        EntryPreparationOwner::new(core.clone(), binding, memory).map_err(|(cause, memory)| {
            drop(memory);
            cause
        })?;
    owner.register()?;
    let context = EntryPreparationContext::new(owner.clone());
    let environment = context.capture_with_port(Box::new(Port(AllocationState::Vacant)))?;
    let config = StartupConfiguration::new(
        ConnectionConfiguration::Attach(LoopbackOrigin::parse("http://localhost:8188", 8188)?),
        ProfileId::new("profile_1".into())?,
        Presentation::new("en", Theme::Light, false)?,
    );
    #[cfg(windows)]
    let path = PathBuf::from(r"E:\entry-owned-fixture");
    #[cfg(not(windows))]
    let path = PathBuf::from("/entry-owned-fixture");
    let input = environment.into_request(
        config,
        path.clone(),
        path.clone(),
        path.clone(),
        path.clone(),
        path,
    )?;
    Ok(Prepared { input: Some(input), owner, queued, job })
}
struct Port(AllocationState);
impl EnvironmentPort for Port {
    fn get(&mut self) -> Result<(), super::cause::EntryEnvironmentCause> {
        self.0 = AllocationState::Held;
        Ok(())
    }
    fn read_unit(&mut self, index: usize) -> Result<u16, super::cause::EntryEnvironmentCause> {
        [65, 61, 0xd800, 0, 0]
            .get(index)
            .copied()
            .ok_or(super::cause::EntryEnvironmentCause::InvalidNativeState)
    }
    fn free(&mut self) -> Result<(), super::cause::EntryEnvironmentCause> {
        self.0 = AllocationState::Released;
        Ok(())
    }
    fn allocation_state(&self) -> AllocationState {
        self.0
    }
}
