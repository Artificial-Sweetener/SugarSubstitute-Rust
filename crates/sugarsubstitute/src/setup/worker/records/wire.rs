// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Closed, bounded request shapes and chunked replies for the private worker record protocol.
use serde::Deserialize;
use serde_json::{Value, json};
use std::io;

/// One strictly monotonic operation within the active host command.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Envelope {
    pub(super) version: u32,
    pub(super) serial: u64,
    pub(super) operation: Request,
}
/// Paths, handles, arbitrary names and cleanup operations cannot be requested through this port.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Request {
    Open { create: bool },
    Page { restart: bool },
    Read { child: Option<String>, leaf: String },
    CreateChild { name: String },
    Begin { child: Option<String>, leaf: String, revision: Option<u64> },
    Chunk { offset: usize, hex: String },
    Commit,
}
impl Request {
    /// Validate before admitting native work, including every name and per-frame upload extent.
    pub(super) fn validate(&self) -> io::Result<()> {
        if matches!(self, Self::Begin { child: None, revision: Some(_), .. }) {
            return Err(invalid());
        }
        match self {
            Self::Read { child, leaf } | Self::Begin { child, leaf, .. } => {
                let expected = match child {
                    None => ".substitute-managed.json",
                    Some(child) if attempt(child) => ".substitute-provisioning.json",
                    _ => return Err(invalid()),
                };
                if leaf != expected {
                    return Err(invalid());
                }
            }
            Self::CreateChild { name } if !attempt(name) => return Err(invalid()),
            Self::Chunk { offset, hex }
                if *offset > 65_536
                    || hex.len() > 16_384
                    || hex.len() % 2 != 0
                    || !hex
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) =>
            {
                return Err(invalid());
            }
            _ => {}
        }
        Ok(())
    }
}
/// Feature attempt names remain the canonical lowercase UUID representation.
fn attempt(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
}
pub(super) fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "Invalid native record operation")
}

/// Preserve native error category/code while keeping paths and record contents out of diagnostics.
pub(super) fn response(request: &str, serial: u64, result: io::Result<Vec<Value>>) -> Vec<Value> {
    let items: Vec<Value> = match result {
        Ok(items) => items.into_iter().map(|data| json!({"ok":true,"data":data})).collect(),
        Err(cause) => {
            tracing::warn!(operation="worker_record_io", request_id=request, serial, cause=%cause, "Native record operation failed");
            let code = match cause.kind() {
                io::ErrorKind::NotFound => "ENOENT",
                io::ErrorKind::AlreadyExists => "EEXIST",
                io::ErrorKind::PermissionDenied => "EACCES",
                io::ErrorKind::WouldBlock => "EBUSY",
                io::ErrorKind::Interrupted => "EINTR",
                io::ErrorKind::TimedOut => "ETIMEDOUT",
                _ => "EIO",
            };
            vec![json!({"ok":false,"error":{"code":code,"osCode":cause.raw_os_error()}})]
        }
    };
    let count = items.len();
    items.into_iter().enumerate().map(|(index, data)| json!({
        "version":1,"id":request,"operation":"storage_reply",
        "input":{"version":1,"serial":serial,"index":index,"done":index + 1 == count,"result":data}
    })).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn record_operations_reject_unknown_authority_paths_and_oversized_chunks() {
        for input in [
            json!({"op":"open","create":false,"path":"E:/other"}),
            json!({"op":"read","child":null,"leaf":"user.json"}),
            json!({"op":"read","child":"../other","leaf":".substitute-provisioning.json"}),
            json!({"op":"begin","child":null,"leaf":".substitute-managed.json","revision":null,"replace":true}),
            json!({"op":"chunk","offset":65537,"hex":"00"}),
            json!({"op":"chunk","offset":0,"hex":"a"}),
            json!({"op":"chunk","offset":0,"hex":"AB"}),
            json!({"op":"chunk","offset":0,"hex":"00".repeat(8193)}),
            json!({"op":"remove","child":null}),
        ] {
            assert!(
                serde_json::from_value::<Request>(input.clone())
                    .map_or(true, |request| request.validate().is_err()),
                "accepted {input}"
            );
        }
    }
    #[test]
    fn replies_preserve_error_category_and_fit_the_existing_frame_bound()
    -> Result<(), Box<dyn std::error::Error>> {
        let replies = response("original", 7, Err(io::ErrorKind::NotFound.into()));
        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0]["id"], "original");
        assert_eq!(replies[0]["input"]["serial"], 7);
        assert_eq!(replies[0]["input"]["result"]["error"]["code"], "ENOENT");
        let chunks = (0..8).map(|_| json!({"hex":"ab".repeat(8192),"revision":1})).collect();
        let replies = response("original", 8, Ok(chunks));
        assert_eq!(replies.len(), 8);
        for (index, reply) in replies.iter().enumerate() {
            assert_eq!(reply["input"]["index"], index);
            assert_eq!(reply["input"]["done"], index == 7);
            assert!(
                serde_json::to_vec(reply)?.len()
                    < sugarsubstitute_execution::provisioning_transport::MAX_FRAME_BYTES
            );
        }
        Ok(())
    }
}
