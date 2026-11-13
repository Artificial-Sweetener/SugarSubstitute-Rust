// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Describe observed startup work without conferring readiness or process authority.

/// Current user-visible operation across the entire desktop startup journey.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StartupOperation {
    /// Establish the configured connection and process ownership.
    #[default]
    Preparing,
    /// Install or validate Python and custom-node dependencies.
    Dependencies,
    /// Import the inference runtime.
    Runtime,
    /// Initialize the compute device and attention implementation.
    Device,
    /// Import ComfyUI extensions.
    CustomNodes,
    /// Prepare persistent ComfyUI metadata.
    Database,
    /// Wait for the actual server readiness probe.
    Server,
    /// Load the frontend and await its readiness receipt.
    Frontend,
    /// Present the prepared main window.
    Reveal,
    /// Native main-window reveal was acknowledged.
    Ready,
}
