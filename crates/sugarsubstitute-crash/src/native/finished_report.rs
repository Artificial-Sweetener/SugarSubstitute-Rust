// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe original finite controls across synchronous finished-report native calls.
//!
//! Native generation/module authority stays with the real capture/read owners.
//! The borrowed ABI source observes revocation and grants no native permission.
#[path = "finished_report/abi.rs"]
mod abi;
#[path = "finished_report/control.rs"]
mod control;
#[path = "finished_report/observation.rs"]
mod observation;

#[path = "finished_report/state.rs"]
mod state;
#[path = "finished_report/token.rs"]
mod token;

#[path = "finished_report/api.rs"]
mod api;
pub(super) use abi::Api;
pub(crate) use token::Token;
pub(crate) const COPY_CHUNK_BYTES: usize = 8192;
#[path = "finished_report/error.rs"]
mod error;
pub(crate) use error::SettlementFailure;
#[path = "finished_report/journal.rs"]
mod journal;
pub(crate) use journal::JournalSource;
