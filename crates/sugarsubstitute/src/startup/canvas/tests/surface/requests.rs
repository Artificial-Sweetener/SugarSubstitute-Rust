// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Remember the surface requests a fixture sent, so a deadlocked wait names what it was owed.

use super::*;

/// One request as sent: its operation, attachment and revision.
#[derive(Debug)]
struct Sent {
    id: u64,
    kind: String,
    attachment: String,
    revision: u64,
}

/// Every request a fixture sent, newest last; a reused id names its latest request.
#[derive(Debug, Default)]
pub(super) struct RequestLog(Vec<Sent>);

impl RequestLog {
    /// Record request `id` of `kind` with the `extra` fields that override the default envelope.
    pub(super) fn record(&mut self, id: u64, kind: &str, extra: &Value) {
        self.0.push(Sent {
            id,
            kind: kind.to_owned(),
            attachment: extra["address"]["attachmentId"]
                .as_str()
                .unwrap_or("attachment:one")
                .to_owned(),
            revision: extra["revision"].as_u64().unwrap_or(id),
        });
    }

    /// Name request `id` as sent.
    pub(super) fn describe(&self, id: u64) -> String {
        self.0.iter().rev().find(|sent| sent.id == id).map_or_else(
            || format!("request {id} (not sent by this fixture)"),
            |sent| {
                format!(
                    "request {} {} to {} at revision {}",
                    sent.id, sent.kind, sent.attachment, sent.revision
                )
            },
        )
    }
}
