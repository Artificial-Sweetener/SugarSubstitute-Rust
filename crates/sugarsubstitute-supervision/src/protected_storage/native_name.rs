// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain bounded native names and SID data through their final shared controls.
use crate::{
    ClaimError,
    diagnostic_storage::{
        DiagnosticNativeGrant, DiagnosticNativeNameLoan, DiagnosticNativeOwnerLoan,
        DiagnosticNativeSidLoan,
    },
    native_claims::files::{NativeLayoutFailure, NativeRoleLayout},
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicUsize},
};

// Pinned std ArcInner representation: both counts precede its actual concrete payload.
#[repr(C)]
struct ArcStorage<T> {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: T,
}

/// Own ordinary names or an alias of the same admitted diagnostic name storage.
#[derive(Clone)]
pub(crate) enum NativeName {
    Ordinary(PathBuf),
    Retained(RetainedNativeName),
    Selected(crate::installed_release::CaptureNativeName),
    SelectedTransient(crate::installed_release::CaptureTransientName),
}
/// Hold the final name allocation before the canonical decoder can produce bytes.
pub(crate) enum NativeNameDestination {
    Ordinary,
    SelectedTransient {
        allocation: crate::installed_release::SharedCaptureLoan,
        original: crate::installed_release::AllocationOrigin,
    },
    Diagnostic {
        data: DiagnosticNativeNameLoan,
        control: DiagnosticNativeOwnerLoan,
        original: DiagnosticNativeGrant,
    },
}
impl NativeNameDestination {
    /// Build one child name directly into its held destination, without a joined temporary.
    pub(crate) fn join(
        self,
        path: &Path,
        child: &std::ffi::OsStr,
        phase: &mut super::native_phase::NativePhase<'_>,
    ) -> Result<NativeName, ClaimError> {
        self.produce(path, Some(child), phase)
    }
    fn produce(
        self,
        path: &Path,
        child: Option<&std::ffi::OsStr>,
        phase: &mut super::native_phase::NativePhase<'_>,
    ) -> Result<NativeName, ClaimError> {
        phase.check_destination(&self)?;
        match self {
            Self::Ordinary => {
                let mut owned = path.to_owned();
                if let Some(child) = child {
                    owned.push(child);
                }
                Ok(NativeName::Ordinary(owned))
            }
            Self::SelectedTransient { .. } => Err(ClaimError::capture_allocation(
                crate::installed_release::CaptureCapacityFailure::identity(),
            )),
            Self::Diagnostic { data, control, original } => {
                if path
                    .as_os_str()
                    .len()
                    .checked_add(child.map_or(0, |child| child.len() + 1))
                    .is_none_or(|bytes| bytes > 192 * 1024)
                {
                    return Err(ClaimError::diagnostic_allocation(original.reject_native_state()));
                }
                let mut owned = PathBuf::new();
                owned.try_reserve_exact(192 * 1024).map_err(|cause| {
                    phase.error(
                        "native_name_reservation",
                        crate::claim_error::BoundedNativeCause::Reserve(cause),
                    )
                })?;
                if owned.capacity() > 192 * 1024 {
                    return Err(ClaimError::diagnostic_allocation(original.reject_native_state()));
                }
                owned.push(path);
                if let Some(child) = child {
                    owned.push(child);
                }
                Self::Diagnostic { data, control, original }.finish(owned)
            }
        }
    }
    /// Compare an inert original; this observation cannot authorize a native effect.
    pub(crate) fn original(&self) -> Option<&DiagnosticNativeGrant> {
        match self {
            Self::Ordinary | Self::SelectedTransient { .. } => None,
            Self::Diagnostic { original, .. } => Some(original),
        }
    }
    /// Transfer the produced data into its preadmitted outside-retained name control.
    pub(crate) fn finish(self, path: PathBuf) -> Result<NativeName, ClaimError> {
        match self {
            Self::Ordinary => Ok(NativeName::Ordinary(path)),
            Self::SelectedTransient { allocation, original } => {
                crate::installed_release::CaptureTransientName::new(path, allocation, original)
                    .map(NativeName::SelectedTransient)
                    .map_err(ClaimError::capture_allocation)
            }
            Self::Diagnostic { data, control, original } => {
                NativeName::retained(&mut Some((path, data, control, original)))
            }
        }
    }
}
pub(crate) struct RetainedNativeName {
    data: Option<Arc<NativeNameData>>,
    depth: Option<usize>,
    original: DiagnosticNativeGrant,
}
struct NativeNameData {
    path: PathBuf,
    _allocation: DiagnosticNativeNameLoan,
    _control: DiagnosticNativeOwnerLoan,
}
impl NativeName {
    /// Observe the closed ordinary branch before any ordinary owner allocation.
    pub(crate) fn is_ordinary(&self) -> bool {
        matches!(self, Self::Ordinary(_))
    }
    /// Return an ordinary owned name without copying or projecting retained data.
    pub(crate) fn into_ordinary(self) -> Result<PathBuf, ClaimError> {
        match self {
            Self::Ordinary(path) => Ok(path),
            Self::Retained(name) => {
                Err(ClaimError::diagnostic_allocation(name.original.reject_native_state()))
            }
            Self::Selected(_) | Self::SelectedTransient(_) => Err(ClaimError::capture_allocation(
                crate::installed_release::CaptureCapacityFailure::identity(),
            )),
        }
    }
    /// Observe the actual owned destination capacity without copying its name.
    pub(crate) fn capacity(&self) -> Result<usize, ClaimError> {
        match self {
            Self::Ordinary(path) => Ok(path.capacity()),
            Self::Selected(name) => name.capacity().map_err(ClaimError::capture_allocation),
            Self::SelectedTransient(name) => {
                name.capacity().map_err(ClaimError::capture_allocation)
            }
            Self::Retained(name) => {
                name.data.as_ref().map(|data| data.path.capacity()).ok_or_else(|| {
                    ClaimError::diagnostic_allocation(name.original.reject_native_state())
                })
            }
        }
    }

    /// Compare the sealed selected allocation without granting a filesystem effect.
    pub(crate) fn matches_selected(
        &self,
        original: &crate::installed_release::AllocationOrigin,
    ) -> bool {
        matches!(self,Self::Selected(name) if name.original().matches(original))
    }
    /// Compare inert accounting provenance with the actual cell admission.
    pub(crate) fn matches_original(&self, original: &DiagnosticNativeGrant) -> bool {
        matches!(self, Self::Retained(name) if name.original.matches_original(original))
    }

    /// Borrow the produced name and its original loans until success consumes them.
    /// # Errors
    /// Rejection leaves the exact path, original and data/control loans in the caller slot.
    pub(crate) fn retained(
        input: &mut Option<(
            PathBuf,
            DiagnosticNativeNameLoan,
            DiagnosticNativeOwnerLoan,
            DiagnosticNativeGrant,
        )>,
    ) -> Result<Self, ClaimError> {
        let (path, _, _, original) = input.as_ref().ok_or_else(|| {
            ClaimError::capture_allocation(
                crate::installed_release::CaptureCapacityFailure::missing(),
            )
        })?;
        if path.capacity() > 192 * 1024 {
            return Err(ClaimError::diagnostic_allocation(original.reject_native_state()));
        }
        let (path, allocation, control, original) = input.take().ok_or_else(|| {
            ClaimError::capture_allocation(
                crate::installed_release::CaptureCapacityFailure::missing(),
            )
        })?;
        Ok(Self::Retained(RetainedNativeName {
            data: Some(Arc::new(NativeNameData {
                path,
                _allocation: allocation,
                _control: control,
            })),
            depth: None,
            original,
        }))
    }
    /// Borrow the actual retained path or a rooted ancestor without copying it.
    /// # Errors
    /// Rejects an absent control, out-of-range ancestor or incomplete prefix.
    pub(crate) fn path(&self) -> Result<&Path, ClaimError> {
        match self {
            Self::Ordinary(path) => Ok(path),
            Self::Selected(name) => name.path().map_err(ClaimError::capture_allocation),
            Self::SelectedTransient(name) => name.path().map_err(ClaimError::capture_allocation),
            Self::Retained(name) => {
                let data = name.data.as_ref().ok_or_else(|| {
                    ClaimError::diagnostic_allocation(name.original.reject_native_state())
                })?;
                let path = match name.depth {
                    None => data.path.as_path(),
                    Some(depth) => data.path.ancestors().nth(depth).ok_or_else(|| {
                        ClaimError::diagnostic_allocation(name.original.reject_native_state())
                    })?,
                };
                if !path.has_root()
                    || matches!(
                        path.components().next_back(),
                        Some(std::path::Component::Prefix(_))
                    )
                {
                    return Err(ClaimError::diagnostic_allocation(
                        name.original.reject_native_state(),
                    ));
                }
                Ok(path)
            }
        }
    }
    /// Describe its concrete shared payload/control and outside alias wrapper.
    pub(crate) fn diagnostic_control_layout() -> Result<NativeRoleLayout, NativeLayoutFailure> {
        let bytes = std::alloc::Layout::new::<ArcStorage<NativeNameData>>()
            .size()
            .checked_add(size_of::<RetainedNativeName>())
            .ok_or(NativeLayoutFailure::Overflow)?;
        Ok(NativeRoleLayout { retained: bytes, peak: bytes })
    }
    /// Describe the sole fixed data destination before any path copy.
    pub(crate) fn diagnostic_owned_layout() -> Result<NativeRoleLayout, NativeLayoutFailure> {
        Ok(NativeRoleLayout { retained: 192 * 1024, peak: 192 * 1024 })
    }
}
impl Clone for RetainedNativeName {
    fn clone(&self) -> Self {
        Self { data: self.data.clone(), depth: self.depth, original: self.original.clone() }
    }
}
impl Drop for RetainedNativeName {
    fn drop(&mut self) {
        if let Some(data) = self.data.take() {
            // Every alias consumes into_inner: the final control is freed before
            // the moved payload/path/loans, while this outside original remains.
            drop(Arc::into_inner(data));
        }
    }
}

/// Own ordinary SID bytes or the original admitted diagnostic shared storage.
#[derive(Clone)]
pub(crate) enum NativeSid {
    Ordinary(Vec<u8>),
    Retained(SharedNativeSid),
    Selected(crate::installed_release::CaptureNativeSid),
}
/// Preadmit the final SID bytes and their actual shared control before copying.
pub(crate) enum NativeSidDestination {
    Ordinary,
    Selected(crate::installed_release::CaptureSidDestination),
    Diagnostic { allocation: DiagnosticNativeSidLoan, original: DiagnosticNativeGrant },
}
impl NativeSidDestination {
    /// Borrow inert original identity before the native query begins.
    pub(crate) fn original(&self) -> Option<&DiagnosticNativeGrant> {
        match self {
            Self::Ordinary | Self::Selected(_) => None,
            Self::Diagnostic { original, .. } => Some(original),
        }
    }
    /// Transfer actual copied SID bytes into their already held final destination.
    pub(crate) fn finish(self, bytes: Vec<u8>) -> Result<NativeSid, ClaimError> {
        match self {
            Self::Ordinary => Ok(NativeSid::Ordinary(bytes)),
            Self::Selected(destination) => destination
                .finish(bytes)
                .map(NativeSid::Selected)
                .map_err(ClaimError::capture_allocation),
            Self::Diagnostic { allocation, original } => {
                NativeSid::retained(&mut Some((bytes, allocation, original)))
            }
        }
    }
}
pub(crate) struct SharedNativeSid {
    data: Option<Arc<NativeSidData>>,
    original: DiagnosticNativeGrant,
}
struct NativeSidData {
    bytes: Vec<u8>,
    _allocation: DiagnosticNativeSidLoan,
}
impl NativeSid {
    /// Observe the closed ordinary branch without copying SID data.
    pub(crate) fn is_ordinary(&self) -> bool {
        matches!(self, Self::Ordinary(_))
    }
    /// Compare inert allocation identity before retaining an exclusion owner.
    pub(crate) fn matches_original(&self, original: &DiagnosticNativeGrant) -> bool {
        matches!(self, Self::Retained(sid) if sid.original.matches_original(original))
    }
    /// Consume ordinary bytes without copying or projecting Diagnostic data.
    pub(crate) fn into_ordinary(self) -> Result<Vec<u8>, ClaimError> {
        match self {
            Self::Ordinary(bytes) => Ok(bytes),
            Self::Selected(_) => Err(ClaimError::capture_allocation(
                crate::installed_release::CaptureCapacityFailure::identity(),
            )),
            Self::Retained(sid) => {
                Err(ClaimError::diagnostic_allocation(sid.original.reject_native_state()))
            }
        }
    }
    /// Borrow bounded bytes and the original loan until successful SID construction.
    /// # Errors
    /// Oversized input stays in the caller slot without a new cause allocation.
    pub(crate) fn retained(
        input: &mut Option<(Vec<u8>, DiagnosticNativeSidLoan, DiagnosticNativeGrant)>,
    ) -> Result<Self, ClaimError> {
        let (bytes, _, original) = input.as_ref().ok_or_else(|| {
            ClaimError::capture_allocation(
                crate::installed_release::CaptureCapacityFailure::missing(),
            )
        })?;
        if bytes.capacity() > 72 {
            return Err(ClaimError::diagnostic_allocation(original.reject_native_state()));
        }
        let (bytes, allocation, original) = input.take().ok_or_else(|| {
            ClaimError::capture_allocation(
                crate::installed_release::CaptureCapacityFailure::missing(),
            )
        })?;
        Ok(Self::Retained(SharedNativeSid {
            data: Some(Arc::new(NativeSidData { bytes, _allocation: allocation })),
            original,
        }))
    }
    /// Borrow actual SID bytes under their original data/control custody.
    /// # Errors
    /// Rejects an already consumed shared control.
    pub(crate) fn bytes(&self) -> Result<&[u8], ClaimError> {
        match self {
            Self::Ordinary(bytes) => Ok(bytes),
            Self::Selected(sid) => sid.bytes().map_err(ClaimError::capture_allocation),
            Self::Retained(sid) => {
                sid.data.as_ref().map(|data| data.bytes.as_slice()).ok_or_else(|| {
                    ClaimError::diagnostic_allocation(sid.original.reject_native_state())
                })
            }
        }
    }
    /// Describe its72-byte destination plus concrete shared and outside controls.
    pub(crate) fn diagnostic_layout() -> Result<NativeRoleLayout, NativeLayoutFailure> {
        let bytes = 72_usize
            .checked_add(std::alloc::Layout::new::<ArcStorage<NativeSidData>>().size())
            .and_then(|value| value.checked_add(size_of::<SharedNativeSid>()))
            .ok_or(NativeLayoutFailure::Overflow)?;
        Ok(NativeRoleLayout { retained: bytes, peak: bytes })
    }
}
impl Clone for SharedNativeSid {
    fn clone(&self) -> Self {
        Self { data: self.data.clone(), original: self.original.clone() }
    }
}
impl Drop for SharedNativeSid {
    fn drop(&mut self) {
        if let Some(data) = self.data.take() {
            drop(Arc::into_inner(data));
        }
    }
}
