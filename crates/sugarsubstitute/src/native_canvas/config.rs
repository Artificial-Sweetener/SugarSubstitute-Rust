// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Configure one canvas lifetime independently of a standalone window or command line.

use super::gpu_policy::GpuRequest;
use std::path::PathBuf;
use sugarsubstitute_image_ops::Extent;

/// The initial source decision belongs to the composing host.
pub(crate) enum InitialSource {
    /// Keep the initialized canvas empty until the host supplies a source.
    AwaitHost,
    /// Ask the standalone desktop to open its image picker.
    PickFile,
    /// Load an explicitly selected local source after initialization.
    File(PathBuf),
}

/// Runtime inputs shared by standalone and application-owned canvas lifetimes.
pub(crate) struct Config {
    /// Native adapter request; absence selects CPU presentation. An embedded request that cannot
    /// be satisfied is replaced by absence before any display work starts.
    pub(crate) gpu: Option<GpuRequest>,
    /// Retain the additional reference pixels required for explicit qualification.
    pub(crate) capture: bool,
    /// Request the separate native display verification readback during qualification.
    pub(crate) verify_native: bool,
    /// Cache anchor supplied by the composing host and opened by the cache owner.
    pub(crate) cache: PathBuf,
    /// One initial source action, consumed after storage initialization.
    pub(crate) initial: InitialSource,
    /// Locale used for canvas-owned status messages.
    pub(crate) locale: String,
    /// Initial native viewport extent in physical pixels.
    pub(crate) extent: Extent,
    /// Largest physical canvas extent whose storage this session funds; normally the box
    /// enclosing every attached display.
    pub(crate) maximum_extent: Extent,
    /// Storage this session funds for recently shown output documents kept for instant
    /// reselection, in bytes; zero retains none.
    pub(crate) retained_document_bytes: u64,
}
