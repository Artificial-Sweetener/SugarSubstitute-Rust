// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject malformed or stale wire input before adapter effects.
/// Closed protocol rejection; native causes remain with their actual owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticWireError {
    /// Frame/header/payload/counter exceeds its literal bound.
    Capacity,
    /// Magic/schema/kind/boolean/flag is unsupported.
    Schema,
    /// A field, reserved byte or trailing byte is noncanonical.
    Shape,
    /// A complete identity is zero, foreign or changed.
    Identity,
    /// Original signed deadline/UTC is invalid or changed.
    Time,
    /// Role does not admit this variant.
    Role,
    /// A duplicate command changed its exact encoded content.
    ChangedDuplicate,
    /// High-water retains an evicted command's retired identity.
    RetiredOperation,
    /// One original command/transfer remains pending.
    Busy,
    /// Completion does not match the retained issued command.
    Stale,
    /// Offset, subject or total changed within one transfer.
    Chunk,
    /// Complete bytes fail digest or strict UTF8 validation.
    Integrity,
    /// The current full view has not yet completed.
    Incomplete,
}
impl std::fmt::Display for DiagnosticWireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Diagnostic protocol rejected: {self:?}")
    }
}
impl std::error::Error for DiagnosticWireError {}
