// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Define the complete runtime namespace registry and its disk/retention policy.
use super::*;
use serde::{Deserialize, Serialize};
const GIB: u64 = 1024 * 1024 * 1024;
const DAY: u64 = 86400;

/// Independently disposable feature storage. Epoch changes use a new namespace.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Namespace {
    /// Signed discovery bytes, reauthenticated by the update owner on every read.
    UpdateOffers,
    /// Partial and complete archives, rehashed against authenticated targets.
    UpdateDownloads,
    /// Verified extracted distributions; installed generations live elsewhere.
    PackedPayloads,
    /// Installer-only browser data. Application browser sessions are authoritative.
    SetupBrowser,
    /// Upstream provisioning archives and resumable prefixes.
    SetupDownloads,
    /// Reconstructible upstream catalogues and HTTP metadata.
    UpstreamMetadata,
    /// Private headless provisioning session data, never application sessions.
    SetupRuntime,
    /// Reconstructible original-precision raster tiles; authored images remain outside cache.
    ImageSources,
}
impl Namespace {
    pub(super) const ALL: [Self; 8] = [
        Self::UpdateOffers,
        Self::UpdateDownloads,
        Self::PackedPayloads,
        Self::SetupBrowser,
        Self::SetupDownloads,
        Self::UpstreamMetadata,
        Self::SetupRuntime,
        Self::ImageSources,
    ];
    /// Canonical schema/epoch-qualified directory component.
    pub const fn directory(self) -> &'static str {
        match self {
            Self::UpdateOffers => "offer-v1",
            Self::UpdateDownloads => "update-downloads-v1",
            Self::PackedPayloads => "payloads-v1",
            Self::SetupBrowser => "setup-webview-v1",
            Self::SetupDownloads => "setup-downloads-v1",
            Self::UpstreamMetadata => "setup-upstream-metadata-v1",
            Self::SetupRuntime => "setup-runtime-v1",
            Self::ImageSources => "image-sources-v1",
        }
    }
    /// Production byte budget for this namespace, including reserved active entries.
    pub const fn budget(self) -> u64 {
        match self {
            Self::UpdateOffers => 64 * 1024 * 1024,
            Self::UpdateDownloads | Self::SetupDownloads => 24 * GIB,
            Self::PackedPayloads => 32 * GIB,
            Self::ImageSources => 32 * GIB,
            Self::SetupBrowser => GIB,
            Self::UpstreamMetadata | Self::SetupRuntime => 256 * 1024 * 1024,
        }
    }
    /// Maximum idle age in UTC seconds. Clock regression never makes an entry older.
    pub const fn retention(self) -> u64 {
        match self {
            Self::UpdateOffers | Self::UpstreamMetadata => 7 * DAY,
            Self::UpdateDownloads | Self::SetupDownloads => 30 * DAY,
            Self::PackedPayloads => 7 * DAY,
            Self::ImageSources => 7 * DAY,
            Self::SetupBrowser | Self::SetupRuntime => DAY,
        }
    }
}

/// Disk capacity for one explicit cache scope. Reservations precede all producer writes.
#[derive(Clone, Copy, Debug)]
pub struct CachePolicy {
    /// Aggregate byte budget across namespaces, counting active reservation ceilings.
    pub total_bytes: u64,
}
impl Default for CachePolicy {
    fn default() -> Self {
        Self { total_bytes: 64 * GIB }
    }
}
impl CachePolicy {
    pub(super) fn validate(self) -> io::Result<()> {
        if self.total_bytes == 0 {
            return Err(invalid("Cache disk budget must be nonzero"));
        }
        Ok(())
    }
}
