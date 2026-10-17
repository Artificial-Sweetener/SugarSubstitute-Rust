// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode exact closed console operations and direct codes inside the first-failure slot.
use sugarsubstitute_desktop::console_failure_diagnostic::{
    ConsoleFailureCode as Code, ConsoleFailureOperation as Operation,
};
/// Optional observational console detail; no native or lifecycle authority is conveyed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) struct ConsoleFailureDetail {
    pub(in super::super) operation: Operation,
    pub(in super::super) code: Code,
}
/// Require both ordered fields, rejecting duplicates, missing values and raw extensions.
pub(super) fn parse(
    mut fields: std::str::SplitAsciiWhitespace<'_>,
) -> Option<ConsoleFailureDetail> {
    let operation = Operation::parse(fields.next()?.strip_prefix("console_operation=")?)?;
    let code = Code::parse(fields.next()?.strip_prefix("console_code=")?)?;
    if fields.next().is_some() {
        return None;
    }
    Some(ConsoleFailureDetail { operation, code })
}
#[cfg(test)]
#[path = "console_tests.rs"]
mod tests;
