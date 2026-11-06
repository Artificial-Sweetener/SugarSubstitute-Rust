// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate trust/correlation facts before application-owned protocol effects.
use super::{
    MAX_CHUNK_BYTES, MAX_MESSAGE_BYTES, SessionError,
    primitives::{digest, file, scope, uuid},
    snapshot::output,
    wire::*,
};
use std::collections::BTreeSet;

/// Require one fresh runtime session per unique durable occurrence.
fn bindings(values: &[SessionBinding]) -> bool {
    let entries: BTreeSet<_> = values.iter().map(|value| &value.entry_id).collect();
    let runtime: BTreeSet<_> = values.iter().map(|value| &value.runtime_session).collect();
    entries.len() == values.len()
        && runtime.len() == values.len()
        && values.iter().all(|value| uuid(&value.entry_id) && !value.runtime_session.is_empty())
}

/// Compare complete committed revision/generation fences independently of tab order.
pub fn same_fence(left: &SessionFence, right: &SessionFence) -> bool {
    left.webview == right.webview
        && left.revision == right.revision
        && left.bindings.len() == right.bindings.len()
        && bindings(&left.bindings)
        && bindings(&right.bindings)
        && left.bindings.iter().all(|value| right.bindings.contains(value))
}

/// Validate only operation semantics; application admission supplies current capabilities/state.
fn operation(value: &SessionOperation, expected_scope: &SessionScope) -> bool {
    match value {
        SessionOperation::Ready { capabilities, fence, .. } => {
            if !uuid(&fence.webview) || !bindings(&fence.bindings) {
                return false;
            }
            !(capabilities.capture || capabilities.materialize) || capabilities.chunk_transfer
        }
        SessionOperation::Capture { fence, checkpoint_id, .. } => {
            uuid(&fence.webview) && bindings(&fence.bindings) && uuid(checkpoint_id)
        }
        SessionOperation::Materialize { fence, transfer_id } => {
            uuid(&fence.webview) && bindings(&fence.bindings) && uuid(transfer_id)
        }
        SessionOperation::Begin { transfer } => {
            uuid(&transfer.id)
                && uuid(&transfer.checkpoint_id)
                && digest(&transfer.digest)
                && transfer.bytes >= transfer.chunks
                && transfer.bytes <= transfer.chunks * MAX_CHUNK_BYTES as u64
        }
        SessionOperation::Chunk { transfer_id, text, .. } => {
            uuid(transfer_id) && !text.is_empty() && text.len() <= MAX_CHUNK_BYTES
        }
        SessionOperation::Commit { transfer_id } => uuid(transfer_id),
        SessionOperation::Cancel { target_request_id } => uuid(target_request_id),
        SessionOperation::Acknowledged { checkpoint_id, bindings: values, .. } => {
            uuid(checkpoint_id) && bindings(values)
        }
        SessionOperation::Error { failure } => {
            failure.entry_id.as_deref().is_none_or(uuid)
                && failure.output_id.as_deref().is_none_or(uuid)
        }
        SessionOperation::Resolve { output: value } => {
            output(value) && value.backend == expected_scope.backend
        }
        SessionOperation::Resolved { output_id, digest: hash, file: descriptor, .. } => {
            uuid(output_id) && digest(hash) && file(descriptor)
        }
    }
}

/// Validate values against independently authorized origin and stable backend/profile namespaces.
/// # Errors
/// Rejects malformed values, untrusted origins, foreign scopes and encoded size overflow.
pub fn validate_message(
    value: &SessionMessage,
    expected_scope: &SessionScope,
    expected_origin: &str,
) -> Result<(), SessionError> {
    if !value.valid() || !scope(&value.scope) || !uuid(&value.request_id) {
        return Err(SessionError::Invalid);
    }
    if expected_origin.is_empty() || value.origin != expected_origin {
        return Err(SessionError::UntrustedOrigin);
    }
    if value.scope != *expected_scope {
        return Err(SessionError::Stale);
    }
    if !operation(&value.operation, expected_scope) {
        return Err(SessionError::Invalid);
    }
    if serde_json::to_vec(value).map_err(|_| SessionError::Invalid)?.len() > MAX_MESSAGE_BYTES {
        return Err(SessionError::TooLarge);
    }
    Ok(())
}

/// Decode a bounded message before trust comparison and application lifecycle admission.
/// # Errors
/// Rejects malformed JSON, duplicate/unknown members, invalid identity, trust or bounds.
pub fn decode_message(
    bytes: &[u8],
    scope: &SessionScope,
    origin: &str,
) -> Result<SessionMessage, SessionError> {
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(SessionError::TooLarge);
    }
    let value = serde_json::from_slice(bytes).map_err(|_| SessionError::Invalid)?;
    validate_message(&value, scope, origin)?;
    Ok(value)
}

/// Encode a message only after complete trust, shape and byte admission.
/// # Errors
/// Returns the same refusals as [`validate_message`] without granting native authority.
pub fn encode_message(
    value: &SessionMessage,
    scope: &SessionScope,
    origin: &str,
) -> Result<Vec<u8>, SessionError> {
    validate_message(value, scope, origin)?;
    serde_json::to_vec(value).map_err(|_| SessionError::Invalid)
}

/// Reject unsupported work, persistence opt-out, loading state or stale committed generations.
///
/// Ready observations advance only within the host-bootstrap webview. Equal observation
/// revisions preserve all binding, capability, commit and persistence facts. Capture carries
/// a separate host durable revision; frontend observations cannot assign publication order.
/// The caller first validates the message against independent trust facts. Transfer ordering,
/// request retirement, cancellation/deadlines and durable publication are application-owned.
/// # Errors
/// Returns the actionable refusal without performing transport or native effects.
pub fn admit_operation(
    message: &SessionMessage,
    capabilities: &SessionCapabilities,
    committed: bool,
    persistence: bool,
    current: &SessionFence,
) -> Result<(), SessionError> {
    if let SessionOperation::Ready {
        fence,
        capabilities: observed,
        committed: ready_committed,
        workflow_persistence,
    } = &message.operation
    {
        if fence.webview != current.webview
            || fence.revision < current.revision
            || (fence.revision == current.revision
                && (!same_fence(fence, current)
                    || ready_committed != &committed
                    || workflow_persistence != &persistence
                    || observed != capabilities))
        {
            return Err(SessionError::Stale);
        }
        return Ok(());
    }
    let fence = match &message.operation {
        SessionOperation::Begin { .. }
        | SessionOperation::Chunk { .. }
        | SessionOperation::Commit { .. } => {
            return if capabilities.chunk_transfer {
                Ok(())
            } else {
                Err(SessionError::Unsupported)
            };
        }
        SessionOperation::Resolve { .. } => {
            return if capabilities.artifact_resolution {
                Ok(())
            } else {
                Err(SessionError::Unsupported)
            };
        }
        SessionOperation::Capture { fence, .. } => {
            if !capabilities.chunk_transfer || !capabilities.capture {
                return Err(SessionError::Unsupported);
            }
            fence
        }
        SessionOperation::Materialize { fence, .. } => {
            if !capabilities.chunk_transfer || !capabilities.materialize {
                return Err(SessionError::Unsupported);
            }
            fence
        }
        _ => return Ok(()),
    };
    if !persistence {
        return Err(SessionError::OptedOut);
    }
    if !committed {
        return Err(SessionError::NotCommitted);
    }
    if !same_fence(fence, current) {
        return Err(SessionError::Stale);
    }
    Ok(())
}
