// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Share signed release identity while native owners keep component-specific storage domains.
use serde::{Deserialize, Serialize};
use sugarsubstitute_contracts::{
    installation::Generation,
    update::{Channel, Component, Platform},
};
pub(super) const MAX_BYTES: usize = 8192;

/// Local identity written under current repository authorization before candidate completion.
/// It records publication identity, not health; protected selection separately supplies health.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Receipt {
    pub(super) schema_version: u32,
    pub(super) generation: Generation,
    pub(super) channel: Channel,
    pub(super) platform: Platform,
    pub(super) component: Component,
    pub(super) sequence: u64,
    pub(super) archive_sha256: [u8; 32],
}
