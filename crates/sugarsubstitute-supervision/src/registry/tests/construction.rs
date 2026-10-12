// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove the original table charge, actual factory custody, and pre-allocation rejection.
use super::*;
#[test]
fn actual_table_capacity_and_identity_pin_retain_original_constructor_charge() -> TestResult {
    let h = Harness::new()?;
    let bytes = WindowsProcessRegistry::required_reservation_bytes();
    assert_eq!(
        size_of::<RegistryIdentity>(),
        size_of::<Arc<sugarsubstitute_contracts::execution::MemoryClaim>>()
    );
    assert_eq!(size_of::<SharedOwner>(), size_of::<Arc<Shared>>() + size_of::<RegistryIdentity>());
    if cfg!(target_pointer_width = "64") {
        assert_eq!(bytes, 249600);
    }

    assert_eq!(h.registry.records.len(), 131);
    assert_eq!(h.registry.records.capacity(), 131);
    assert_eq!(h.table_scope.snapshot().reserved_bytes, bytes);
    assert_eq!(h.scope.snapshot().reserved_bytes, 0);
    drop(h.tables);
    drop(h._executor);
    let pin = h.registry.identity.clone();
    let nonfinal = pin.clone();
    assert!(pin.same_as(&nonfinal));
    drop(h.registry);
    assert_eq!(h.table_scope.snapshot().reserved_bytes, bytes);
    drop(nonfinal);
    assert_eq!(h.table_scope.snapshot().reserved_bytes, bytes);
    drop(pin);
    assert_eq!(h.table_scope.snapshot().reserved_bytes, 0);
    Ok(())
}
/// Release a held native effect before its actual execution owner joins on fixture failure.
struct Release(Arc<Fake>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.release();
    }
}
#[test]
fn retained_actual_creation_factory_keeps_table_charge_after_registry_drop() -> TestResult {
    let mut h = Harness::new()?;
    let release = Release(h.fake.clone());
    h.fake.hold_create.store(true, Ordering::Release);
    let ticket = h.launch(1)?;
    h.fake.entered()?;
    assert!(h.registry.snapshot(ticket.process())?.creation_pending);
    let replacement = h.construct_registry(owner()?)?;
    drop(h.tables);
    let original = std::mem::replace(&mut h.registry, replacement);
    drop(original);
    let bytes = WindowsProcessRegistry::required_reservation_bytes();
    assert_eq!(h.table_scope.snapshot().reserved_bytes, 2 * bytes);
    h.fake.release();
    h.fake.closed()?;
    drop(h._executor);
    assert_eq!(h.table_scope.snapshot().reserved_bytes, bytes);
    drop(h.registry);
    assert_eq!(h.table_scope.snapshot().reserved_bytes, 0);
    drop(release);
    Ok(())
}
#[test]
fn constructor_rejects_wrong_class_or_byte_count_before_actual_table_effect() -> TestResult {
    let mut h = Harness::new()?;
    for class in [JobClass::NativeObservation, JobClass::InfrastructureLifecycle] {
        let mut spec = h.spec(class, WindowsProcessRegistry::required_reservation_bytes())?;
        if class == JobClass::InfrastructureLifecycle {
            spec.reservation_bytes -= 1;
        }
        assert!(matches!(
            WindowsProcessRegistry::submit_new(&h.client, &h.scope, spec, owner()?),
            Err(RegistryAdmissionError::InvalidSpecification)
        ));
    }
    assert_eq!(h.scope.snapshot().reserved_bytes, 0);
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 0);
    assert_eq!(
        h.table_scope.snapshot().reserved_bytes,
        WindowsProcessRegistry::required_reservation_bytes()
    );
    Ok(())
}

/// Hold both actual worker callbacks before their concurrent final identity drops.
#[derive(Default)]
struct DropBoundary {
    state: Mutex<(usize, bool)>,
    changed: Condvar,
}
impl DropBoundary {
    /// Count entry before releasing the finite actual worker callback.
    fn enter(&self) -> Result<(), RegistryError> {
        let mut state = lock(&self.state);
        state.0 += 1;
        self.changed.notify_all();
        while !state.1 {
            state = self
                .changed
                .wait(state)
                .map_err(|_| RegistryError::invalid("identity_drop_fixture_poisoned"))?;
        }
        Ok(())
    }
    /// Await causal entry by both existing blocking workers, with a failure guard.
    fn wait_entered(&self) -> TestResult {
        let (_state, timed) = self
            .changed
            .wait_timeout_while(lock(&self.state), Duration::from_secs(10), |state| state.0 != 2)
            .map_err(|_| "Identity drop fixture poisoned")?;
        if timed.timed_out() {
            return Err("Both actual drop callbacks did not enter".into());
        }
        Ok(())
    }
    /// Release held callbacks before any executor teardown on normal or failed paths.
    fn release(&self) {
        lock(&self.state).1 = true;
        self.changed.notify_all();
    }
}
/// Release the external return hold before the actual execution owner's fixture Drop.
struct DropRelease(Arc<DropBoundary>);
impl Drop for DropRelease {
    fn drop(&mut self) {
        self.0.release();
    }
}
#[test]
fn concurrent_actual_identity_clone_drops_return_original_table_charge_once() -> TestResult {
    let mut h = Harness::new()?;
    let boundary = Arc::new(DropBoundary::default());
    let release = DropRelease(boundary.clone());
    let first = h.registry.identity.clone();
    let second = first.clone();
    assert!(first.same_as(&second));
    let replacement = h.construct_registry(owner()?)?;
    let original = std::mem::replace(&mut h.registry, replacement);
    drop(original);
    let a_spec = h.spec(JobClass::InfrastructureLifecycle, 4096)?;
    let b_spec = h.spec(JobClass::InfrastructureLifecycle, 4096)?;
    drop(h.tables);
    let a_boundary = boundary.clone();
    let mut a = h.client.submit_blocking(&h.scope, a_spec, move |_| {
        a_boundary.enter()?;
        drop(first);
        Ok::<(), RegistryError>(())
    })?;
    let b_boundary = boundary.clone();
    let mut b = h.client.submit_blocking(&h.scope, b_spec, move |_| {
        b_boundary.enter()?;
        drop(second);
        Ok::<(), RegistryError>(())
    })?;
    boundary.wait_entered()?;
    let bytes = WindowsProcessRegistry::required_reservation_bytes();
    assert_eq!(h.table_scope.snapshot().reserved_bytes, 2 * bytes);
    boundary.release();
    let (mut a_done, mut b_done) = (false, false);
    loop {
        let prior = *lock(&h.wake.count);
        h.registry.consume(&h.client.drain_completions(std::num::NonZeroUsize::MIN)?)?;
        if let Some(completion) = a.try_take_completion() {
            assert!(matches!(
                completion.outcome,
                sugarsubstitute_contracts::execution::JobOutcome::Success(())
            ));
            a_done = true;
        }
        if let Some(completion) = b.try_take_completion() {
            assert!(matches!(
                completion.outcome,
                sugarsubstitute_contracts::execution::JobOutcome::Success(())
            ));
            b_done = true;
        }
        if a_done && b_done {
            break;
        }
        h.wake.wait(prior)?;
    }
    drop(h._executor);
    assert_eq!(h.table_scope.snapshot().reserved_bytes, bytes);
    drop(h.registry);
    assert_eq!(h.table_scope.snapshot().reserved_bytes, 0);
    drop(release);
    Ok(())
}
