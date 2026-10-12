// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare the genuine registry producer without entering qualification/native process work.
use super::*;
use std::sync::{Condvar, Mutex};
use sugarsubstitute_contracts::execution::{CompletionWake, ExecutionError};
/// Observe only actual constructor terminal wake edges.
struct Wake {
    revision: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> std::result::Result<(), ExecutionError> {
        let mut revision = self.revision.lock().map_err(|_| {
            ExecutionError::new(
                Operation::NativeLifecycle,
                Box::new(std::io::Error::other("Constructor wake poisoned")),
            )
        })?;
        *revision += 1;
        self.changed.notify_all();
        Ok(())
    }
}
/// Use the original owner/specification and one completion drain; perform no GUI or process launch.
pub(super) fn prepare_registry(fixture: &mut Fixture) -> Result<()> {
    let wake = Arc::new(Wake { revision: Mutex::new(0), changed: Condvar::new() });
    let owner = ExecutionOwner::new(fixture.clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let spec = fixture.spec(
        JobClass::InfrastructureLifecycle,
        WindowsProcessRegistry::required_reservation_bytes(),
    )?;
    fixture.registry_creation =
        Some(registry::PendingRegistry::submit(&client, &scope, spec, fixture.owner_key)?);
    fixture.owner = Some(owner);
    fixture.client = Some(client);
    fixture.scope = Some(scope);
    loop {
        let revision = *wake.revision.lock().map_err(|_| "Constructor wake poisoned")?;
        fixture
            .client
            .as_ref()
            .ok_or("Constructor client missing")?
            .drain_completions(NonZeroUsize::MIN)?;
        if registry::poll(&mut fixture.registry_creation, &mut fixture.registry, false)? {
            return Ok(());
        }
        let guard = wake.revision.lock().map_err(|_| "Constructor wake poisoned")?;
        let (_guard, timed) = wake
            .changed
            .wait_timeout_while(guard, Duration::from_secs(10), |value| *value == revision)
            .map_err(|_| "Constructor wake poisoned")?;
        if timed.timed_out() {
            return Err("Actual constructor did not complete".into());
        }
    }
}
