// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Configure one canvas lifetime independently of a standalone window or command line.

use std::path::PathBuf;
use sugarsubstitute_image_gpu::AdapterKey;
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
    /// Explicit native adapter selection; absence selects CPU presentation.
    pub(crate) gpu: Option<AdapterKey>,
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
}
