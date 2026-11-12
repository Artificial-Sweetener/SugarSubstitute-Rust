// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bounded synchronous HTTP mechanics for admitted loopback and update workers.
//!
//! Callers retain their original memory claims until checked native retirement. This
//! adapter creates no tasks and exports no handles, callbacks or TLS bypasses.
//! Windows proxy resolution returns bounded ordered routes through its owned resolver.
//! Loopback peer identity, update origin policy and authentication remain independent
//! feature decisions. Every operation requires exclusive custody.
mod deadline;
mod request;
#[cfg(windows)]
pub mod system_proxy;
#[cfg(all(windows, test))]
mod test_support;
pub use deadline::{DeadlineError, milliseconds};
pub use request::{InvalidRequest, UpdateRequest};
#[cfg(windows)]
#[allow(unsafe_code)]
mod native;
#[cfg(windows)]
pub use native::{Response, Transport};

#[cfg(windows)]
mod failure;
#[cfg(windows)]
pub use failure::{Cause, Failure};
