// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare the genuine registry producer without entering qualification/native process work.
use super::*;
use crate::completion_wake::Wake;
/// Use the original owner/specification and one completion drain; perform no GUI or process launch.
pub(super) fn prepare_registry(fixture: &mut Fixture) -> Result<()> {
    let wake = Arc::new(Wake::default());
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
    wake.until(|| {
        let created = (|| -> Result<bool> {
            fixture
                .client
                .as_ref()
                .ok_or("Constructor client missing")?
                .drain_completions(NonZeroUsize::MIN)?;
            registry::poll(&mut fixture.registry_creation, &mut fixture.registry, false)
        })();
        match created {
            Ok(false) => None,
            Ok(true) => Some(Ok(())),
            Err(cause) => Some(Err(cause)),
        }
    })
}
