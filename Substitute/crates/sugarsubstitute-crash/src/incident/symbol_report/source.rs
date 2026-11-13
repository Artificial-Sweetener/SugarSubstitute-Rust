// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Park actual partial capsules outside the metadata lock across worker unwinds.
use super::{SymbolReportRequest, cause::SymbolReportCause};
use crate::incident::{
    DbgHelpSymbolProvider, IncidentDumpReadLease, IncidentDumpReader, SymbolFrameObservation,
};
use std::sync::{Arc, Mutex, atomic::AtomicUsize};
use sugarsubstitute_contracts::execution::MemoryClaim;
use sugarsubstitute_execution::SymbolProviderOrigin;
use sugarsubstitute_supervision::{
    committed_reader::CommittedDiagnosticReader,
    installed_release::{SymbolBundleLease, SymbolBundleOwner},
};
pub(super) struct Data {
    pub(super) provider: Option<DbgHelpSymbolProvider>,
    pub(super) native_reader: Option<CommittedDiagnosticReader>,
    pub(super) reader: Option<IncidentDumpReader>,
    pub(super) dump: Option<IncidentDumpReadLease>,
    pub(super) selected_owner: Option<SymbolBundleOwner>,
    pub(super) selected: Option<SymbolBundleLease>,
    pub(super) observation: Option<SymbolFrameObservation>,
    pub(super) first: Option<SymbolReportCause>,
    pub(super) retirement: Option<SymbolReportCause>,
    pub(super) settled: bool,
}
pub(super) struct DataOwner {
    fields: Box<Data>,
    // The complete Box allocation drops before this same original charge.
    _original: SymbolProviderOrigin,
}
impl std::ops::Deref for DataOwner {
    type Target = Data;
    fn deref(&self) -> &Data {
        &self.fields
    }
}
impl std::ops::DerefMut for DataOwner {
    fn deref_mut(&mut self) -> &mut Data {
        &mut self.fields
    }
}
impl DataOwner {
    pub(super) fn new(
        provider: DbgHelpSymbolProvider,
        original: SymbolProviderOrigin,
    ) -> DataOwner {
        DataOwner {
            fields: Box::new(Data {
                provider: Some(provider),
                native_reader: None,
                reader: None,
                dump: None,
                selected_owner: None,
                selected: None,
                observation: None,
                first: None,
                retirement: None,
                settled: false,
            }),
            _original: original,
        }
    }
}
impl Data {
    pub(super) fn fail(&mut self, cause: SymbolReportCause) {
        // Main stops immediately at its first failure. Cleanup owns a distinct slot.
        self.first = Some(cause);
    }
}
pub(super) struct State {
    pub(super) request: Option<SymbolReportRequest>,
    pub(super) identity: Option<(
        sugarsubstitute_contracts::incident::IncidentKey,
        sugarsubstitute_contracts::incident::IncidentRevision,
        sugarsubstitute_contracts::incident::DumpId,
    )>,
    pub(super) data: Option<DataOwner>,
    pub(super) causes: [Option<SymbolReportCause>; 4],
    pub(super) started: bool,
    pub(super) before_effects: bool,
    pub(super) settled: bool,
    pub(super) taken: bool,
}
struct Allocation {
    state: Mutex<State>,
    _claim: MemoryClaim,
}
#[repr(C)]
struct Control {
    strong: AtomicUsize,
    weak: AtomicUsize,
    allocation: Allocation,
}
/// Closed metadata aliases keep the original4KiB claim after jobs and service Drop.
pub(super) struct Source(Option<Arc<Allocation>>);
impl Clone for Source {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl Drop for Source {
    fn drop(&mut self) {
        if let Some(value) = self.0.take() {
            drop(Arc::into_inner(value));
        }
    }
}
impl Source {
    pub(super) fn new(claim: MemoryClaim) -> Self {
        Self(Some(Arc::new(Allocation {
            state: Mutex::new(State {
                request: None,
                identity: None,
                data: None,
                causes: [None, None, None, None],
                started: false,
                before_effects: true,
                settled: false,
                taken: false,
            }),
            _claim: claim,
        })))
    }
    pub(super) fn with<T>(&self, action: impl FnOnce(&mut State) -> T) -> Option<T> {
        let allocation = self.0.as_ref()?;
        let mut state = match allocation.state.lock() {
            Ok(state) => state,
            Err(error) => error.into_inner(),
        };
        Some(action(&mut state))
    }
    pub(super) fn update(&self, action: impl FnOnce(&mut State)) {
        if let Some(allocation) = &self.0 {
            let mut state = match allocation.state.lock() {
                Ok(state) => state,
                Err(error) => error.into_inner(),
            };
            action(&mut state);
        }
    }
    pub(super) fn fail(&self, cause: SymbolReportCause) {
        self.update(|state| {
            if let Some(slot) = state.causes.iter_mut().find(|slot| slot.is_none()) {
                *slot = Some(cause);
            }
        });
    }
}
pub(super) struct Parking {
    pub(super) data: Option<DataOwner>,
    source: Source,
}
impl Parking {
    pub(super) fn new(source: &Source, data: DataOwner) -> Self {
        Self { data: Some(data), source: source.clone() }
    }
}
impl Drop for Parking {
    fn drop(&mut self) {
        if let Some(data) = self.data.take() {
            self.source.update(|state| state.data = Some(data));
        }
    }
}
// Include two original bounded PanicCause texts and all closed metadata aliases.
const _: () = assert!(
    size_of::<Control>()
        + 2048
        + 8 * size_of::<Source>()
        + size_of::<super::service::SymbolReportResult>()
        <= 4096
);
// Provider1MiB reserves this exact closed host tail before its native buffer allocations.
const _: () = assert!(size_of::<Data>() + 3 * size_of::<Box<Data>>() <= 65536);
