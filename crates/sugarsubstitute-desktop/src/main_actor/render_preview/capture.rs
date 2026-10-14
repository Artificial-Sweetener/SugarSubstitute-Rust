// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one actual CapturePreview callback and its native interfaces through checked retirement.

use super::{
    actor::MainRenderSeal,
    private_desktop::PrivateDesktop,
    stream::{self, Shared},
};
use crate::{
    CapturedMainPreview, DesktopError, MainPreviewSnapshot, main_events::Events,
    timing::TimingClock, windows_timing::WindowsClock,
};
use std::{
    cell::Cell,
    rc::Rc,
    sync::{Arc, atomic::AtomicUsize},
};
use sugarsubstitute_contracts::execution::{CounterDeadline, MemoryLease};
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::{
    Win32::{Foundation::STG_E_INVALIDFUNCTION, System::Com::IStream},
    core::{HRESULT, Interface, implement},
};

struct Completion {
    events: Rc<Events>,
    stream: Arc<Shared>,
    pending: Cell<bool>,
    invoking: Cell<bool>,
    status: Cell<Option<HRESULT>>,
}

#[implement(ICoreWebView2CapturePreviewCompletedHandler)]
struct Completed {
    owner: Rc<Completion>,
}
impl ICoreWebView2CapturePreviewCompletedHandler_Impl for Completed_Impl {
    fn Invoke(&self, status: HRESULT) -> windows::core::Result<()> {
        let _callback = self.owner.events.callback();
        self.owner.invoking.set(true);
        if !self.owner.pending.replace(false) || self.owner.status.get().is_some() {
            let _retained_status = self.owner.stream.fail(STG_E_INVALIDFUNCTION);
        } else {
            self.owner.status.set(Some(status));
            if status.is_err() {
                let _retained_status = self.owner.stream.fail(status);
            } else if let Err(failure) = self.owner.stream.freeze() {
                let _retained_status = self.owner.stream.fail(failure);
            }
        }
        self.owner.invoking.set(false);
        self.owner.events.notify_progress();
        Ok(())
    }
}

/// Every partial read-only handle, stream, handler and source stays with this actual main actor.
pub(in crate::main_actor) struct Capture {
    snapshot: Arc<MainPreviewSnapshot>,
    core: ICoreWebView2,
    stream: IStream,
    handler: ICoreWebView2CapturePreviewCompletedHandler,
    completion: Rc<Completion>,
    private: PrivateDesktop,
    issued: bool,
    published: bool,
    closing: bool,
}

/// Pinned Rust Arc header plus fixed immutable evidence, funded in original preview metadata.
#[repr(C)]
struct SnapshotControlLayout {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: MainPreviewSnapshot,
}

impl Capture {
    /// Claim before allocating wrappers; this constructor performs no renderer or desktop effect.
    pub(super) fn new(
        seal: MainRenderSeal,
        lease: &MemoryLease,
        events: Rc<Events>,
    ) -> Result<Self, DesktopError> {
        let (snapshot, core) = seal.into_parts();
        let known_metadata = size_of::<Self>()
            + size_of::<Completion>()
            + size_of::<Completed>()
            + size_of::<Completed_Impl>()
            + size_of::<CapturedMainPreview>()
            + size_of::<Shared>()
            + size_of::<SnapshotControlLayout>()
            + 1024;
        if known_metadata as u64 > stream::METADATA_BYTES {
            return Err(DesktopError::message(
                "claim_preview_callback",
                "metadata capacity exceeded",
            ));
        }
        let shared = Shared::new(lease)?;
        let stream = stream::interface(&shared);
        let completion = Rc::new(Completion {
            events,
            stream: shared,
            pending: Cell::new(false),
            invoking: Cell::new(false),
            status: Cell::new(None),
        });
        let handler: ICoreWebView2CapturePreviewCompletedHandler =
            Completed { owner: completion.clone() }.into();
        Ok(Self {
            snapshot: Arc::new(snapshot),
            core,
            stream,
            handler,
            completion,
            private: PrivateDesktop::default(),
            issued: false,
            published: false,
            closing: false,
        })
    }

    /// Issue once after adoption by MainActor. Failed submissions retain their callback obligation.
    pub(super) fn issue(&mut self) -> Result<(), DesktopError> {
        if self.issued || self.closing || self.published {
            return Err(DesktopError::message("capture_main_preview", "capture admission closed"));
        }
        self.private.observe()?;
        if !self.private.admitted() {
            return Err(DesktopError::message("capture_main_preview", "inactive desktop missing"));
        }
        current_ticks(self.snapshot.source_deadline)?;
        self.issued = true;
        self.completion.pending.set(true);
        // SAFETY: MainActor has adopted this owner before a possibly synchronous callback;
        // all three interfaces belong to its creating STA and every output remains bounded.
        unsafe {
            self.core.CapturePreview(
                COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
                &self.stream,
                &self.handler,
            )
        }
        .map_err(|cause| DesktopError::new("capture_main_preview", cause))
    }

    /// Match this exact retained source and immutable current-document/viewport facts.
    pub(super) fn matches(&self, current: MainPreviewSnapshot, core: &ICoreWebView2) -> bool {
        (*self.snapshot).same_source(current) && core.as_raw() == self.core.as_raw()
    }

    /// Observe actual callback completion separately from a successful image publication.
    pub(super) fn completed(&self) -> bool {
        self.issued
            && !self.completion.pending.get()
            && !self.completion.invoking.get()
            && self.completion.status.get().is_some()
            && !self.completion.events.in_callback()
    }

    /// Wake only for a genuine completed unpublished callback, without spinning while pending.
    pub(in crate::main_actor) fn publication_pending(&self) -> bool {
        !self.published && !self.closing && self.completed()
    }

    /// Return the unchanged source snapshot for outer-current checks; it grants no new admission.
    pub(super) fn snapshot(&self) -> MainPreviewSnapshot {
        *self.snapshot
    }

    /// Check completion and cutoff before moving the original image allocation once.
    pub(super) fn take(&mut self) -> Result<Option<CapturedMainPreview>, DesktopError> {
        if self.published || self.closing || !self.completed() {
            return Ok(None);
        }
        current_ticks(self.snapshot.source_deadline)?;
        self.private.recheck()?;
        current_ticks(self.snapshot.source_deadline)?;
        self.completion
            .status
            .get()
            .ok_or_else(|| {
                DesktopError::message("complete_main_preview", "native completion missing")
            })?
            .ok()
            .map_err(|cause| DesktopError::new("complete_main_preview", cause))?;
        self.completion
            .stream
            .status()
            .ok()
            .map_err(|cause| DesktopError::new("complete_preview_stream", cause))?;
        let image = self.completion.stream.take().map_err(|cause| {
            DesktopError::new("take_main_preview", windows::core::Error::from(cause))
        })?;
        self.published = true;
        Ok(Some(CapturedMainPreview {
            snapshot: self.snapshot.clone(),
            bytes: image.bytes,
            image_claim: image.claim,
            metadata: self.completion.stream.metadata(),
        }))
    }

    /// Keep an already reported qualification failure from producing another observation.
    pub(super) fn reported_failure(&mut self) {
        self.published = true;
    }

    /// Close publication admission without manufacturing a callback or stopping native work.
    pub(in crate::main_actor) fn close(&mut self) {
        self.closing = true;
    }

    /// Require actual callback return and exact read-only handle closure before controller teardown.
    pub(in crate::main_actor) fn retirement_ready(&mut self) -> Result<bool, DesktopError> {
        self.private.retire()?;
        Ok(!self.completion.pending.get()
            && !self.completion.invoking.get()
            && !self.completion.events.in_callback())
    }
}

/// Observe the existing UI-affine QPC boundary against the original immutable counter facts.
pub(super) fn current_ticks(source: CounterDeadline) -> Result<i64, DesktopError> {
    let mut clock = WindowsClock::new()?;
    let sample = clock.read()?;
    if sample.thread != clock.thread() {
        return Err(DesktopError::message("observe_preview_cutoff", "creating thread changed"));
    }
    admit_counter(source, clock.frequency(), sample.ticks)
}

/// Validate observed native units without introducing another clock or renewing the cutoff.
fn admit_counter(source: CounterDeadline, frequency: i64, ticks: i64) -> Result<i64, DesktopError> {
    if frequency != source.frequency() || ticks < 0 || ticks >= source.ticks() {
        return Err(DesktopError::message(
            "observe_preview_cutoff",
            "original campaign expired or foreign",
        ));
    }
    Ok(ticks)
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;
