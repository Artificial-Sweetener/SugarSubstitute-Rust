// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Follow ComfyUI's prompt queue over its WebSocket, so the application knows whether a workflow
//! is queued or running without the web interface.
//!
//! ComfyUI sends `{"type":"status","data":{"status":{"exec_info":{"queue_remaining":N}}}}` to
//! every socket when it connects and after each queue change; `N` counts the running prompt and
//! every pending one. Every other message is ignored. A reconnect therefore learns the current
//! queue at once, so a lost connection only loses knowledge, never a state change.
#[cfg(test)]
mod tests;

use serde_json::Value;

/// Whether ComfyUI's queue holds work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueueActivity {
    /// Nothing is running or pending.
    Idle,
    /// A prompt is running or pending.
    Busy,
}

/// The queue activity a ComfyUI WebSocket text message states, or `None` for any other message.
#[must_use]
pub fn queue_activity(message: &str) -> Option<QueueActivity> {
    let message: Value = serde_json::from_str(message).ok()?;
    if message.get("type")?.as_str()? != "status" {
        return None;
    }
    let remaining =
        message.get("data")?.get("status")?.get("exec_info")?.get("queue_remaining")?.as_u64()?;
    Some(if remaining == 0 { QueueActivity::Idle } else { QueueActivity::Busy })
}

#[cfg(windows)]
pub use watch::{QueueObservation, RESERVATION_BYTES, next_change, target};

#[cfg(windows)]
mod watch {
    use super::{QueueActivity, queue_activity};
    use std::io;
    use sugarsubstitute_contracts::{
        configuration::{LoopbackHost, LoopbackOrigin},
        execution::{JobContext, Moment},
    };
    use sugarsubstitute_execution::loopback_websocket::{
        self, Ended, Incoming, LoopbackFamily, Target,
    };

    /// Bytes a watch job reserves.
    pub const RESERVATION_BYTES: u64 = loopback_websocket::RESERVATION_BYTES;

    /// How one watch ended.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum QueueObservation {
        /// The queue now reads differently from what the caller knew.
        Changed(QueueActivity),
        /// The window ended on a healthy connection with no change.
        Quiet,
        /// ComfyUI closed the connection.
        Closed,
    }

    /// The WebSocket endpoint of the ComfyUI at `origin`. `localhost` resolves to IPv4 loopback,
    /// the address ComfyUI listens on by default. The client id is this process's own, because
    /// ComfyUI replaces an earlier socket that presents the same id.
    /// # Errors
    /// Returns the transport's rejection of the endpoint.
    pub fn target(origin: LoopbackOrigin) -> io::Result<Target> {
        let family = match origin.host() {
            LoopbackHost::Ipv4 | LoopbackHost::Localhost => LoopbackFamily::Ipv4,
            LoopbackHost::Ipv6 => LoopbackFamily::Ipv6,
        };
        Target::loopback(
            family,
            origin.port().get(),
            &format!("/ws?clientId=sugarsubstitute-queue-{}", std::process::id()),
        )
    }

    /// Watch until the queue differs from `known`, `until` arrives, or ComfyUI closes. With
    /// `known` absent the first status ComfyUI sends is the change.
    ///
    /// Call only from the body of an admitted async job reserving [`RESERVATION_BYTES`].
    /// # Errors
    /// Returns connect, handshake, protocol and write failures of the transport.
    pub async fn next_change(
        target: &Target,
        context: &JobContext,
        until: Moment,
        known: Option<QueueActivity>,
    ) -> io::Result<QueueObservation> {
        let ended =
            loopback_websocket::watch(target, context, until, |incoming| changed(&incoming, known))
                .await?;
        Ok(match ended {
            Ended::Observed(activity) => QueueObservation::Changed(activity),
            Ended::WindowElapsed => QueueObservation::Quiet,
            Ended::ServerClosed(_) => QueueObservation::Closed,
        })
    }

    /// The activity `incoming` states when it differs from `known`.
    pub(super) fn changed(
        incoming: &Incoming<'_>,
        known: Option<QueueActivity>,
    ) -> Option<QueueActivity> {
        match incoming {
            Incoming::Text(text) => {
                queue_activity(text).filter(|activity| Some(*activity) != known)
            }
            Incoming::Discarded { .. } => None,
        }
    }
}
