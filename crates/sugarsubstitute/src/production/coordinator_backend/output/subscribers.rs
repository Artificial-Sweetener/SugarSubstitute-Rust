// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound subscription state by the existing global lease cap, never by resources times clients.
use super::fanout::{Position, Receipt};
use super::*;
use sugarsubstitute_application::supervision::MAX_LEASES;

pub(in crate::production::coordinator_backend) struct Subscriber {
    pub(in crate::production::coordinator_backend) lease: LeaseKey,
    pub(in crate::production::coordinator_backend) positions: [Position; 2],
    pub(in crate::production::coordinator_backend) serial: u64,
    pub(in crate::production::coordinator_backend) pending: Option<Receipt>,
}
/// All metadata is preclaimed by the backend fixed constructor; no pull allocates a client table.
pub(in crate::production::coordinator_backend) struct Subscribers(Box<[Option<Subscriber>]>);
impl Subscribers {
    pub(in crate::production::coordinator_backend) const fn storage_bytes() -> u64 {
        (MAX_LEASES * std::mem::size_of::<Option<Subscriber>>()) as u64
    }
    pub(in crate::production::coordinator_backend) fn new() -> Result<Self> {
        let mut entries = Vec::new();
        entries.try_reserve_exact(MAX_LEASES)?;
        if entries.capacity() != MAX_LEASES {
            return Err(LifecycleRejection::Overloaded.into());
        }
        entries.resize_with(MAX_LEASES, || None);
        Ok(Self(entries.into_boxed_slice()))
    }
    pub(in crate::production::coordinator_backend) fn existing(
        &self,
        lease: LeaseKey,
    ) -> Result<Option<&Subscriber>> {
        if let Some(client) = self.0.iter().flatten().find(|s| s.lease == lease) {
            return Ok(Some(client));
        }
        if self.0.iter().all(Option::is_some) {
            return Err(LifecycleRejection::Overloaded.into());
        }
        Ok(None)
    }
    /// The authenticated Parent has already validated the exact actual Pending/Granted lease.
    pub(in crate::production::coordinator_backend) fn subscribe(
        &mut self,
        lease: LeaseKey,
    ) -> Result<&mut Subscriber> {
        let index = self
            .0
            .iter()
            .position(|slot| slot.as_ref().is_some_and(|s| s.lease == lease))
            .or_else(|| self.0.iter().position(Option::is_none))
            .ok_or(LifecycleRejection::Overloaded)?;
        Ok(self.0[index].get_or_insert(Subscriber {
            lease,
            positions: [Position::default(); 2],
            serial: 0,
            pending: None,
        }))
    }
    pub(in crate::production::coordinator_backend) fn acknowledge(
        &mut self,
        lease: LeaseKey,
        receipt: &Receipt,
    ) -> Result<()> {
        let client = self
            .0
            .iter_mut()
            .flatten()
            .find(|s| s.lease == lease)
            .ok_or(LifecycleRejection::StaleIdentity)?;
        if receipt.lease != lease || client.pending.as_ref() != Some(receipt) {
            return Err(LifecycleRejection::StaleIdentity.into());
        }
        client.positions[receipt.stream.index()] = receipt.position;
        client.serial = client.serial.max(receipt.serial);
        client.pending = None;
        Ok(())
    }
    pub(in crate::production::coordinator_backend) fn consumed(
        &self,
        key: ResourceKey,
        serial: u64,
    ) -> bool {
        self.0
            .iter()
            .flatten()
            .filter(|s| s.lease.resource == key)
            .all(|s| s.serial >= serial && s.pending.is_none())
    }
    pub(in crate::production::coordinator_backend) fn remove_lease(&mut self, lease: LeaseKey) {
        for slot in &mut self.0 {
            if slot.as_ref().is_some_and(|s| s.lease == lease) {
                *slot = None;
            }
        }
    }
    pub(in crate::production::coordinator_backend) fn remove_client(
        &mut self,
        run: RunKey,
        connection: ConnectionId,
    ) {
        for slot in &mut self.0 {
            if slot.as_ref().is_some_and(|s| s.lease.run == run && s.lease.connection == connection)
            {
                *slot = None;
            }
        }
    }
    pub(in crate::production::coordinator_backend) fn remove_resource(&mut self, key: ResourceKey) {
        for slot in &mut self.0 {
            if slot.as_ref().is_some_and(|s| s.lease.resource == key) {
                *slot = None;
            }
        }
    }
}
