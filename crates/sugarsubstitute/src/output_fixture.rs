// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual contained output without treating stream EOF as family settlement.
mod channel;
mod child;
mod child_runtime;
#[cfg(test)]
mod cleanup_tests;
mod corpus;
mod evidence;
mod failure;
mod options;
mod parent;
mod parent_activity;
mod parent_cleanup;
mod parent_control;
mod parent_output;
mod parent_runtime;
mod receipt;
#[cfg(test)]
mod tests;
mod writers;
pub(crate) use options::ChildOptions;
use std::error::Error;
type Result<T> = std::result::Result<T, Box<dyn Error>>;
/// Run exactly four bounded cases; the caller persists the returned receipt on D:.
pub(crate) fn run() -> Result<serde_json::Value> {
    parent::run()
}
/// Run one isolated self-EXE role through actual headless/execution shutdown.
pub(crate) fn run_child(options: ChildOptions) -> Result<i32> {
    child::run(options)
}
