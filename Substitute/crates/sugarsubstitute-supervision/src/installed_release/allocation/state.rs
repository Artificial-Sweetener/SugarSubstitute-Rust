// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep pure counters before the original Core charge across every final control free.

use super::*;
use std::sync::{Arc, Mutex, atomic::AtomicUsize};
use sugarsubstitute_execution::CaptureBundleOrigin;

pub(super) struct Counters {
    paths: [bool; PATH_SLOTS],
    scratch: bool,
    metadata: usize,
    ingress: usize,
    causes: [usize; CAUSE_SLOTS],
    closed: bool,
}

/// Share only accounting identity; original Core still checks every native effect.
pub(crate) struct AllocationOrigin {
    state: Option<Arc<Mutex<Counters>>>,
    original: crate::installed_release::ReleaseAuthority,
}

pub(in crate::installed_release) struct CaptureOriginConstructionFailure {
    pub(in crate::installed_release) cause: CaptureCapacityFailure,
    pub(in crate::installed_release) original: CaptureBundleOrigin,
}

pub(crate) struct ReleaseOriginConstructionFailure {
    pub(crate) cause: CaptureCapacityFailure,
    pub(crate) _original: crate::installed_release::ReleaseAuthority,
}

#[repr(C)]
struct CounterControl {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: Mutex<Counters>,
}

impl Clone for AllocationOrigin {
    fn clone(&self) -> Self {
        Self { state: self.state.clone(), original: self.original.clone() }
    }
}
impl Drop for AllocationOrigin {
    fn drop(&mut self) {
        if let Some(state) = self.state.take() {
            let _completed = Arc::into_inner(state);
        }
    }
}

impl AllocationOrigin {
    /// Consume the already preflighted original claim; allocate only this counter control.
    pub(in crate::installed_release) fn new(
        original: CaptureBundleOrigin,
    ) -> Result<Self, CaptureOriginConstructionFailure> {
        let retained = original.clone();
        Self::from_authority(crate::installed_release::ReleaseAuthority::Capture(original)).map_err(
            |failure| CaptureOriginConstructionFailure { cause: failure.cause, original: retained },
        )
    }
    pub(crate) fn from_authority(
        original: crate::installed_release::ReleaseAuthority,
    ) -> Result<Self, ReleaseOriginConstructionFailure> {
        let Some(metadata) = original
            .control_storage_bytes()
            .checked_add(size_of::<CounterControl>())
            .filter(|metadata| *metadata <= METADATA_BYTES)
        else {
            return Err(ReleaseOriginConstructionFailure {
                cause: CaptureCapacityFailure::new(CapacityFailureKind::Layout),
                _original: original,
            });
        };
        Ok(Self {
            state: Some(Arc::new(Mutex::new(Counters {
                paths: [false; PATH_SLOTS],
                scratch: false,
                metadata,
                ingress: 0,
                causes: [0; CAUSE_SLOTS],
                closed: false,
            }))),
            original,
        })
    }

    pub(crate) const fn control_bytes() -> usize {
        size_of::<CounterControl>()
    }

    /// Borrow the sole reconstructed Core origin without exposing its claim or Arc.
    pub(crate) fn original(&self) -> &crate::installed_release::ReleaseAuthority {
        &self.original
    }

    /// Compare inert allocation identity; this check cannot authorize an OS operation.
    pub(crate) fn matches(&self, other: &Self) -> bool {
        match (&self.state, &other.state) {
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }

    /// Close new region admission while existing loans remain owned for original cleanup.
    pub(crate) fn close(&self) -> Result<(), CaptureCapacityFailure> {
        let state =
            self.state.as_ref().ok_or(CaptureCapacityFailure::new(CapacityFailureKind::Missing))?;
        let mut state =
            state.lock().map_err(|_| CaptureCapacityFailure::new(CapacityFailureKind::Closed))?;
        state.closed = true;
        Ok(())
    }

    pub(crate) fn reserve(
        &self,
        region: CaptureRegion,
        bytes: usize,
    ) -> Result<CaptureLoan, CaptureCapacityFailure> {
        let state =
            self.state.as_ref().ok_or(CaptureCapacityFailure::new(CapacityFailureKind::Missing))?;
        let mut state =
            state.lock().map_err(|_| CaptureCapacityFailure::new(CapacityFailureKind::Closed))?;
        if state.closed {
            return Err(CaptureCapacityFailure::new(CapacityFailureKind::Closed));
        }
        let accepted = match region {
            CaptureRegion::Path(slot) => {
                state.paths.get_mut(usize::from(slot)).is_some_and(|held| {
                    if *held || bytes > PATH_BYTES {
                        false
                    } else {
                        *held = true;
                        true
                    }
                })
            }
            CaptureRegion::Scratch => {
                if state.scratch || bytes > SCRATCH_BYTES {
                    false
                } else {
                    state.scratch = true;
                    true
                }
            }
            CaptureRegion::Metadata => state.metadata.checked_add(bytes).is_some_and(|next| {
                if next > METADATA_BYTES {
                    false
                } else {
                    state.metadata = next;
                    true
                }
            }),
            CaptureRegion::Ingress => state.ingress.checked_add(bytes).is_some_and(|next| {
                if next > INGRESS_BYTES {
                    false
                } else {
                    state.ingress = next;
                    true
                }
            }),
            CaptureRegion::Cause(slot) => {
                let fits =
                    state.ingress.checked_add(bytes).is_some_and(|next| next <= INGRESS_BYTES);
                if fits && state.causes.get(usize::from(slot)).is_some_and(|held| *held == 0) {
                    state.causes[usize::from(slot)] = bytes;
                    state.ingress += bytes;
                    true
                } else {
                    false
                }
            }
        };
        drop(state);
        if accepted {
            Ok(CaptureLoan { region, bytes, original: Some(self.clone()) })
        } else {
            Err(CaptureCapacityFailure::new(CapacityFailureKind::Occupied))
        }
    }

    pub(super) fn release(&self, region: CaptureRegion, bytes: usize) -> bool {
        let Some(state) = self.state.as_ref() else { return false };
        let Ok(mut state) = state.lock() else { return false };
        match region {
            CaptureRegion::Path(slot) => match state.paths.get_mut(usize::from(slot)) {
                Some(held) if *held => *held = false,
                _ => return false,
            },
            CaptureRegion::Scratch if state.scratch => state.scratch = false,
            CaptureRegion::Scratch => return false,
            CaptureRegion::Metadata => match state.metadata.checked_sub(bytes) {
                Some(next) => state.metadata = next,
                None => return false,
            },
            CaptureRegion::Ingress => match state.ingress.checked_sub(bytes) {
                Some(next) => state.ingress = next,
                None => return false,
            },
            CaptureRegion::Cause(slot) => {
                if !state
                    .causes
                    .get(usize::from(slot))
                    .is_some_and(|held| *held == bytes && bytes != 0)
                {
                    return false;
                }
                let Some(next) = state.ingress.checked_sub(bytes) else { return false };
                state.causes[usize::from(slot)] = 0;
                state.ingress = next;
            }
        }
        true
    }
}
