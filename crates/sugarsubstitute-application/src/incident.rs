// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coordinate bounded incident decisions from accepted lifecycle and durable observations.
//!
//! Values correlate effects with actual adapter proofs; they grant no filesystem,
//! capture, reporter, process, or restart authority. Methods perform no I/O.
mod admission;
mod budget;
mod coordinator;
mod effects;
mod error;
mod observations;
mod publication;
mod recovery;
mod reporting;
mod retirement;
mod scope;
mod state;
#[cfg(test)]
mod tests;
mod types;

pub use budget::*;
pub use coordinator::IncidentCoordinator;
pub use effects::*;
pub use error::IncidentDecisionError;
pub use observations::*;
pub use scope::*;
pub use types::*;
