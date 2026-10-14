// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer genuine bounded WebView renderer evidence without transferring COM or file authority.

use crate::{
    MainIdentity,
    main_readiness_types::{DocumentId, RenderDocument},
};
use std::sync::Arc;
use sugarsubstitute_contracts::{
    configuration::LoopbackOrigin,
    execution::{CounterDeadline, MemoryClaim},
};
#[cfg(test)]
#[path = "qualified_preview/tests.rs"]
mod tests;

/// The actual current controller rectangle in physical pixels relative to its parent client.
/// PNG dimensions are the renderer viewport width and height, excluding native window chrome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MainPreviewViewport {
    pub(crate) left: i32,
    pub(crate) top: i32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) dpi: u32,
}
impl MainPreviewViewport {
    /// Return the actual physical left edge relative to the main client rectangle.
    pub const fn left(self) -> i32 {
        self.left
    }
    /// Return the actual physical top edge relative to the main client rectangle.
    pub const fn top(self) -> i32 {
        self.top
    }
    /// Return the renderer's physical pixel width.
    pub const fn width(self) -> u32 {
        self.width
    }
    /// Return the renderer's physical pixel height.
    pub const fn height(self) -> u32 {
        self.height
    }
    /// Return the actual parent-window dots per logical inch.
    pub const fn dpi(self) -> u32 {
        self.dpi
    }
}

/// Immutable observations from the exact owned main view at native capture admission.
/// These facts establish no active-display composition, focus, input or Mica appearance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MainPreviewSnapshot {
    pub(crate) document: RenderDocument,
    pub(crate) native_navigation: u64,
    pub(crate) browser_pid: u32,
    pub(crate) viewport: MainPreviewViewport,
    pub(crate) origin: LoopbackOrigin,
    pub(crate) versions: [DocumentId; 2],
    pub(crate) source_deadline: CounterDeadline,
    pub(crate) issued_qpc: i64,
}
impl MainPreviewSnapshot {
    /// Bind publication to the same original document, native view facts and finite source.
    /// A later observation time does not itself change an otherwise identical source.
    pub(crate) fn same_source(self, current: Self) -> bool {
        self.document == current.document
            && self.native_navigation == current.native_navigation
            && self.browser_pid == current.browser_pid
            && self.viewport == current.viewport
            && self.origin == current.origin
            && self.versions == current.versions
            && self.source_deadline == current.source_deadline
    }
    /// Return the exact authenticated application's main run, launch and navigation identity.
    pub const fn identity(self) -> MainIdentity {
        self.document.identity
    }
    /// Return the actual current native WebView navigation identifier.
    pub const fn native_navigation(self) -> u64 {
        self.native_navigation
    }
    /// Return the native browser process identifier from this retained view.
    pub const fn browser_process_id(self) -> u32 {
        self.browser_pid
    }
    /// Return the actual captured viewport rectangle and native DPI.
    pub const fn viewport(self) -> MainPreviewViewport {
        self.viewport
    }
    /// Return the root-admitted frontend loopback origin.
    pub const fn origin(self) -> LoopbackOrigin {
        self.origin
    }
    /// Return the original finite campaign cutoff in absolute QPC ticks and ticks per second.
    pub const fn source_deadline(self) -> CounterDeadline {
        self.source_deadline
    }
    /// Return actual creating-thread QPC ticks at capture admission, not physical presentation time.
    pub const fn issued_qpc(self) -> i64 {
        self.issued_qpc
    }
    /// Return the actual current native default execution context identifier.
    pub const fn context(self) -> i64 {
        self.document.context
    }
    /// Borrow bounded UTF-8 native root-frame identity bytes.
    pub fn frame(&self) -> &[u8] {
        self.document.frame.bytes()
    }
    /// Borrow bounded UTF-8 native root-loader identity bytes.
    pub fn loader(&self) -> &[u8] {
        self.document.loader.bytes()
    }
    /// Borrow the served frontend version from the admitted request's original source owner.
    pub fn frontend_version(&self) -> &[u8] {
        self.versions[0].bytes()
    }
    /// Borrow the installed SugarCubes version from the admitted request's original source owner.
    pub fn sugarcubes_version(&self) -> &[u8] {
        self.versions[1].bytes()
    }
}

/// Own one actual completed native PNG and its original admitted storage guards.
/// Only the desktop adapter constructs this value after current-document/callback checks.
/// The composing host persists bytes through central execution; it receives no COM interface.
pub struct CapturedMainPreview {
    pub(crate) snapshot: Arc<MainPreviewSnapshot>,
    pub(crate) bytes: Vec<u8>,
    pub(crate) image_claim: MemoryClaim,
    pub(crate) metadata: Arc<MemoryClaim>,
}
impl CapturedMainPreview {
    /// Borrow the bounded encoded PNG without cloning its allocation or releasing admission.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Observe exact immutable native/source facts without granting file or scheduling authority.
    pub fn snapshot(&self) -> MainPreviewSnapshot {
        *self.snapshot
    }
    /// Return the original charged image capacity; actual PNG length may be smaller.
    pub fn image_capacity_bytes(&self) -> u64 {
        self.image_claim.bytes()
    }
    /// Return the same fixed metadata charge retained across native retirement and persistence.
    pub fn metadata_capacity_bytes(&self) -> u64 {
        self.metadata.bytes()
    }
}
