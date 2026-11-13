// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Inspect actual loopback HTTP bytes against independent pinned installed sources.
//! One finite attempt owns its native transaction outside discardable job results.
//! Fixed GET endpoints grant no arbitrary URL, proxy, redirect, authentication or cookie authority.
pub(crate) mod headers;
mod memory;
mod native;
mod owner;
mod probe;
mod terminal;
#[cfg(test)]
mod tests;
mod types;
mod verification;
pub use owner::ReadinessProbeOwner;
pub use types::{
    ProbeRetryDisposition, ReadinessProbeInput, ReadinessTransportError, VerifiedReadiness,
};
/// Peak admitted attempt storage, including bounded response parsing, native and TCP-query scratch.
pub const READINESS_TRANSPORT_BYTES: u64 = 16 * 1024 * 1024;
#[cfg(test)]
pub(crate) mod campaign_fixture;
