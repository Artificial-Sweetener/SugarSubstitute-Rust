// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Carry first-host startup observations, intents and bounded frontend readiness.
//!
//! A receipt describes observed web presentation only. It grants no native
//! privileges. The desktop adapter verifies origin before passing bytes here;
//! the application separately admits one terminal receipt for its current epoch.
//! Concrete observations and effects describe acknowledged lifecycle ordering;
//! they carry no live process, browser or native surface ownership.

mod events;
mod intents;
mod readiness;

pub use events::{StartupEvent, StartupFailure, StartupFailureCode, StartupObservation};
pub use intents::{ConnectionCleanup, SplashIntent, StartupEffect};

pub use readiness::{
    FrontendEvidence, LaunchId, ProbeFailure, ReadinessError, ReadinessFailure, ReadinessOutcome,
    ReadinessReceipt,
};
