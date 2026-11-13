// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native update offer routing and package admission, independent of discovery and signatures.
use crate::installation::PreparationIntent;
use serde::{Deserialize, Serialize};
pub mod attempt;
pub mod pack;
pub mod status;
pub mod subscription;

/// Maximum byte size of the small authenticated channel descriptor.
pub const MAX_OFFER_BYTES: usize = 8192;

/// Explicit subscription; a new installation follows Release.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    /// Normal production updates.
    #[default]
    Release,
    /// Opt-in development updates.
    Canary,
}
/// Independently versioned application and updater payloads.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Component {
    /// Desktop application payload.
    Application,
    /// Installation/update engine payload.
    Updater,
}
/// Package platform; naming does not imply that a platform has passed native qualification.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Platform {
    /// Windows on x86-64.
    #[serde(rename = "windows-x86_64")]
    WindowsX86_64,
    /// Windows on ARM64.
    WindowsAarch64,
    /// macOS on x86-64.
    #[serde(rename = "macos-x86_64")]
    MacosX86_64,
    /// macOS on ARM64.
    MacosAarch64,
    /// Linux on x86-64.
    #[serde(rename = "linux-x86_64")]
    LinuxX86_64,
    /// Linux on ARM64.
    LinuxAarch64,
}
impl Platform {
    /// Stable platform spelling used in signed channel target names.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WindowsX86_64 => "windows-x86_64",
            Self::WindowsAarch64 => "windows-aarch64",
            Self::MacosX86_64 => "macos-x86_64",
            Self::MacosAarch64 => "macos-aarch64",
            Self::LinuxX86_64 => "linux-x86_64",
            Self::LinuxAarch64 => "linux-aarch64",
        }
    }
}

/// Locally owned subscription and compatibility facts; never populate from a remote offer.
#[derive(Clone, Copy)]
pub struct Request {
    /// Explicit user subscription.
    pub channel: Channel,
    /// Actual host platform.
    pub platform: Platform,
    /// Component being checked.
    pub component: Component,
    /// Healthy release sequence known locally for this route, never a merely prepared candidate.
    /// Protected release history separately enforces all authorized publication floors.
    pub sequence_floor: u64,
    /// Archive digest accepted at that sequence; absent exactly when the floor is zero.
    pub sequence_digest: Option<[u8; 32]>,
    /// Running updater's monotonically increasing release sequence.
    pub updater_release: u64,
    /// Update protocol understood by this engine.
    pub updater_protocol: u32,
    /// Package schema understood by this engine.
    pub package_schema: u32,
    /// Explicit user repair permits retry of the current sequence, never a downgrade.
    pub intent: PreparationIntent,
}
impl Request {
    /// Flat signed target name, suitable for GitHub release assets and local mirrors.
    pub fn target_name(&self) -> String {
        let component = match self.component {
            Component::Application => "application",
            Component::Updater => "updater",
        };
        let channel = match self.channel {
            Channel::Release => "release",
            Channel::Canary => "canary",
        };
        format!("{component}-{channel}-{}.json", self.platform.as_str())
    }
}

/// Offer decoding or routing failed; no installation mutation is authorized.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rejection {
    /// Unknown schema, malformed JSON, unexpected fields or invalid values.
    Format,
    /// Offer belongs to a different product, component, platform or subscription.
    Routing,
    /// Offer exceeds its fixed byte bound.
    Bound,
    /// Release sequence predates an accepted release.
    Rollback,
    /// An accepted release sequence was reused for different package bytes.
    Identity,
    /// Local release history is incomplete and must be recovered before admitting an offer.
    LocalState,
}
impl std::fmt::Display for Rejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Update offer rejected: {self:?}")
    }
}
impl std::error::Error for Rejection {}

/// Package transport admission only; data migration and activation have separate owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Admission {
    /// This release is already accepted and no repair was requested.
    Current,
    /// Package routing and engine compatibility permit preparation.
    Prepare,
    /// Authenticate and install a compatible updater before preparing this package.
    UpdateEngine,
    /// This engine cannot consume the package/protocol; use a compatible installer.
    Installer,
    /// The installation withholds this package after repeated bad trials; nothing is downloaded
    /// until the withholding expires or a newer release is published.
    Withheld,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    schema_version: u32,
    product: String,
    channel: Channel,
    platform: Platform,
    component: Component,
    sequence: u64,
    version: String,
    package_sha256: String,
    package_bytes: u64,
    package_schema: u32,
    updater_protocol: u32,
    minimum_updater_release: u64,
}
/// Validated channel descriptor; successful decoding alone proves no authenticity.
pub struct Offer {
    wire: Wire,
    digest: [u8; 32],
}
impl Offer {
    /// Decode a bounded descriptor without trusting an update URL or executable command.
    /// # Errors
    /// Rejects malformed/unknown formats, unbounded values and noncanonical archive digests.
    pub fn decode(bytes: &[u8]) -> Result<Self, Rejection> {
        if bytes.len() > MAX_OFFER_BYTES {
            return Err(Rejection::Bound);
        }
        let wire: Wire = serde_json::from_slice(bytes).map_err(|_| Rejection::Format)?;
        if wire.schema_version != 1
            || wire.sequence == 0
            || wire.version.is_empty()
            || wire.version.len() > 64
            || !wire.version.bytes().all(|b| b.is_ascii_alphanumeric() || b".-+".contains(&b))
            || wire.package_bytes == 0
            || wire.package_schema == 0
            || wire.updater_protocol == 0
            || wire.package_sha256.len() != 64
            || !wire
                .package_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Rejection::Format);
        }
        let mut digest = [0; 32];
        for (index, byte) in digest.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&wire.package_sha256[index * 2..index * 2 + 2], 16)
                .map_err(|_| Rejection::Format)?;
        }
        Ok(Self { wire, digest })
    }
    /// Evaluate locally owned subscription, release history and engine compatibility.
    /// # Errors
    /// Rejects cross-channel/platform/component offers and sequence rollback, including repair.
    pub fn admission(&self, request: &Request) -> Result<Admission, Rejection> {
        let offer = &self.wire;
        if (request.sequence_floor == 0) != request.sequence_digest.is_none() {
            return Err(Rejection::LocalState);
        }
        if offer.product != "sugarsubstitute"
            || offer.channel != request.channel
            || offer.platform != request.platform
            || offer.component != request.component
        {
            return Err(Rejection::Routing);
        }
        if offer.sequence < request.sequence_floor {
            return Err(Rejection::Rollback);
        }
        if offer.sequence == request.sequence_floor && request.sequence_digest != Some(self.digest)
        {
            return Err(Rejection::Identity);
        }
        if offer.sequence == request.sequence_floor
            && request.intent == PreparationIntent::Automatic
        {
            return Ok(Admission::Current);
        }
        if offer.minimum_updater_release > request.updater_release {
            return Ok(Admission::UpdateEngine);
        }
        if offer.updater_protocol != request.updater_protocol
            || offer.package_schema != request.package_schema
        {
            return Ok(Admission::Installer);
        }
        Ok(Admission::Prepare)
    }
    /// Signed release sequence; presentation version strings never determine ordering.
    pub fn sequence(&self) -> u64 {
        self.wire.sequence
    }
    /// Minimum engine release required by this descriptor; authentication remains separate.
    pub fn minimum_updater_release(&self) -> u64 {
        self.wire.minimum_updater_release
    }
    /// Bounded user-facing version label.
    pub fn version(&self) -> &str {
        &self.wire.version
    }
    /// SHA-256 of the complete update archive, distinct from the installation manifest hash.
    pub fn package_digest(&self) -> [u8; 32] {
        self.digest
    }
    /// Exact signed archive size in bytes.
    pub fn package_bytes(&self) -> u64 {
        self.wire.package_bytes
    }
    /// Content-addressed package target; no arbitrary URL or path comes from the descriptor.
    pub fn package_name(&self) -> String {
        format!("{}.ssupdate", self.wire.package_sha256)
    }
}

#[cfg(test)]
mod tests;
