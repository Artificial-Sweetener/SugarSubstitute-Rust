// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer genuine bounded WebView renderer evidence without transferring COM or file authority.

use crate::{
    MainIdentity,
    main_readiness_types::{DocumentId, RenderDocument},
};
use std::{error::Error, fmt, sync::Arc};
use sugarsubstitute_contracts::{
    configuration::LoopbackOrigin,
    execution::{CounterDeadline, MemoryClaim},
};
#[cfg(test)]
#[path = "qualified_preview/tests.rs"]
mod tests;

/// One pre-inspected existing ComfyUI output for an explicit native download qualification.
/// This value identifies a server output basename; it grants no native path or URL authority.
pub struct MainDownloadFixture {
    name: String,
    bytes: u64,
    digest: [u8; 32],
}
impl MainDownloadFixture {
    /// Validate a bounded PNG basename and its independently observed original size/digest.
    /// # Errors
    /// Rejects path/URL syntax, unsupported spelling, empty/oversized PNGs and invalid SHA-256.
    pub fn new(name: String, bytes: u64, digest: &str) -> Result<Self, crate::DesktopError> {
        if name.len() > 128
            || name.capacity() > 256
            || !name.ends_with(".png")
            || name.len() <= 4
            || name.starts_with('.')
            || !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
            || bytes == 0
            || bytes > 2 * 1024 * 1024
            || digest.len() != 64
        {
            return Err(crate::DesktopError::message(
                "qualify_download_fixture",
                "invalid PNG fixture",
            ));
        }
        let mut decoded = [0; 32];
        for (target, pair) in decoded.iter_mut().zip(digest.as_bytes().chunks_exact(2)) {
            let pair = std::str::from_utf8(pair)
                .map_err(|cause| crate::DesktopError::new("qualify_download_fixture", cause))?;
            *target = u8::from_str_radix(pair, 16)
                .map_err(|cause| crate::DesktopError::new("qualify_download_fixture", cause))?;
        }
        Ok(Self { name, bytes, digest: decoded })
    }
    /// Borrow the validated server output basename, never a filesystem path.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Return the original independently observed byte length.
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
    /// Borrow the original independently observed SHA-256 bytes.
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    /// Account the retained name allocation within the original qualification budget.
    pub fn allocated_bytes(&self) -> usize {
        self.name.capacity()
    }
    /// Encode bounded existing facts for the inherited qualification argument projection.
    pub fn digest_hex(&self) -> String {
        let mut encoded = String::with_capacity(64);
        for byte in self.digest {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 15)]));
        }
        encoded
    }
}

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

/// Closed source differences carry no document payload, path or native capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreviewMismatch {
    Document,
    Navigation,
    Browser,
    Viewport,
    Dpi,
    Origin,
    Versions,
    Deadline,
    NativeView,
}
impl PreviewMismatch {
    /// Preserve the precise original category under an existing bounded diagnostic operation.
    /// The Host's closed decoder relays these operations without a protocol or capacity change.
    pub(crate) fn into_error(self) -> crate::DesktopError {
        let operation = match self {
            Self::Document | Self::Navigation | Self::Origin => "observe_preview_content",
            Self::Browser | Self::NativeView => "observe_preview_browser",
            Self::Viewport => "observe_preview_bounds",
            Self::Dpi => "observe_preview_dpi",
            Self::Versions => "observe_preview_versions",
            Self::Deadline => "observe_preview_cutoff",
        };
        crate::DesktopError::new(operation, self)
    }
}
impl fmt::Display for PreviewMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Native preview source changed: {self:?}")
    }
}
impl Error for PreviewMismatch {}

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
    /// Revalidate every original document, viewport and finite source fact before publication.
    /// A later observation time preserves the same source; failure keeps both snapshots unchanged.
    pub(crate) fn validate_current(self, current: Self) -> Result<(), PreviewMismatch> {
        if self.document != current.document {
            return Err(PreviewMismatch::Document);
        }
        if self.native_navigation != current.native_navigation {
            return Err(PreviewMismatch::Navigation);
        }
        if self.browser_pid != current.browser_pid {
            return Err(PreviewMismatch::Browser);
        }
        if self.viewport.dpi != current.viewport.dpi {
            return Err(PreviewMismatch::Dpi);
        }
        if self.viewport != current.viewport {
            return Err(PreviewMismatch::Viewport);
        }
        if self.origin != current.origin {
            return Err(PreviewMismatch::Origin);
        }
        if self.versions != current.versions {
            return Err(PreviewMismatch::Versions);
        }
        if self.source_deadline != current.source_deadline {
            return Err(PreviewMismatch::Deadline);
        }
        Ok(())
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
