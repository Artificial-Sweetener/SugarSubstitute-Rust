// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Declare the physical native adapter contract and retained causal errors.
use super::*;
use std::{error::Error, fmt};

/// Retain one actual same-store capture intent; there is no public constructor.
pub struct DiagnosticCaptureIntent {
    pub(super) shared: Shared,
    pub(super) operation: LogicalStoreOperationKey,
    pub(super) id: u64,
}
/// Invoke the actual pinned native capture owner while canonical files remain fenced.
///
/// # Safety
/// Implementations must retain their original generation, module PIN, token and funding.
/// They must compare that actual generation with the fence, strictly validate the live
/// Journal/Summary and optional payload (absent only for validated discard), and invoke
/// only that same token under the fence's original context. `Ok` requires actual source
/// Removed/AlreadyAbsent, candidate metadata and absence of both native stages, with
/// complete operation-specific result validation. Failure/unwind preserves all custody.
/// The production implementation is the reviewed Crash finished-report journal adapter.
/// Calls are synchronous, nonreentrant and outside storage locks; no borrowed state escapes.
/// Owned error aliases must retain their source-data funding; a borrowed cause must not
/// expose a clone that separates allocated data from its original custody.
#[allow(unsafe_code)]
pub unsafe trait DiagnosticCaptureNativeSource {
    /// Preserve the native adapter's actual original cause without string conversion.
    type Error: Error + Send + Sync + 'static;
    /// Perform or reconcile only the same original physical disposition.
    /// # Errors
    /// Retains actual token/source/stage custody whenever settlement remains uncertain.
    fn settle(&mut self, fence: &mut DiagnosticCaptureFence<'_>) -> Result<(), Self::Error>;
}
/// Preserve source/control data before the unchanged original whole-operation guard.
/// No cause or guard can be extracted separately; borrowed queries grant no authority.
pub struct DiagnosticCaptureRetirementError<E> {
    kind: ErrorKind<E>,
}
enum ErrorKind<E> {
    Admission(DiagnosticStorageError),
    Retained(CaptureErrorSlot<E>),
}
struct ErrorData<E> {
    source: Option<E>,
    post: Option<PostFailure>,
}
/// Closed destination for one returned source/post failure, including its box/control.
pub const CAPTURE_RETIREMENT_ERROR_BYTES: usize = 4096;
pub(in crate::diagnostic_storage) struct CaptureErrorSlot<E> {
    data: Box<Option<ErrorData<E>>>,
    _allocation: allocation::DiagnosticNativeOwnerLoan,
}
impl<E> CaptureErrorSlot<E> {
    pub(in crate::diagnostic_storage) fn new(
        original: &Shared,
        key: LogicalStoreOperationKey,
    ) -> Result<Self, DiagnosticStorageError> {
        let bytes = size_of::<Option<ErrorData<E>>>()
            .checked_add(size_of::<Self>())
            .and_then(|n| n.checked_add(size_of::<DiagnosticCaptureRetirementError<E>>()))
            .and_then(|n| n.checked_add(2 * size_of::<usize>()));
        if bytes.is_none_or(|bytes| bytes > CAPTURE_RETIREMENT_ERROR_BYTES) {
            return Err(error(key, Code::Capacity));
        }
        let grant = lock(original).allocation.native_grant();
        let allocation = grant.capture_retirement_error().map_err(|cause| {
            native(key, Code::Capacity, ClaimError::diagnostic_allocation(cause))
        })?;
        Ok(Self { data: Box::new(None), _allocation: allocation })
    }
    pub(super) fn finish(
        mut self,
        source: Option<E>,
        post: Option<PostFailure>,
    ) -> DiagnosticCaptureRetirementError<E> {
        *self.data = Some(ErrorData { source, post });
        DiagnosticCaptureRetirementError { kind: ErrorKind::Retained(self) }
    }
}
pub(super) enum PostFailure {
    Storage(DiagnosticStorageError),
    Panicked(Mutex<Box<dyn std::any::Any + Send>>),
}
impl<E> DiagnosticCaptureRetirementError<E> {
    pub(super) fn admission(cause: DiagnosticStorageError) -> Self {
        Self { kind: ErrorKind::Admission(cause) }
    }
    fn data(&self) -> Option<&ErrorData<E>> {
        match &self.kind {
            ErrorKind::Retained(slot) => slot.data.as_ref().as_ref(),
            _ => None,
        }
    }
    /// Borrow the genuine primary native cause, when one preceded storage observation.
    pub fn source_error(&self) -> Option<&E> {
        self.data().and_then(|data| data.source.as_ref())
    }
    /// Borrow an actual original storage/control failure without extracting its custody.
    pub fn post_storage_error(&self) -> Option<&DiagnosticStorageError> {
        match &self.kind {
            ErrorKind::Admission(cause) => Some(cause),
            _ => match self.data().and_then(|data| data.post.as_ref()) {
                Some(PostFailure::Storage(cause)) => Some(cause),
                _ => None,
            },
        }
    }
    /// Observe a retained actual post-provider panic without formatting its payload.
    pub fn post_panicked(&self) -> bool {
        matches!(self.data().and_then(|data| data.post.as_ref()), Some(PostFailure::Panicked(_)))
    }
}
impl<E: fmt::Debug> fmt::Debug for DiagnosticCaptureRetirementError<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("DiagnosticCaptureRetirementError")
            .field("source", &self.source_error())
            .field("storage", &self.post_storage_error())
            .field("post_panicked", &self.post_panicked())
            .finish_non_exhaustive()
    }
}
impl<E: fmt::Display> fmt::Display for DiagnosticCaptureRetirementError<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(cause) = self.source_error() {
            fmt::Display::fmt(cause, out)
        } else if let Some(cause) = self.post_storage_error() {
            fmt::Display::fmt(cause, out)
        } else {
            out.write_str("Capture retirement control provider panicked")
        }
    }
}
impl<E: Error + 'static> Error for DiagnosticCaptureRetirementError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source_error()
            .map(|cause| cause as &(dyn Error + 'static))
            .or_else(|| self.post_storage_error().map(|cause| cause as &(dyn Error + 'static)))
    }
}
impl Drop for PostFailure {
    fn drop(&mut self) {
        if let Self::Panicked(payload) = self {
            // Touch no payload formatter/provider; ordinary destruction precedes the guard.
            let _retained = payload.get_mut().unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }
}
