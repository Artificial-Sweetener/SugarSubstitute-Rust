// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Throwaway keys, release files and an installation that checks with the application's verifier.
//!
//! The installation discovers metadata in the order the application's update repository owner
//! does and keeps its trust checkpoint between checks, as installed copies persist it.

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use sugarsubstitute_contracts::{
    installation::PreparationIntent,
    update::{Admission, Channel, Component, Platform, Request},
};
use sugarsubstitute_update::{MetadataSet, RepositoryTrust};
use sugarsubstitute_update_repository::{Error, Files, NewTarget, Payloads, keys::SigningKey};

pub(super) type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A fixed start time; every test time is derived from it, never from a clock.
pub(super) const START: i64 = 1_800_000_000;
pub(super) const DAY: i64 = 24 * 60 * 60;

/// A throwaway key from a fixed seed; never use these seeds for a real repository.
pub(super) fn key(seed: u8) -> Result<SigningKey, Error> {
    SigningKey::from_seed(&[seed; 32])
}

/// The installation's subscription: release-channel application updates for Windows x64.
pub(super) fn request() -> Request {
    Request {
        channel: Channel::Release,
        platform: Platform::WindowsX86_64,
        component: Component::Application,
        sequence_floor: 0,
        sequence_digest: None,
        updater_release: 1,
        updater_protocol: 1,
        package_schema: 1,
        intent: PreparationIntent::Automatic,
    }
}

/// A package and the channel descriptor that offers it, as release CI would produce them.
pub(super) struct Release {
    pub(super) package: Vec<u8>,
    pub(super) package_name: String,
    pub(super) descriptor: Vec<u8>,
}
impl Release {
    pub(super) fn new(sequence: u64, version: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let package = format!("package bytes of {version}").into_bytes();
        let digest = hex(&Sha256::digest(&package));
        let descriptor = serde_json::to_vec(&serde_json::json!({
            "schema_version": 1, "product": "sugarsubstitute", "channel": "release",
            "platform": "windows-x86_64", "component": "application", "sequence": sequence,
            "version": version, "package_sha256": digest, "package_bytes": package.len(),
            "package_schema": 1, "updater_protocol": 1, "minimum_updater_release": 0
        }))?;
        Ok(Self { package, package_name: format!("{digest}.ssupdate"), descriptor })
    }

    /// The release's two targets: the package, then its descriptor.
    pub(super) fn targets(&self) -> Vec<NewTarget> {
        vec![
            target(&self.package_name, &self.package, false),
            target(&request().target_name(), &self.descriptor, true),
        ]
    }

    pub(super) fn payloads(&self) -> Memory {
        Memory(BTreeMap::from([(self.package_name.clone(), self.package.clone())]))
    }
}

pub(super) fn target(name: &str, bytes: &[u8], descriptor: bool) -> NewTarget {
    NewTarget {
        name: name.into(),
        length: bytes.len() as u64,
        sha256: Sha256::digest(bytes).into(),
        descriptor: descriptor.then(|| bytes.to_vec()),
    }
}

pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Packages held in memory.
pub(super) struct Memory(pub(super) BTreeMap<String, Vec<u8>>);
impl Payloads for Memory {
    fn stream(
        &self,
        name: &str,
        sink: &mut dyn FnMut(&[u8]) -> Result<(), Error>,
    ) -> Result<bool, Error> {
        let Some(bytes) = self.0.get(name) else { return Ok(false) };
        for chunk in bytes.chunks(7) {
            sink(chunk)?;
        }
        Ok(true)
    }
}

/// An installed copy: its shipped root and its persisted trust checkpoint.
pub(super) struct Installation {
    checkpoint: Vec<u8>,
}
impl Installation {
    pub(super) fn new(shipped_root: &[u8]) -> Result<Self, sugarsubstitute_update::Error> {
        Ok(Self { checkpoint: RepositoryTrust::bootstrap(shipped_root)?.checkpoint()? })
    }

    /// One update check at `now`: the offered version, after verifying its package bytes.
    /// The checkpoint is kept even when the check fails, as the application keeps it.
    pub(super) fn check(
        &mut self,
        files: &Files,
        package: &BTreeMap<String, Vec<u8>>,
        now: i64,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut trust = RepositoryTrust::from_trusted_checkpoint(&self.checkpoint)?;
        let outcome = Self::discover(&mut trust, files, package, now);
        self.checkpoint = trust.checkpoint()?;
        outcome
    }

    fn discover(
        trust: &mut RepositoryTrust,
        files: &Files,
        package: &BTreeMap<String, Vec<u8>>,
        now: i64,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let fetch = |name: &str| files.get(name).ok_or_else(|| format!("404 {name}"));
        while let Some(root) = files.get(&format!("{}.root.json", trust.next_root_version()?)) {
            trust.rotate(root)?;
        }
        let timestamp = fetch("timestamp.json")?;
        let snapshot =
            fetch(&format!("{}.snapshot.json", trust.snapshot_version(timestamp, now)?))?;
        let version = trust.targets_version(timestamp, snapshot, now)?;
        let targets = fetch(&format!("{version}.targets.json"))?;
        trust.refresh(MetadataSet { timestamp, snapshot, targets }, now)?;
        let offer = trust.offer(&request(), fetch(&request().target_name())?, now)?;
        if offer.admission() != Admission::Prepare {
            return Err(format!("unexpected admission {:?}", offer.admission()).into());
        }
        let label = offer.offer().version().to_owned();
        let name = offer.offer().package_name();
        let bytes = package.get(&name).ok_or_else(|| format!("404 {name}"))?;
        let mut stream = offer.into_package()?.verifier(now)?;
        stream.push(bytes)?;
        stream.finish(now)?;
        Ok(label)
    }
}
