// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Define finite backend/profile custody operations and correlated receipts.
use super::*;
/// Matching facts for the two actual installation-trial outputs in the authenticated helper.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct InstallationTrialCustody {
    /// Destination duplicate of the continuously owned installation run exclusion.
    pub run: u64,
    /// Destination duplicate of the protected immutable original trial snapshot.
    pub snapshot: u64,
    /// Original establishment cutoff in Windows performance-counter ticks.
    pub until_ticks: i64,
    /// Original performance-counter ticks per second.
    pub frequency: i64,
}
/// Bounded commands exchanged only through the purpose-specific authenticated session.
/// Handle integers are destination-process matching facts. They confer no control
/// until the native owner adopts their actual output cells and validates type,
/// membership, birth, account and original source allocation.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum BackendCustodyCommand {
    /// Demonstrate coordinator progress on the authenticated custody channel.
    Heartbeat,
    /// Confirm actual Held activation after Prepared, before root creation.
    ProfilePublished {
        /// Same authentic prepared profile/family.
        profile: ProfileCustodyKey,
    },
    /// Adopt the actual empty desktop Job and protected profile directory before Held.
    ProfilePrepare {
        /// Genuine desktop run and original prepared registry family identity.
        profile: ProfileCustodyKey,
        /// Fresh recovery grant correlation, authenticated by the existing Root session.
        /// Ordinary profile preparation retains the strict original wire tag.
        recovery_grant: Option<crate::recovery_supervisor::RecoveryGrantId>,
        /// Destination duplicate of that empty prepared Job.
        job: u64,
        /// Destination duplicate of the pinned protected profile directory.
        directory: u64,
        /// Exact prospective schema-two profile row.
        record: [u8; 160],
        /// Original writer-exclusion file identity.
        claim_identity: [u8; 24],
        /// Original absolute QPC cutoff.
        until_ticks: i64,
        /// Original QPC frequency.
        frequency: i64,
    },
    /// Adopt the same suspended desktop root before original resume commitment.
    ProfileRoot {
        /// Same acknowledged profile/family matching facts.
        profile: ProfileCustodyKey,
        /// Destination process duplicate.
        process: u64,
        /// Actual kernel PID.
        pid: u32,
        /// Actual creation time in100ns units since1601.
        birth: u64,
    },
    /// Request checked retirement of the original profile family and writer.
    ProfileStop {
        /// Same registered profile/family.
        profile: ProfileCustodyKey,
        /// Original cleanup cutoff.
        until_ticks: i64,
        /// Original QPC frequency.
        frequency: i64,
    },
    /// Authenticate this immutable session before custody operations.
    Hello {
        /// Original private bootstrap capability.
        capability: [u8; 32],
    },
    /// Adopt the exact parent observation and helper containment after helper resume.
    Establish {
        /// Destination parent process duplicate.
        parent: u64,
        /// Original parent kernel PID.
        pid: u32,
        /// Original parent creation time in 100ns units since1601.
        birth: u64,
        /// Destination duplicate of this helper's actual containment Job.
        containment: u64,
        /// Installation custody must be validated before acknowledging establishment.
        trial: Option<InstallationTrialCustody>,
    },
    /// Adopt a prepared Job before any backend process creation.
    Prepare {
        /// Original resource generation.
        resource: ResourceKey,
        /// Original exact backend digest.
        digest: [u8; 32],
        /// Destination Job duplicate.
        job: u64,
        /// Original absolute Windows QPC cutoff.
        until_ticks: i64,
        /// Original positive QPC frequency.
        frequency: i64,
        /// Exact prospective version-two record bytes.
        record: [u8; 128],
        /// Original pinned writer-exclusion volume/file identity.
        claim_identity: [u8; 24],
        /// Original registry process identity, distinct from kernel PID.
        registry_process: u64,
    },
    /// Adopt the original suspended root before its actual resume CAS.
    Root {
        /// Same acknowledged resource generation.
        resource: ResourceKey,
        /// Destination root process duplicate.
        process: u64,
        /// Exact kernel PID.
        pid: u32,
        /// Exact creation time in100ns units since1601.
        birth: u64,
        /// Same prepared registry process identity.
        registry_process: u64,
    },
    /// Request same-owner cleanup without replacing the original transaction cutoff.
    Stop {
        /// Exact retained resource generation.
        resource: ResourceKey,
        /// Original absolute cleanup QPC cutoff.
        until_ticks: i64,
        /// Original QPC frequency.
        frequency: i64,
    },
    /// End admission and settle all registrations under the original shutdown.
    Finish {
        /// Original absolute cleanup QPC cutoff.
        until_ticks: i64,
        /// Original QPC frequency.
        frequency: i64,
    },
    /// Return a correlated owned result; receipt facts alone grant no launch authority.
    Receipt(BackendCustodyReceipt),
}

/// Acknowledgements retained by the parent operation's exact source/nonce owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackendCustodyReceipt {
    /// The independently running custodian observed this exact heartbeat.
    Heartbeat,
    /// Exact matching Held row was observed under independent profile custody.
    ProfilePublished(ProfileCustodyKey),
    /// Actual independent profile lock, protected directory and empty Job retained.
    ProfilePrepared(ProfileCustodyKey),
    /// Actual same-Job suspended desktop root adopted.
    ProfileRoot(ProfileCustodyKey),
    /// Actual complete family and exact profile publication settled.
    ProfileReleased(ProfileCustodyKey),
    /// Parent and helper containment duplicates were actually adopted.
    Established,
    /// All actual registrations and temporary helper outputs settled.
    Finished,
    /// Independent custody exclusion and prepared Job are actually retained.
    Prepared(ResourceKey),
    /// The same suspended root duplicate passed actual Job membership/birth checks.
    Root(ResourceKey),
    /// Actual family/outputs settled and same-generation Released publication returned.
    Released(ResourceKey),
}
