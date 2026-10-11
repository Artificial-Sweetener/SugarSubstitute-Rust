// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Bound frontend readiness evidence to one launch and navigation.
//!
//! A receipt describes observed web presentation only. It grants no native
//! privileges. The desktop adapter verifies origin before passing bytes here;
//! the application separately admits one terminal receipt for its current epoch.

mod readiness;

pub use readiness::{
    FrontendEvidence, LaunchId, ProbeFailure, ReadinessError, ReadinessFailure, ReadinessOutcome,
    ReadinessReceipt,
};
