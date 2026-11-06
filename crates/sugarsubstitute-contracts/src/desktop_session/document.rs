// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve Comfy current and initial graphs within one portable document admission budget.
use super::{MAX_DOCUMENT_BYTES, SessionError, primitives::graph, wire::SessionDocument};

/// Admit one current/baseline snapshot without interpreting Comfy graph meaning.
///
/// Combined UTF-8 graph bytes are bounded before either graph is decoded. The complete
/// encoded envelope shares that same 32 MiB budget; a saved baseline cannot double it.
/// Parsing is synchronous; callers own CPU/memory admission and cancellation at their execution boundary.
/// # Errors
/// Returns `TooLarge` for byte overflow and `Invalid` for shape, JSON or document binding errors.
pub fn validate_document(value: &SessionDocument) -> Result<(), SessionError> {
    if !value.valid() {
        return Err(SessionError::Invalid);
    }
    if value
        .graph_json
        .len()
        .saturating_add(value.baseline_graph_json.as_ref().map_or(0, String::len))
        > MAX_DOCUMENT_BYTES
    {
        return Err(SessionError::TooLarge);
    }
    if value.name.is_empty()
        || value.path.as_ref().is_some_and(String::is_empty)
        || (value.temporary && value.path.is_some())
        || !graph(&value.graph_json)
        || value.baseline_graph_json.as_ref().is_some_and(|baseline| !graph(baseline))
    {
        return Err(SessionError::Invalid);
    }
    if serde_json::to_vec(value).map_err(|_| SessionError::Invalid)?.len() > MAX_DOCUMENT_BYTES {
        return Err(SessionError::TooLarge);
    }
    Ok(())
}
