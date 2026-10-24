// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Producing engine identity shared by runtime admission and the native build record.
//!
//! Increment the release for each independently published engine generation. Protocol changes
//! describe implemented compatibility and have their own version. These declarations compile
//! into the engine; command-line arguments and installed state cannot override them.

/// Monotonic release identity of this installation engine.
pub const ENGINE_RELEASE: u64 = 1;
/// Update protocol implemented by this engine.
pub const ENGINE_PROTOCOL: u32 = 1;
