// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own one serialized DbgHelp session under genuine original Provider1MiB/4 authority.
//! DLL lookup is System32-only. Paths come only from the authenticated selected pair;
//! dump-derived names are never used for native lookup. Provider calls are serialized
//! by one process-wide session fence and owned mutable borrows, across finite workers.
mod api;
mod cause;
mod control;
mod lookup;
mod retire;
use api::Api;
use cause::ProviderCause;
pub use cause::{SymbolProviderFailure, SymbolProviderFailureCode};
use std::sync::atomic::{AtomicBool, Ordering};
use sugarsubstitute_contracts::{execution::NativeHandleOutput, incident::FailingFrame};
use sugarsubstitute_execution::{SymbolProviderAdmission, SymbolProviderOrigin};
static ACTIVE: AtomicBool = AtomicBool::new(false);
const PATH_WORDS: usize = 32768;
const BUFFER_BYTES: usize = 2 * PATH_WORDS * size_of::<u16>() + SYMBOL_WORDS * size_of::<u64>();
const SYMBOL_WORDS: usize =
    (size_of::<windows::Win32::System::Diagnostics::Debug::SYMBOL_INFOW>() + 1024).div_ceil(8);
struct Fields {
    api: Option<Api>,
    dll: Option<usize>,
    process: Option<usize>,
    session: bool,
    module: Option<u64>,
    outputs: [Option<NativeHandleOutput>; 4],
    first: Option<ProviderCause>,
    secondary: Option<ProviderCause>,
    retirement: Option<ProviderCause>,
    image: Vec<u16>,
    search: Vec<u16>,
    symbol: Vec<u64>,
    global: bool,
}
/// Keep actual provider outputs and its original1MiB claim through checked cleanup.
/// Construction runs only inside the genuine paired finite Main worker.
pub struct DbgHelpSymbolProvider {
    construction: Option<sugarsubstitute_contracts::execution::AdmissionError>,
    fields: Option<Box<Fields>>,
    original: Option<SymbolProviderOrigin>,
}
impl DbgHelpSymbolProvider {
    /// Consume actual Core admission before any allocation or native effect.
    /// # Errors
    /// Returns original admission in the rejected owner; denial creates no native output.
    pub fn new(admission: SymbolProviderAdmission) -> Result<Self, Self> {
        let original = admission.into_origin();
        let mut owner = Self { construction: None, fields: None, original: Some(original) };
        let Some(origin) = owner.original.as_ref() else {
            return Err(owner);
        };
        if let Err(cause) = origin.check_effect() {
            owner.construction = Some(cause);
            return Err(owner);
        }
        if origin.control_storage_bytes() + 3 * size_of::<Fields>() + BUFFER_BYTES + 65_536
            > 1_048_576
        {
            owner.construction =
                Some(sugarsubstitute_contracts::execution::AdmissionError::InvalidSymbolAdmission);
            return Err(owner);
        }
        owner.fields = Some(Box::new(Fields {
            api: None,
            dll: None,
            process: None,
            session: false,
            module: None,
            outputs: std::array::from_fn(|_| None),
            first: None,
            secondary: None,
            retirement: None,
            image: Vec::new(),
            search: Vec::new(),
            symbol: Vec::new(),
            global: false,
        }));
        if let Err(cause) = owner.prepare_buffers() {
            owner.record(cause);
            return Err(owner);
        }
        Ok(owner)
    }
    fn prepare_buffers(&mut self) -> Result<(), ProviderCause> {
        let fields = self.fields.as_mut().ok_or(ProviderCause::Identity)?;
        fields.image.try_reserve_exact(PATH_WORDS).map_err(ProviderCause::Reserve)?;
        fields.search.try_reserve_exact(PATH_WORDS).map_err(ProviderCause::Reserve)?;
        fields.symbol.try_reserve_exact(SYMBOL_WORDS).map_err(ProviderCause::Reserve)?;
        if fields.image.capacity() != PATH_WORDS
            || fields.search.capacity() != PATH_WORDS
            || fields.symbol.capacity() != SYMBOL_WORDS
        {
            return Err(ProviderCause::Capacity);
        }
        fields.image.resize(PATH_WORDS, 0);
        fields.search.resize(PATH_WORDS, 0);
        fields.symbol.resize(SYMBOL_WORDS, 0);
        self.original
            .as_ref()
            .ok_or(ProviderCause::Identity)?
            .check_effect()
            .map_err(ProviderCause::Control)
    }
    /// Borrow an actual pre-effect rejection retained inline before original admission.
    pub fn construction_failure(
        &self,
    ) -> Option<&sugarsubstitute_contracts::execution::AdmissionError> {
        self.construction.as_ref()
    }
    /// Borrow the genuine original target for paired close-only cleanup.
    pub fn original_origin(&self) -> Option<&SymbolProviderOrigin> {
        self.original.as_ref()
    }
    /// Borrow the first actual lookup/control failure without copying paths or source data.
    pub fn failure(&self) -> Option<SymbolProviderFailure<'_>> {
        self.fields.as_ref().and_then(|f| f.first.as_ref()).map(SymbolProviderFailure)
    }
    /// Borrow independent actual retirement failure; success cannot erase lookup failure.
    pub fn retirement_failure(&self) -> Option<SymbolProviderFailure<'_>> {
        self.fields.as_ref().and_then(|f| f.retirement.as_ref()).map(SymbolProviderFailure)
    }
    /// Borrow independent post-call control failure without replacing a native first cause.
    pub fn secondary_failure(&self) -> Option<SymbolProviderFailure<'_>> {
        self.fields.as_ref().and_then(|f| f.secondary.as_ref()).map(SymbolProviderFailure)
    }
    /// Inject only an external native status through the actual first-cause retention owner.
    #[cfg(test)]
    pub(crate) fn retain_native_fixture_failure(&mut self, cause: windows::core::Error) {
        self.record(ProviderCause::Native(cause));
    }
    fn record(&mut self, cause: ProviderCause) -> SymbolProviderFailureCode {
        let code = cause.code();
        if let Some(fields) = self.fields.as_mut()
            && fields.first.is_none()
        {
            fields.first = Some(cause);
        }
        code
    }
}
/// Retain the actual result allocation under the same original provider charge.
/// Fields drop before the original shared control/claim; no Clone or extraction escapes.
pub struct SymbolFrameObservation {
    frame: FailingFrame,
    _original: SymbolProviderOrigin,
}
impl SymbolFrameObservation {
    /// Observe the real original Main owner and effective cutoff after native work and cleanup.
    /// This readonly correlation performs no Running, effect, clock or cancellation check.
    pub(crate) fn original_admission(
        &self,
    ) -> Result<
        (
            sugarsubstitute_contracts::execution::ExecutionId,
            sugarsubstitute_contracts::execution::Moment,
        ),
        sugarsubstitute_contracts::execution::AdmissionError,
    > {
        let original = self._original.original_context()?;
        let until = original
            .deadline()
            .ok_or(sugarsubstitute_contracts::execution::AdmissionError::InvalidSymbolAdmission)?;
        Ok((original.owner_id(), until))
    }
    /// Borrow the exact resolved frame and authenticated release association.
    pub const fn frame(&self) -> &FailingFrame {
        &self.frame
    }
}
impl Drop for DbgHelpSymbolProvider {
    fn drop(&mut self) {
        if self.fields.as_ref().is_some_and(|f| {
            f.dll.is_some() || f.process.is_some() || f.session || f.module.is_some()
        }) {
            // Unresolved provider effects retain the identical capsule, four credits,
            // original claim and global fence until this process exits.
            if let Some(fields) = self.fields.take() {
                std::mem::forget(fields);
            }
            if let Some(origin) = self.original.take() {
                std::mem::forget(origin);
            }
        } else if self.fields.as_ref().is_some_and(|f| f.global) {
            // No native output exists; dropping pre-effect or failed-load bookkeeping is passive.
            ACTIVE.store(false, Ordering::Release);
        }
    }
}
const _: () = assert!(3 * size_of::<Fields>() + BUFFER_BYTES + 65_536 < 1_048_576);

impl std::fmt::Debug for DbgHelpSymbolProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DbgHelpSymbolProvider")
    }
}
impl std::fmt::Display for DbgHelpSymbolProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Symbol provider retained failure")
    }
}
impl std::error::Error for DbgHelpSymbolProvider {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Some(cause) = &self.construction {
            return Some(cause);
        }
        let fields = self.fields.as_ref()?;
        fields
            .first
            .as_ref()
            .or(fields.retirement.as_ref())
            .map(|cause| cause as &(dyn std::error::Error + 'static))
    }
}
