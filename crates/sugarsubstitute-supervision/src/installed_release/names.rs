// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep each selected native path and its shared control within its original fixed region.
use super::allocation::*;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicUsize},
};
use sugarsubstitute_contracts::execution::AdmissionError;

/// Choose one immutable native path slot; callers cannot choose a capacity.
#[derive(Clone, Copy)]
pub(crate) enum CapturePathRole {
    CurrentImage,
    Manifest,
    Signature,
    CurrentPdb,
    ClientImage,
    ClientPdb,
    HandlerImage,
    HandlerPdb,
}
impl CapturePathRole {
    const fn slot(self) -> u8 {
        self as u8
    }
}

struct NameData {
    path: PathBuf,
    _path: CaptureLoan,
    _control: SharedCaptureLoan,
}
#[repr(C)]
struct NameControl {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: NameData,
}
/// Keep the same final path allocation outside every borrowed ancestor alias.
pub(crate) struct CaptureNativeName {
    data: Option<Arc<NameData>>,
    depth: Option<usize>,
    original: AllocationOrigin,
}
/// Retain both final destinations before producing any name bytes.
pub(crate) struct CaptureNameDestination {
    path: CaptureLoan,
    control: SharedCaptureLoan,
    original: AllocationOrigin,
}
/// Preserve exact constructor failures before any native file effect.
#[derive(Debug)]
pub(crate) enum CaptureNameFailure {
    Capacity(CaptureCapacityFailure),
    Admission(AdmissionError),
    Reserve(std::collections::TryReserveError),
    Shape(&'static str),
}
impl std::fmt::Display for CaptureNameFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shape(reason) => write!(f, "Selected native name rejected: {reason}"),
            _ => f.write_str("Selected native name construction failed"),
        }
    }
}
impl std::error::Error for CaptureNameFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Capacity(c) => Some(c),
            Self::Admission(c) => Some(c),
            Self::Reserve(c) => Some(c),
            Self::Shape(_) => None,
        }
    }
}
pub(crate) const fn name_control_bytes() -> usize {
    size_of::<NameControl>()
}
impl AllocationOrigin {
    pub(crate) fn name(
        &self,
        role: CapturePathRole,
    ) -> Result<CaptureNameDestination, CaptureCapacityFailure> {
        let path = self.reserve(CaptureRegion::Path(role.slot()), PATH_BYTES)?;
        let control = self.reserve(
            CaptureRegion::Metadata,
            size_of::<NameControl>()
                .checked_add(SharedCaptureLoan::control_bytes())
                .ok_or(CaptureCapacityFailure::new(CapacityFailureKind::Overflow))?,
        )?;
        let control = SharedCaptureLoan::new(control)?;
        Ok(CaptureNameDestination { path, control, original: self.clone() })
    }
}
impl CaptureNameDestination {
    pub(crate) fn produce(
        self,
        path: &Path,
        child: Option<&std::ffi::OsStr>,
    ) -> Result<CaptureNativeName, CaptureNameFailure> {
        self.original.original().check_effect().map_err(CaptureNameFailure::Admission)?;
        let count =
            path.as_os_str().len().checked_add(child.map_or(0, |child| child.len() + 1)).ok_or(
                CaptureNameFailure::Capacity(CaptureCapacityFailure::new(
                    CapacityFailureKind::Overflow,
                )),
            )?;
        if count > PATH_BYTES {
            return Err(CaptureNameFailure::Capacity(CaptureCapacityFailure::new(
                CapacityFailureKind::Layout,
            )));
        }
        let mut owned = PathBuf::new();
        owned.try_reserve_exact(PATH_BYTES).map_err(CaptureNameFailure::Reserve)?;
        if owned.capacity() > PATH_BYTES {
            return Err(CaptureNameFailure::Capacity(CaptureCapacityFailure::new(
                CapacityFailureKind::Layout,
            )));
        }
        owned.push(path);
        if let Some(child) = child {
            owned.push(child);
        }
        crate::protected_storage::paths::validate_local_shape(&owned)
            .map_err(CaptureNameFailure::Shape)?;
        self.finish(owned)
    }
    pub(crate) fn finish(self, path: PathBuf) -> Result<CaptureNativeName, CaptureNameFailure> {
        if path.capacity() > PATH_BYTES {
            return Err(CaptureNameFailure::Capacity(CaptureCapacityFailure::new(
                CapacityFailureKind::Layout,
            )));
        }
        Ok(CaptureNativeName {
            data: Some(Arc::new(NameData { path, _path: self.path, _control: self.control })),
            depth: None,
            original: self.original,
        })
    }
}
impl CaptureNativeName {
    pub(crate) fn original(&self) -> &AllocationOrigin {
        &self.original
    }
    pub(crate) fn path(&self) -> Result<&Path, CaptureCapacityFailure> {
        let data =
            self.data.as_ref().ok_or(CaptureCapacityFailure::new(CapacityFailureKind::Missing))?;
        let path = match self.depth {
            None => data.path.as_path(),
            Some(depth) => data
                .path
                .ancestors()
                .nth(depth)
                .ok_or(CaptureCapacityFailure::new(CapacityFailureKind::Layout))?,
        };
        if !path.has_root()
            || matches!(path.components().next_back(), Some(std::path::Component::Prefix(_)))
        {
            return Err(CaptureCapacityFailure::new(CapacityFailureKind::Layout));
        }
        Ok(path)
    }
    pub(crate) fn capacity(&self) -> Result<usize, CaptureCapacityFailure> {
        self.data
            .as_ref()
            .map(|data| data.path.capacity())
            .ok_or(CaptureCapacityFailure::new(CapacityFailureKind::Missing))
    }
}
impl Clone for CaptureNativeName {
    fn clone(&self) -> Self {
        Self { data: self.data.clone(), depth: self.depth, original: self.original.clone() }
    }
}
impl Drop for CaptureNativeName {
    fn drop(&mut self) {
        if let Some(data) = self.data.take() {
            drop(Arc::into_inner(data));
        }
    }
}

struct TransientNameData {
    path: PathBuf,
}
#[repr(C)]
struct TransientControl {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: TransientNameData,
}
pub(super) const fn transient_control_bytes() -> usize {
    size_of::<TransientControl>()
}
pub(crate) struct CaptureTransientName {
    data: Option<Arc<TransientNameData>>,
    allocation: SharedCaptureLoan,
    original: AllocationOrigin,
}
impl CaptureTransientName {
    pub(crate) fn new(
        path: PathBuf,
        allocation: SharedCaptureLoan,
        original: AllocationOrigin,
    ) -> Result<Self, CaptureCapacityFailure> {
        if !allocation.original().matches(&original) || path.capacity() > PATH_BYTES {
            return Err(CaptureCapacityFailure::identity());
        }
        Ok(Self { data: Some(Arc::new(TransientNameData { path })), allocation, original })
    }
    pub(crate) fn path(&self) -> Result<&Path, CaptureCapacityFailure> {
        self.data.as_ref().map(|data| data.path.as_path()).ok_or(CaptureCapacityFailure::missing())
    }
    pub(crate) fn capacity(&self) -> Result<usize, CaptureCapacityFailure> {
        self.data.as_ref().map(|data| data.path.capacity()).ok_or(CaptureCapacityFailure::missing())
    }
}
impl Clone for CaptureTransientName {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            allocation: self.allocation.clone(),
            original: self.original.clone(),
        }
    }
}
impl Drop for CaptureTransientName {
    fn drop(&mut self) {
        if let Some(data) = self.data.take() {
            drop(Arc::into_inner(data));
        }
    }
}

struct SidData {
    bytes: Vec<u8>,
    _allocation: SharedCaptureLoan,
}
#[repr(C)]
struct SidControl {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: SidData,
}
pub(crate) struct CaptureNativeSid {
    data: Option<Arc<SidData>>,
    original: AllocationOrigin,
}
pub(crate) struct CaptureSidDestination {
    allocation: SharedCaptureLoan,
    original: AllocationOrigin,
}
impl AllocationOrigin {
    pub(crate) fn sid_destination(&self) -> Result<CaptureSidDestination, CaptureCapacityFailure> {
        let bytes = 72_usize
            .checked_add(size_of::<SidControl>())
            .and_then(|n| n.checked_add(SharedCaptureLoan::control_bytes()))
            .ok_or(CaptureCapacityFailure::new(CapacityFailureKind::Overflow))?;
        let allocation = SharedCaptureLoan::new(self.reserve(CaptureRegion::Metadata, bytes)?)?;
        Ok(CaptureSidDestination { allocation, original: self.clone() })
    }
}
impl CaptureSidDestination {
    pub(crate) fn original(&self) -> &AllocationOrigin {
        &self.original
    }
    pub(crate) fn finish(self, bytes: Vec<u8>) -> Result<CaptureNativeSid, CaptureCapacityFailure> {
        if bytes.capacity() > 72 {
            return Err(CaptureCapacityFailure::identity());
        }
        Ok(CaptureNativeSid {
            data: Some(Arc::new(SidData { bytes, _allocation: self.allocation })),
            original: self.original,
        })
    }
}
impl CaptureNativeSid {
    pub(crate) fn original(&self) -> &AllocationOrigin {
        &self.original
    }
    pub(crate) fn bytes(&self) -> Result<&[u8], CaptureCapacityFailure> {
        self.data
            .as_ref()
            .map(|data| data.bytes.as_slice())
            .ok_or(CaptureCapacityFailure::missing())
    }
    pub(crate) const fn control_bytes() -> usize {
        size_of::<SidControl>()
    }
}
impl Clone for CaptureNativeSid {
    fn clone(&self) -> Self {
        Self { data: self.data.clone(), original: self.original.clone() }
    }
}
impl Drop for CaptureNativeSid {
    fn drop(&mut self) {
        if let Some(data) = self.data.take() {
            drop(Arc::into_inner(data));
        }
    }
}
