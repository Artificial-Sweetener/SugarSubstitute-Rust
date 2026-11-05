// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compact native canvas surface request lifecycle records survive the bounded stderr relay.
//!
//! The desktop child emits one closed record per lifecycle step of a surface request: its
//! admission, the effect it waits for, each change in why its reply condition is unmet, and its
//! terminal reply. The parent's authenticated output owner decodes only complete records of this
//! exact shape and relays their fields. Records hold integers and fixed tokens only; file names,
//! item identities, paths and payload bytes never enter them.

/// Declare a closed token set whose spelling is both its emitted and its parsed form.
macro_rules! tokens {
    ($(#[$meta:meta])* $name:ident { $($(#[$variant_meta:meta])* $variant:ident),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub(crate) enum $name {
            $($(#[$variant_meta])* $variant),+
        }
        impl $name {
            const ALL: &'static [Self] = &[$(Self::$variant),+];
            /// The exact token written to and accepted from the diagnostic line.
            const fn token(self) -> &'static str {
                match self {
                    $(Self::$variant => stringify!($variant)),+
                }
            }
            /// Accept only an exact token of this closed set.
            fn parse(token: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|value| value.token() == token)
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.token())
            }
        }
    };
}

tokens! {
    /// One step in a surface request's lifecycle.
    Stage {
        /// The request passed attachment, ordering and availability checks.
        Admitted,
        /// The request now waits for a later native or worker effect.
        Pending,
        /// The request's selection was offered to the canvas session.
        Offered,
        /// The pending request's reply condition is unmet for a new reason.
        Waiting,
        /// The pending selection now waits on the session's rebuild of its corrupt cached source.
        Rebound,
        /// The pending viewport request outlived its settle budget; its reply follows.
        Expired,
        /// The request's terminal reply was queued for native delivery.
        Replied,
        /// The request's navigation retired first, so no page remains to receive a reply.
        Retired,
        /// A cancellation matched no pending request; any earlier reply stands.
        Unmatched,
    }
}

tokens! {
    /// The requested surface operation.
    Operation {
        Prepare,
        Geometry,
        Select,
        /// A selection with a comparison result shown through a wipe.
        Compare,
        Overview,
        Viewport,
        Commit,
        Release,
        Preview,
        Cancel,
    }
}

tokens! {
    /// The native or worker effect a pending request waits for.
    Pending {
        None,
        Prepare,
        Restore,
        Select,
        Geometry,
        Hide,
        Commit,
        EmptyCommit,
        Viewport,
    }
}

tokens! {
    /// Why a pending selection's or viewport request's reply condition is currently unmet.
    Wait {
        None,
        /// The viewport's navigation is still moving toward its destination.
        Moving,
        /// The selected source is still being acquired or decoded.
        Acquiring,
        /// The selected source loaded but its frame is not yet prepared, or the viewport's
        /// load, backing render or display is still running.
        Preparing,
        /// The frame is prepared but host graphics are not yet available.
        Graphics,
        /// The committed attachment's frame awaits its native presentation acknowledgement.
        Presenting,
        /// A frame was presented but its accepted camera is not this selection's, or no work
        /// is running and the accepted camera is not the viewport's destination.
        Accepting,
        /// The committed attachment or the canvas is hidden, so no frame can be presented.
        Hidden,
        /// The session's source intent no longer matches the one this selection offered.
        Superseded,
    }
}

tokens! {
    /// The terminal reply status, or `None` before a reply.
    Status {
        None,
        Ready,
        Applied,
        Stale,
        Cancelled,
        Unavailable,
        Error,
    }
}

/// The leading hexadecimal characters of an attachment identity, enough to correlate records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Attachment([u8; Self::LENGTH]);

impl Attachment {
    const LENGTH: usize = 8;
    /// The page-chosen prefix of every attachment identity.
    const PREFIX: &'static str = "attachment:";

    /// Retain only a lowercase hexadecimal prefix; any other identity shape is not recorded.
    pub(crate) fn of(attachment_id: &str) -> Option<Self> {
        Self::parse(attachment_id.strip_prefix(Self::PREFIX)?.get(..Self::LENGTH)?)
    }

    fn parse(token: &str) -> Option<Self> {
        let bytes: [u8; Self::LENGTH] = token.as_bytes().try_into().ok()?;
        bytes.iter().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')).then_some(Self(bytes))
    }

    fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or("none")
    }
}

/// One closed lifecycle step of a surface request, correlated by its page identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Record {
    pub(crate) stage: Stage,
    pub(crate) operation: Operation,
    /// Page request identity; cancellation records carry the cancelled request's identity.
    pub(crate) request_id: u64,
    /// Attachment revision of the request, or zero when the record has none.
    pub(crate) revision: u64,
    /// Attachment generation of the request's address, or zero when the record has none.
    pub(crate) generation: u64,
    pub(crate) attachment: Option<Attachment>,
    pub(crate) pending: Pending,
    pub(crate) wait: Wait,
    /// Source intent the pending selection offered, or zero.
    pub(crate) intent: u64,
    /// The session's current source intent when the record was taken, or zero.
    pub(crate) current: u64,
    pub(crate) status: Status,
}

/// The exact producer target, which the parser requires before the closed fields.
const TARGET: &str = "sugarsubstitute::canvas_surface_diagnostic: ";
/// The closed operation code shared by producer and parser.
const MARKER: &str = "operation=\"canvas_surface_request\" ";
/// The relay's line bound; longer lines are never decoded.
const LINE_BYTES: usize = 512;

impl Record {
    /// Emit the schema-one record at `info`, in the field order [`Record::parse`] requires.
    #[cfg(all(windows, feature = "native-composition"))]
    pub(crate) fn emit(self) {
        tracing::info!(
            operation = "canvas_surface_request",
            schema = 1,
            stage = %self.stage,
            op = %self.operation,
            request_id = self.request_id,
            revision = self.revision,
            generation = self.generation,
            attachment = self.attachment.as_ref().map_or("none", Attachment::as_str),
            pending = %self.pending,
            wait = %self.wait,
            intent = self.intent,
            current = self.current,
            status = %self.status
        );
    }

    /// Decode exact schema-one fields from this producer. Unknown or extra data is discarded.
    #[cfg(all(windows, feature = "windows-native-host"))]
    pub(crate) fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > LINE_BYTES {
            return None;
        }
        let line = std::str::from_utf8(bytes).ok()?.trim_end_matches('\r');
        let (prefix, fields) = line.split_once(MARKER)?;
        if prefix.contains("operation=") || !prefix.ends_with(TARGET) {
            return None;
        }
        let mut fields = fields.split_ascii_whitespace();
        let mut field = |name: &str| fields.next()?.strip_prefix(name)?.strip_prefix('=');
        if field("schema")? != "1" {
            return None;
        }
        let stage = Stage::parse(field("stage")?)?;
        let operation = Operation::parse(field("op")?)?;
        let request_id = number(field("request_id")?)?;
        let revision = number(field("revision")?)?;
        let generation = number(field("generation")?)?;
        let attachment = match field("attachment")?.strip_prefix('"')?.strip_suffix('"')? {
            "none" => None,
            token => Some(Attachment::parse(token)?),
        };
        let pending = Pending::parse(field("pending")?)?;
        let wait = Wait::parse(field("wait")?)?;
        let intent = number(field("intent")?)?;
        let current = number(field("current")?)?;
        let status = Status::parse(field("status")?)?;
        if fields.next().is_some() {
            return None;
        }
        Some(Self {
            stage,
            operation,
            request_id,
            revision,
            generation,
            attachment,
            pending,
            wait,
            intent,
            current,
            status,
        })
    }

    /// Relay the decoded fields as the parent's own structured event for this child run.
    #[cfg(all(windows, feature = "windows-native-host"))]
    pub(crate) fn relay(self, run: sugarsubstitute_contracts::lifecycle::RunKey) {
        tracing::info!(
            operation = "desktop_child_canvas_surface",
            run = ?run,
            stage = %self.stage,
            op = %self.operation,
            request_id = self.request_id,
            revision = self.revision,
            generation = self.generation,
            attachment = self.attachment.as_ref().map_or("none", Attachment::as_str),
            pending = %self.pending,
            wait = %self.wait,
            intent = self.intent,
            current = self.current,
            status = %self.status,
            "Observed owned child's canvas surface request lifecycle"
        );
    }
}

/// Accept canonical unsigned decimal only, rejecting signs, padding and overflow.
#[cfg(all(windows, feature = "windows-native-host"))]
fn number(raw: &str) -> Option<u64> {
    if raw.is_empty() || (raw.len() > 1 && raw.starts_with('0')) {
        return None;
    }
    if !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    raw.parse().ok()
}

#[cfg(all(test, windows, feature = "native-composition", feature = "windows-native-host"))]
#[path = "canvas_surface_diagnostic/tests.rs"]
mod tests;
