// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Deterministic authored ownership checks without UIA core registration or external caches.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_contracts::execution::{MemoryLease, MemoryRelease};
use windows::core::PCWSTR;

struct Release(Arc<AtomicU64>);
impl MemoryRelease for Release {
    fn release(&self, bytes: u64) {
        self.0.fetch_add(bytes, Ordering::AcqRel);
    }
}

#[implement(IValueProvider, Agile = false)]
struct NativeValue;
impl IValueProvider_Impl for NativeValue_Impl {
    fn SetValue(&self, _: &PCWSTR) -> windows::core::Result<()> {
        Err(E_INVALIDARG.into())
    }
    fn Value(&self) -> windows::core::Result<BSTR> {
        Ok(BSTR::from("owned Unicode 😀"))
    }
    fn IsReadOnly(&self) -> windows::core::Result<BOOL> {
        Ok(true.into())
    }
}

#[test]
fn bounded_inventory_retains_credit_until_last_guarded_wrapper_drops()
-> Result<(), Box<dyn std::error::Error>> {
    let released = Arc::new(AtomicU64::new(0));
    let reservation =
        MemoryLease::new(NATIVE_CONSOLE_ACCESSIBILITY_BYTES, Box::new(Release(released.clone())));
    let claim = reservation.claim(NATIVE_CONSOLE_ACCESSIBILITY_BYTES)?;
    drop(reservation);
    let shared = Rc::new(Shared {
        hwnd: HWND::default(),
        // SAFETY: All fixture-owned interfaces are called synchronously on this test thread.
        thread: unsafe { GetCurrentThreadId() },
        painting: Rc::new(Cell::new(false)),
        closing: Cell::new(false),
        label: Cell::new("test"),
        root: Cell::new(std::ptr::null_mut()),
        child: Cell::new(std::ptr::null_mut()),
        leases: RefCell::new(storage::inventory()?),
        claim: RefCell::new(claim),
    });
    let source: IValueProvider = NativeValue.into();
    let native = shared.lease(source.cast()?)?;
    let value: IValueProvider =
        text::Value { shared: shared.clone(), native: native.clone() }.into();
    // SAFETY: The guarded wrapper delegates only to this deterministic external boundary fake.
    assert_eq!(unsafe { value.Value() }?.to_string(), "owned Unicode 😀");
    let mut held = Vec::with_capacity(storage::MAX_LEASES - 1);
    for _ in 1..storage::MAX_LEASES {
        held.push(shared.lease(source.cast()?)?);
    }
    assert_eq!(shared.leases.borrow().capacity(), storage::MAX_LEASES);
    assert_eq!(shared.leases.borrow().len(), storage::MAX_LEASES);
    assert_eq!(
        shared.lease(source.cast()?).err().ok_or("inventory exceeded")?.code(),
        E_OUTOFMEMORY
    );
    shared.closing.set(true);
    shared.revoke();
    assert!(native.0.borrow().is_none());
    assert!(held.iter().all(|lease| lease.0.borrow().is_none()));
    drop(held);
    drop(native);
    drop(source);
    drop(shared);
    assert_eq!(released.load(Ordering::Acquire), 0);
    // SAFETY: An escaped wrapper must reject after revocation while still retaining its credit.
    let retired_value = unsafe { value.Value() };
    assert_eq!(
        retired_value.err().ok_or("retired value remained live")?.code(),
        unavailable().code()
    );
    drop(value);
    assert_eq!(released.load(Ordering::Acquire), NATIVE_CONSOLE_ACCESSIBILITY_BYTES);
    Ok(())
}
