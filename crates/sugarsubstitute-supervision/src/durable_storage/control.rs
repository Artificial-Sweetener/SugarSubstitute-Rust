// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe original job controls around ordinary native storage effects, retaining both failures.
use super::check;
use std::io;
use sugarsubstitute_contracts::execution::JobContext;

/// Preserve an original I/O cause even when the same effect observes cancellation or expiry.
pub(super) fn effect<T>(
    context: &JobContext,
    operation: impl FnOnce() -> io::Result<T>,
) -> io::Result<T> {
    check(context)?;
    let result = operation();
    match (result, check(context)) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(cause), Ok(())) | (Ok(_), Err(cause)) => Err(cause),
        (Err(source), Err(control)) => {
            Err(io::Error::new(control.kind(), InterruptedEffect { source, control }))
        }
    }
}

/// Keep both observed failures available while exposing the original control category.
#[derive(Debug)]
pub(super) struct InterruptedEffect {
    pub(super) source: io::Error,
    pub(super) control: io::Error,
}
impl std::fmt::Display for InterruptedEffect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}; native storage effect failed: {}", self.control, self.source)
    }
}
impl std::error::Error for InterruptedEffect {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}
