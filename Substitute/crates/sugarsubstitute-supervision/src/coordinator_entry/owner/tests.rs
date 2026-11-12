// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual finite execution consumption while faking only external native effects.
use super::*;
use crate::coordinator_entry::fixture::*;
use std::{error::Error, num::NonZeroUsize};
use sugarsubstitute_contracts::execution::{ScopeId, ScopeSpec};
use sugarsubstitute_execution::ExecutionOwner;
type TestResult = Result<(), Box<dyn Error>>;
mod admission;
mod custody;
mod proof;
