// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Author the channel descriptor that offers a sealed update package to installations.
//!
//! A descriptor names its package by SHA-256, carries the release's signed sequence and declares
//! the engine compatibility of the updater code this tool is built from, which is the release's
//! own. Every descriptor is decoded and admitted by the application's offer contract before it
//! is returned, so release CI never signs one an installation would refuse.
use crate::{Error, invalid};
use sugarsubstitute_contracts::{
    installation::PreparationIntent,
    update::{Admission, Channel, Component, MAX_OFFER_BYTES, Offer, Platform, Request},
};
use sugarsubstitute_update::{ENGINE_PROTOCOL, package};

/// Bits of the signed sequence given to each of minor and patch; major takes the rest.
const PART_BITS: u32 = 20;
/// Largest major version whose sequence fits in the 64-bit signed sequence.
const MAX_MAJOR: u64 = (1 << (64 - 2 * PART_BITS)) - 1;

/// Where a descriptor is published: one subscription route of installations.
#[derive(Clone, Copy)]
pub struct Route {
    /// Subscription channel.
    pub channel: Channel,
    /// Package platform.
    pub platform: Platform,
    /// Application or updater payload.
    pub component: Component,
}

impl Route {
    /// A route from its published spellings, such as `release`, `windows-x86_64` and
    /// `application`, exactly as descriptors and their names spell them.
    /// # Errors
    /// Rejects any spelling the offer contract does not define.
    pub fn parse(channel: &str, platform: &str, component: &str) -> Result<Self, Error> {
        let spelled = |kind: &str, value: &str| {
            let text = serde_json::Value::String(value.to_owned());
            (text, invalid(format!("{value} is not a {kind} installations subscribe to")))
        };
        let (text, refusal) = spelled("channel", channel);
        let channel = serde_json::from_value(text).map_err(|_| refusal)?;
        let (text, refusal) = spelled("platform", platform);
        let platform = serde_json::from_value(text).map_err(|_| refusal)?;
        let (text, refusal) = spelled("component", component);
        let component = serde_json::from_value(text).map_err(|_| refusal)?;
        Ok(Self { channel, platform, component })
    }
}
/// A descriptor ready to sign as a release target beside its package.
pub struct Descriptor {
    /// Flat published name installations request for the route.
    pub name: String,
    /// Exact descriptor bytes.
    pub bytes: Vec<u8>,
    /// Signed release sequence it carries.
    pub sequence: u64,
}

/// The signed release sequence of a plain `major.minor.patch` release version.
///
/// Sequences order releases for rollback protection, so they must grow with every release.
/// semantic-release only ever moves a version forward, and this encoding preserves version
/// order exactly: `major << 40 | minor << 20 | patch`, with minor and patch below 2^20.
/// # Errors
/// Rejects anything but a plain release version, and parts too large to encode.
pub fn sequence(version: &str) -> Result<u64, Error> {
    let parts: Vec<u64> = version
        .split('.')
        .map(|part| {
            let plain = !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && (part == "0" || !part.starts_with('0'));
            if plain { part.parse::<u64>().ok() } else { None }
        })
        .collect::<Option<_>>()
        .ok_or_else(|| invalid(format!("{version} is not a plain major.minor.patch version")))?;
    let [major, minor, patch] = *parts.as_slice() else {
        return Err(invalid(format!("{version} is not a plain major.minor.patch version")));
    };
    if major > MAX_MAJOR || minor >= 1 << PART_BITS || patch >= 1 << PART_BITS {
        return Err(invalid(format!("{version} has a part too large for a release sequence")));
    }
    let sequence = (major << (2 * PART_BITS)) | (minor << PART_BITS) | patch;
    if sequence == 0 {
        return Err(invalid("Version 0.0.0 has no release sequence"));
    }
    Ok(sequence)
}

/// The descriptor that offers a package of `package_bytes` bytes with digest `package_sha256`
/// as `version` on `route`, to updaters of at least `minimum_updater_release`.
/// # Errors
/// Rejects versions without a sequence, and a descriptor the application's offer contract
/// would not decode or admit for a fresh installation on the route.
pub fn describe(
    route: Route,
    version: &str,
    minimum_updater_release: u64,
    package_sha256: [u8; 32],
    package_bytes: u64,
) -> Result<Descriptor, Error> {
    let sequence = sequence(version)?;
    let digest: String = package_sha256.iter().map(|byte| format!("{byte:02x}")).collect();
    let bytes = serde_json::to_vec(&serde_json::json!({
        "schema_version": 1,
        "product": "sugarsubstitute",
        "channel": route.channel,
        "platform": route.platform,
        "component": route.component,
        "sequence": sequence,
        "version": version,
        "package_sha256": digest,
        "package_bytes": package_bytes,
        "package_schema": package::SCHEMA_VERSION,
        "updater_protocol": ENGINE_PROTOCOL,
        "minimum_updater_release": minimum_updater_release,
    }))
    .map_err(|cause| invalid(format!("The descriptor could not be encoded: {cause}")))?;
    let request = Request {
        channel: route.channel,
        platform: route.platform,
        component: route.component,
        sequence_floor: 0,
        sequence_digest: None,
        updater_release: minimum_updater_release,
        updater_protocol: ENGINE_PROTOCOL,
        package_schema: package::SCHEMA_VERSION,
        intent: PreparationIntent::Automatic,
    };
    let offer = Offer::decode(&bytes)
        .map_err(|cause| invalid(format!("Installations would refuse the descriptor: {cause}")))?;
    let admission = offer
        .admission(&request)
        .map_err(|cause| invalid(format!("Installations would refuse the descriptor: {cause}")))?;
    if bytes.len() > MAX_OFFER_BYTES
        || admission != Admission::Prepare
        || offer.package_digest() != package_sha256
        || offer.sequence() != sequence
    {
        return Err(invalid("Installations would not prepare the package this descriptor offers"));
    }
    Ok(Descriptor { name: request.target_name(), bytes, sequence })
}
