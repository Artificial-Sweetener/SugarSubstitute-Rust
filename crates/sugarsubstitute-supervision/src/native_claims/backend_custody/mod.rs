// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own independent witness exclusion and exact original native family custody.
mod commands;
pub(crate) mod control;
mod driver;
mod exit;
pub(crate) mod pool;
mod registration;
mod retirement;
mod source;
pub use driver::BackendCustodyDriverInputError;
pub use source::{BackendCustodyFailure, BackendCustodySource};
