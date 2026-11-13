// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify contained same-executable lifecycle transport through its actual owners.
//! Isolated matching namespace/epoch facts establish no installation or backend trust.
//! Secrets remain in the inherited read-only section and admitted message buffers.
mod child;
mod child_runtime;
mod evidence;
mod failure;
mod options;
mod parent;
mod parent_activity;
mod parent_cleanup;
mod parent_protocol;
mod parent_runtime;
mod protocol;
mod receipt;
mod terminal;
#[cfg(test)]
mod tests;

pub(crate) use options::ChildOptions;
use std::error::Error;
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Return bounded evidence only after actual transport/family/worker/native teardown.
pub(crate) fn run() -> Result<serde_json::Value> {
    parent::run()
}
/// Return the isolated child's status only after its actual owned shutdown.
pub(crate) fn run_child(options: ChildOptions) -> Result<i32> {
    child::run(options)
}
