// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual account/installation and backend claims across owned processes.
//! Controlled event acknowledgements and diagnostic files prove only this fixture;
//! they establish neither authenticated native IPC nor complete M1/K1 acceptance.
mod child;
mod failure;
mod options;
mod parent;
mod receipt;
mod specification;
pub(crate) use child::run_child;
pub(crate) use options::{CampaignOptions, ChildOptions};
pub(crate) use parent::run;
use std::error::Error;
type Result<T> = std::result::Result<T, Box<dyn Error>>;
