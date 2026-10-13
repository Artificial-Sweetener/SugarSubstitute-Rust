// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Loan one original preclaimed capsule while every provider and finite native call runs unlocked.
use super::desktop_driver::{invalid, loader::Api};
use crate::capture::lock;
use std::sync::Mutex;
use sugarsubstitute_contracts::execution::MemoryClaim;
use sugarsubstitute_execution::windows::NativeObservationError;
use sugarsubstitute_execution::{CaptureBundleAdmission, CaptureBundleOrigin};
use sugarsubstitute_supervision::installed_release::{
    CaptureBundleLease, CaptureBundleOwner, CaptureBundleParts,
};

/// All potential output cells exist before selection, loading or callback installation.
pub(super) struct Fields {
    pub(super) admission: Option<CaptureBundleAdmission>,
    pub(super) original: Option<CaptureBundleOrigin>,
    pub(super) selected: Option<CaptureBundleOwner>,
    pub(super) lease: Option<CaptureBundleLease>,
    pub(super) parts: Option<CaptureBundleParts>,
    pub(super) loaded: usize,
    pub(super) pinned: usize,
    pub(super) pin_uncertain: bool,
    pub(super) api: Option<Api>,
    pub(super) build: [u8; 64],
    pub(super) build_length: usize,
}
impl Fields {
    /// Count the complete owner and maximum admitted UTF16 loader scratch before allocation.
    pub(super) const fn storage_bytes() -> usize {
        size_of::<Self>() + size_of::<Capsule>() + 65_536
    }
    /// Only real checked no-output/closure permits the original outside claims to return.
    fn closed(&self) -> bool {
        self.selected.is_none()
            && self.lease.is_none()
            && self.parts.is_none()
            && self.loaded == 0
            && self.pinned == 0
            && !self.pin_uncertain
    }
}
/// Box storage is destroyed before either outside original claim can return.
struct Capsule {
    fields: Option<Box<Fields>>,
    native: Option<MemoryClaim>,
    callback: Option<MemoryClaim>,
}
impl Drop for Capsule {
    /// Retain the entire actual capsule before diagnostics if any output remains uncertain.
    fn drop(&mut self) {
        if self.fields.as_ref().is_some_and(|fields| !fields.closed()) {
            if let Some(fields) = self.fields.take() {
                std::mem::forget(fields)
            }
            if let Some(native) = self.native.take() {
                std::mem::forget(native)
            }
            if let Some(callback) = self.callback.take() {
                std::mem::forget(callback)
            }
        }
    }
}
struct State {
    owner: Option<Capsule>,
    legacy: Option<LegacyPin>,
    legacy_bytes: u64,
    legacy_reserved: u64,
    borrowed: bool,
    api: Option<Api>,
    bytes: u64,
    build: [u8; 64],
    build_length: usize,
}
/// A slot is occupied while its sole actual worker holds the original loan.
pub(super) struct Slot(Mutex<State>);
impl Slot {
    /// Establish empty inert storage without allocating or invoking native code.
    pub(super) const fn new() -> Self {
        Self(Mutex::new(State {
            owner: None,
            legacy: None,
            legacy_bytes: 0,
            legacy_reserved: 0,
            borrowed: false,
            api: None,
            bytes: 0,
            build: [0; 64],
            build_length: 0,
        }))
    }
    /// Copy only already adopted process-pinned function pointers; no provider/FFI runs.
    pub(super) fn api(&self) -> Option<Api> {
        lock(&self.0).api
    }
    /// Copy only already selected bounded build facts with their installed ABI.
    pub(super) fn binding(&self) -> Option<(Api, [u8; 64], usize)> {
        let state = lock(&self.0);
        state.api.map(|api| (api, state.build, state.build_length))
    }
    /// Observe only original v1 retained callback claims, including accepted PIN uncertainty.
    pub(super) fn legacy_bytes(&self) -> u64 {
        lock(&self.0).legacy_bytes
    }
    /// Observe the actual whole original reservation retained by the v1 module owner.
    pub(super) fn legacy_reservation(&self) -> u64 {
        lock(&self.0).legacy_reserved
    }
    /// Observe unique original retained claims; this is not new admission or retirement proof.
    pub(super) fn retained_bytes(&self) -> u64 {
        lock(&self.0).bytes
    }
}
pub(super) static SLOT: Slot = Slot::new();
/// Restore the identical owner after every returned error or unwinding boundary.
pub(super) struct Loan<'a> {
    slot: &'a Slot,
    owner: Option<Capsule>,
}
impl<'a> Loan<'a> {
    /// Install all output cells and actual original claims before selection may create outputs.
    pub(super) fn install(
        slot: &'a Slot,
        admission: CaptureBundleAdmission,
        native: MemoryClaim,
        callback: MemoryClaim,
    ) -> Result<Self, NativeObservationError> {
        let fields = Box::new(Fields {
            admission: Some(admission),
            original: None,
            selected: None,
            lease: None,
            parts: None,
            loaded: 0,
            pinned: 0,
            pin_uncertain: false,
            api: None,
            build: [0; 64],
            build_length: 0,
        });
        let owner =
            Capsule { fields: Some(fields), native: Some(native), callback: Some(callback) };
        let mut state = lock(&slot.0);
        if state.owner.is_some() || state.legacy.is_some() || state.borrowed {
            drop(state);
            drop(owner);
            return Err(invalid("capture_driver_occupied"));
        }
        state.borrowed = true;
        drop(state);
        Ok(Self { slot, owner: Some(owner) })
    }
    /// Take exclusive actual custody without holding the slot guard across external work.
    pub(super) fn take(slot: &'a Slot) -> Result<Option<Self>, NativeObservationError> {
        let mut state = lock(&slot.0);
        if state.borrowed {
            return Err(invalid("capture_driver_borrowed"));
        }
        let Some(owner) = state.owner.take() else { return Ok(None) };
        state.borrowed = true;
        drop(state);
        Ok(Some(Self { slot, owner: Some(owner) }))
    }
    /// Borrow concrete output cells only after the slot's lifetime guard has been released.
    pub(super) fn fields(&mut self) -> Result<&mut Fields, NativeObservationError> {
        self.owner
            .as_mut()
            .and_then(|owner| owner.fields.as_deref_mut())
            .ok_or_else(|| invalid("capture_driver_loan"))
    }
    /// Free actual closed boxed storage before releasing its original outside claims.
    pub(super) fn return_closed(mut self) -> Result<(), NativeObservationError> {
        if !self.owner.as_ref().and_then(|o| o.fields.as_ref()).is_some_and(|f| f.closed()) {
            return Err(invalid("capture_driver_not_closed"));
        }
        drop(self.owner.take());
        let mut state = lock(&self.slot.0);
        state.borrowed = false;
        state.api = None;
        state.bytes = 0;
        Ok(())
    }
}
impl Drop for Loan<'_> {
    /// Restore every partial output before status, panic or another observation can discard it.
    fn drop(&mut self) {
        if let Some(owner) = self.owner.take() {
            let api = owner.fields.as_ref().and_then(|f| f.api);
            let (build, build_length) =
                owner.fields.as_ref().map_or(([0; 64], 0), |f| (f.build, f.build_length));
            let bytes = owner.native.as_ref().map_or(0, MemoryClaim::bytes)
                + owner.callback.as_ref().map_or(0, MemoryClaim::bytes)
                + owner
                    .fields
                    .as_ref()
                    .and_then(|f| f.original.as_ref())
                    .map_or(0, CaptureBundleOrigin::retained_bytes);
            let mut state = lock(&self.slot.0);
            state.owner = Some(owner);
            state.borrowed = false;
            state.api = api;
            state.bytes = bytes;
            state.build = build;
            state.build_length = build_length;
        }
    }
}

/// Actual v1 loaded module and callback claim; no native close occurs on Drop.
struct LegacyPin {
    module: usize,
    pin_issued: bool,
    memory: MemoryClaim,
    bundle: Option<Box<CaptureBundleParts>>,
}
/// One actual module owner across both public ABI generations.
/// Finite calls run without its slot mutex; every partial output survives unwind.
pub(super) struct LegacyLoan<'a> {
    slot: &'a Slot,
    owner: Option<LegacyPin>,
}
impl<'a> LegacyLoan<'a> {
    /// Claim the same occupied process slot before the first actual loader effect.
    /// Rejection returns the identical callback claim without allocating another owner.
    pub(super) fn reserve(slot: &'a Slot, memory: MemoryClaim) -> Result<Self, MemoryClaim> {
        let mut state = lock(&slot.0);
        if state.owner.is_some() || state.legacy.is_some() || state.borrowed {
            drop(state);
            return Err(memory);
        }
        state.borrowed = true;
        drop(state);
        Ok(Self {
            slot,
            owner: Some(LegacyPin { module: 0, pin_issued: false, memory, bundle: None }),
        })
    }
    /// Move the identical verified file/module capsule into durable custody before effects.
    pub(super) fn install_bundle(
        &mut self,
        bundle: Box<CaptureBundleParts>,
    ) -> Result<(), Box<CaptureBundleParts>> {
        let Some(owner) = self.owner.as_mut() else { return Err(bundle) };
        if owner.bundle.is_some() {
            return Err(bundle);
        }
        owner.bundle = Some(bundle);
        Ok(())
    }
    /// Borrow the already verified capsule without holding the process slot mutex.
    pub(super) fn bundle(&mut self) -> Option<&mut CaptureBundleParts> {
        self.owner.as_mut().and_then(|owner| owner.bundle.as_deref_mut())
    }
    /// Adopt the actual LoadLibrary output before any status/symbol/PIN operation.
    pub(super) fn adopt_module(&mut self, module: usize) {
        if let Some(owner) = self.owner.as_mut() {
            owner.module = module;
        }
    }
    /// Latch possible permanent effects BEFORE the actual PIN call begins.
    /// Zero/error output cannot later manufacture no-effect or unload proof.
    pub(super) fn pin_attempt(&mut self) {
        if let Some(owner) = self.owner.as_mut() {
            owner.pin_issued = true;
            let mut state = lock(&self.slot.0);
            state.legacy_bytes = owner.memory.bytes()
                + owner
                    .bundle
                    .as_ref()
                    .map_or(0, |bundle| bundle.retention.original_origin().retained_bytes());
            state.legacy_reserved = owner.memory.reserved_bytes();
        }
    }
    /// Return only a genuine no-output failed load, outside the process slot mutex.
    pub(super) fn return_unused(
        mut self,
    ) -> Option<(MemoryClaim, Option<Box<CaptureBundleParts>>)> {
        if self.owner.as_ref().is_none_or(|owner| owner.module != 0 || owner.pin_issued) {
            return None;
        }
        let memory = self.owner.take().map(|owner| (owner.memory, owner.bundle));
        let mut state = lock(&self.slot.0);
        state.borrowed = false;
        drop(state);
        memory
    }
}
impl Drop for LegacyLoan<'_> {
    /// Restore the original actual module/callback owner after status or unwind.
    fn drop(&mut self) {
        if let Some(owner) = self.owner.take() {
            let mut state = lock(&self.slot.0);
            state.legacy = Some(owner);
            state.borrowed = false;
        }
    }
}

#[cfg(test)]
#[path = "module_pin_tests.rs"]
mod tests;
