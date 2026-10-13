// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep native repository data and real codec failures with their original admitted owners.
mod collection;
mod error;
mod layout;
mod publication;
mod records;
mod validation_failure;
pub(super) use collection::SummaryCollection;
pub use error::{LoanedAllocationFailure, LoanedCodecFailure};
pub(super) use error::{decode, validation};
pub(super) use records::{OwnerSlots, RetainedSummary};
pub(super) use validation_failure::ValidationFailure;
