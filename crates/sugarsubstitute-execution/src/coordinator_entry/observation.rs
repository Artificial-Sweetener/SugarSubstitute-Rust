// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Expose closed actual transport facts while preserving separate root authority.
use super::{AuthenticatedEntryLaunch, EntryTransportError, VerifiedEntryPeer};
use sugarsubstitute_contracts::{
    coordinator_entry::{
        EntryConnectionId, EntryDiscoveryPhase, EntryOperationId, EntryOutcome, EntryRequestId,
    },
    lifecycle::CoordinatorKey,
};
/// Take actual bounded native transport observations; none grant coordinator authority.
#[derive(Debug)]
pub enum EntryTransportObservation {
    /// Actual credential-checked endpoint preparation completed under its original finite budget.
    Prepared,
    /// Actual connected kernel peer passed current PID/birth/token/session/image checks.
    ConnectionVerified {
        /// Exact nonreused connection lifetime.
        connection: EntryConnectionId,
        /// Cached actual kernel metadata; root validates its retained native authority separately.
        peer: VerifiedEntryPeer,
    },
    /// The actual server's closed discovery response matched the original request and source.
    DiscoveryVerified {
        /// Exact authenticated connection.
        connection: EntryConnectionId,
        /// Initializing grants no coordinator authority; Serving carries matching owner facts.
        phase: EntryDiscoveryPhase,
        /// Actual pinned kernel peer facts.
        peer: VerifiedEntryPeer,
    },
    /// Actual closed reply matched current authenticated owner, nonce and original request.
    OutcomeReceived {
        /// Exact authenticated connection.
        connection: EntryConnectionId,
        /// Original source-counter-derived request identity.
        request: EntryRequestId,
        /// Typed admission result, distinct from child or backend readiness.
        outcome: EntryOutcome,
    },
    /// A Launch passed actual kernel recheck and unchanged source/epoch/nonce validation.
    RequestAuthenticated {
        /// Exact authenticated connection.
        connection: EntryConnectionId,
        /// Charged immutable frame; root admission requires a successful borrowed view.
        request: AuthenticatedEntryLaunch,
    },
    /// Actual native write completed; peer consumption remains a separate fact.
    Written {
        /// Exact authenticated connection.
        connection: EntryConnectionId,
        /// Original root-issued operation identity.
        operation: EntryOperationId,
        /// Actual complete written byte count.
        bytes: u32,
    },
    /// Actual I/O, callbacks and peer handles retired for this connection lifetime.
    ConnectionRetired {
        /// Exact retired connection; an outstanding immutable frame loan still retains storage.
        connection: EntryConnectionId,
        /// Original disconnected-requester write failure, after checked native retirement.
        /// This connection-local observation does not fail the serving session.
        failure: Option<EntryTransportError>,
    },
    /// Actual worker activated authenticated Serving metadata under unchanged preparation.
    ServerActivated {
        /// Matching owner facts; root retains the actual election and claims proof.
        owner: CoordinatorKey,
    },
}
