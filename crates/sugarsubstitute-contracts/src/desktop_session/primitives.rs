// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate portable session spelling without filesystem or backend authority.
use super::{
    MAX_DOCUMENT_BYTES,
    wire::{SessionFile, SessionScope},
};

/// Match canonical non-nil UUIDs using the same grammar as the portable TypeScript owner.
pub(super) fn uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                *byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(byte)
            }
        })
        && (b'1'..=b'8').contains(&bytes[14])
        && matches!(bytes[19], b'8' | b'9' | b'a' | b'b')
}

/// Match lowercase SHA-256 hex without an algorithm prefix.
pub(super) fn digest(value: &str) -> bool {
    value.len() == 64
        && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Validate every independently scoped namespace.
pub(super) fn scope(value: &SessionScope) -> bool {
    crate::installation::InstallationId::new(value.installation.clone()).is_ok()
        && crate::configuration::ProfileId::validate(&value.profile).is_ok()
        && [&value.workspace, &value.backend].into_iter().all(|id| uuid(id))
}

/// Reject traversal, Windows device/stream names and ambiguous path components.
fn component(value: &str) -> bool {
    let device = value.split('.').next().unwrap_or_default().to_ascii_lowercase();
    !value.is_empty()
        && !matches!(value, "." | "..")
        && !value.ends_with(['.', ' '])
        && !value.chars().any(|character| {
            character <= '\u{1f}' || character == '\u{7f}' || "<>:\"/\\|?*".contains(character)
        })
        && !matches!(device.as_str(), "con" | "prn" | "aux" | "nul")
        && !(device.len() == 4
            && (device.starts_with("com") || device.starts_with("lpt"))
            && matches!(device.as_bytes()[3], b'1'..=b'9'))
}

/// Validate only relative backend descriptors; current approved roots remain server-owned.
pub(super) fn file(value: &SessionFile) -> bool {
    component(&value.filename)
        && (value.subfolder.is_empty() || value.subfolder.split('/').all(component))
        && value.managed_artifact.as_deref().is_none_or(component)
}

/// Bound opaque graph JSON while leaving Comfy graph semantics to its authoritative adapter.
pub(super) fn graph(encoded: &str) -> bool {
    if encoded.len() > MAX_DOCUMENT_BYTES {
        return false;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(encoded) else {
        return false;
    };
    if !value.is_object() {
        return false;
    }
    let mut pending = vec![(&value, 0)];
    while let Some((value, depth)) = pending.pop() {
        if depth > 64 {
            return false;
        }
        match value {
            serde_json::Value::Array(items) => {
                pending.extend(items.iter().map(|item| (item, depth + 1)))
            }
            serde_json::Value::Object(items) => {
                pending.extend(items.values().map(|item| (item, depth + 1)))
            }
            _ => {}
        }
    }
    true
}
