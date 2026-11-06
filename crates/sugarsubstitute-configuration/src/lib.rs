// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own desktop startup document decoding, explicit path selection and initial defaults.
//!
//! Document operations perform no I/O. Installer defaults derive from the setup executable's
//! location, and the installation layout names every folder below one root. Neither grants
//! native authority. Storage adapters retain
//! actual files and preserve saved bytes; execution owns admitted reads and their lifetimes.
//! Explicit production arguments select all six inputs together. An installed launch uses
//! its versioned descriptor; environment variables never override persisted startup values.
pub mod descriptor;
pub mod initial;
pub mod installer;
pub mod layout;
pub mod paths;
pub mod startup;
