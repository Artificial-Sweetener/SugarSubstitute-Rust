// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify the server end of the actual connected loopback TCP tuple against an owned family.
//! Listener rows and PID-only observations cannot establish managed readiness.
mod attached;
mod table;
mod workspace;
use crate::{
    FamilyBinding, LaunchTicket, RegistryError, WindowsProcessRegistry,
    registry::family_proof::RetainedFamilyProof,
};
pub use attached::AttachedListenerLease;
use sugarsubstitute_contracts::{execution::JobContext, lifecycle::ResourceKey};
pub use table::TcpTableFailure;
pub use workspace::{TCP_OWNER_WORKSPACE_BYTES, TcpOwnerWorkspace};

/// Observe bounded kernel IPv4 loopback listener presence before attempting HTTP.
/// Absence permits delaying a connection attempt; presence grants no peer, family or readiness authority.
/// Actual established peer qualification and complete served-content validation remain mandatory.
/// # Errors
/// Rejects zero ports, stale/non-readiness contexts and incomplete or oversized native tables.
pub fn loopback_listener_present(
    port: u16,
    storage: &mut TcpOwnerWorkspace,
    context: &JobContext,
) -> Result<bool, RegistryError> {
    table::listening(port, storage, context)
}

/// Observe one actual IPv4 loopback connection in network byte order.
/// Values come from the HTTP adapter's connected request, never configured LISTEN facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectedLoopback {
    /// Client TCP port in host byte order.
    pub client_port: u16,
    /// Server TCP port in host byte order.
    pub server_port: u16,
}
impl ConnectedLoopback {
    /// Reject zero ports before performing a kernel query.
    /// # Errors
    /// Neither endpoint can be port zero on an established connection.
    pub fn new(client_port: u16, server_port: u16) -> Result<Self, RegistryError> {
        if client_port == 0 || server_port == 0 {
            return Err(RegistryError::invalid("connected_loopback_ports"));
        }
        Ok(Self { client_port, server_port })
    }
}

/// Bind read-only managed peer qualification to one exact retained registry family.
/// This owner exports no handles, process termination or arbitrary network authority.
pub struct ManagedListenerLease {
    resource: ResourceKey,
    family: RetainedFamilyProof,
}
impl WindowsProcessRegistry {
    /// Bind an already started retained managed launch to its exact resource generation.
    /// Performs cached identity checks only; connected peer qualification runs off the UI thread.
    /// # Errors
    /// Rejects desktop, foreign, stale, pending, closing or settled families.
    pub fn managed_listener_lease(
        &self,
        ticket: LaunchTicket,
        resource: ResourceKey,
    ) -> Result<ManagedListenerLease, RegistryError> {
        if ticket.binding() != FamilyBinding::Managed(resource) {
            return Err(RegistryError::invalid("managed_listener_binding"));
        }
        let family = self
            .retained_family(ticket)
            .map_err(|e| RegistryError::new("managed_listener_family", e))?;
        let identity = self
            .claim_identity(resource.owner)
            .map_err(|e| RegistryError::new("managed_listener_registry", e))?;
        if family.ticket != ticket || !family.identity.same_as(&identity) {
            return Err(RegistryError::invalid("managed_listener_retained_owner"));
        }
        Ok(ManagedListenerLease { resource, family })
    }
}

/// Keep exact observed member PID/birth matching facts without kernel authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackendPeerIdentity {
    process_id: u32,
    creation_time_100ns: u64,
}
impl BackendPeerIdentity {
    /// Return the complete kernel process identity's PID.
    pub const fn process_id(self) -> u32 {
        self.process_id
    }
    /// Return actual process birth in 100ns units since January 1, 1601 UTC.
    pub const fn creation_time_100ns(self) -> u64 {
        self.creation_time_100ns
    }
}
impl ManagedListenerLease {
    /// Return the original managed resource matching fact.
    pub const fn resource(&self) -> ResourceKey {
        self.resource
    }
    /// Observe the exact managed-family stop fence without blocking or invoking native effects.
    pub fn is_live(&self) -> bool {
        self.family.is_live()
    }
    /// Qualify the exact established server-side TCP row and complete retained family membership.
    /// Call before and after each request; both observations must retain the same PID/birth.
    /// # Errors
    /// Rejects ambiguous, absent, changed, exited, unrelated or uncontained peers.
    /// Requires a finite admitted readiness/native lifecycle blocking context.
    pub fn qualify_connection(
        &self,
        connection: ConnectedLoopback,
        storage: &mut TcpOwnerWorkspace,
        context: &JobContext,
    ) -> Result<BackendPeerIdentity, RegistryError> {
        let pid = table::connected_owner(connection, storage, context)?;
        let identity = self.family.qualify_member(pid, context)?;
        if table::connected_owner(connection, storage, context)? != identity.pid {
            return Err(RegistryError::invalid("managed_listener_changed"));
        }
        Ok(BackendPeerIdentity { process_id: identity.pid, creation_time_100ns: identity.birth })
    }
}

#[cfg(feature = "boundary-fixtures")]
pub mod boundary_fixture;
