// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Correlate one desktop main attempt across presentation and native authorization.

use super::LaunchId;
use crate::{execution::Generation, lifecycle::RunKey};

/// Bind main presentation to its desktop run, startup attempt and navigation.
///
/// Matching identities correlate observations. They confer no native authority,
/// process ownership, profile lease or authenticated retirement proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MainIdentity {
    /// Coordinator-bound desktop run supplied by the composing application.
    pub run: RunKey,
    /// Application startup attempt; independent of the lifecycle generation.
    pub launch: LaunchId,
    /// Navigation generation; independent of the rendered frame revision.
    pub navigation: Generation,
}
