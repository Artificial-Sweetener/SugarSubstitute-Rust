// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own independent witness exclusion and exact original native family custody.
mod commands;
pub(crate) mod control;
mod driver;
mod entry;
mod installation;
pub use installation::BackendCustodyInstallation;
mod exit;
mod exit_diagnostic;
pub use exit_diagnostic::BackendCustodyExitDiagnostic;
pub(crate) mod pool;
mod profile_commands;
mod profile_registration;
mod registration;
mod retirement;
mod source;
pub use driver::BackendCustodyDriverInputError;
pub use source::{BackendCustodyFailure, BackendCustodySource};
