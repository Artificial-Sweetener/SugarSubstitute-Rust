// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve the execution owner's bounded panic cause across HTTP result delivery.
use std::{error::Error, fmt};
use sugarsubstitute_contracts::execution::PanicCause;
#[derive(Debug)]
pub(super) struct ProbePanic(pub(super) PanicCause);
impl fmt::Display for ProbePanic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}
impl Error for ProbePanic {}
