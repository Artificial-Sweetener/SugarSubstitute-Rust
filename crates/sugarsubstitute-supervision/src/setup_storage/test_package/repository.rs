// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Genuine signed channel/package fixture shared by native publication qualification.
use crate::setup_storage::{SelectionStore, TrustStore, UpdateAuthorization};
use sha2::{Digest, Sha256};
use std::path::Path;
use sugarsubstitute_contracts::{
    execution::JobContext,
    installation::{InstallationId, InstallationRecord, PreparationIntent},
    update::{Channel, Component, Platform, Request},
};
use sugarsubstitute_update::{ENGINE_PROTOCOL, ENGINE_RELEASE, fixture::*, package};
type Failure = Box<dyn std::error::Error + Send + Sync>;

/// Current signed distribution and independently owned native admission facts.
pub(crate) struct SignedPackage {
    pub(crate) root: Vec<u8>,
    pub(crate) metadata: Set,
    pub(crate) descriptor: Vec<u8>,
    pub(crate) request: Request,
}
impl SignedPackage {
    /// Sign exact descriptor and archive bytes with the fixed boundary-test key.
    pub(crate) fn new(bytes: &[u8], version: u32) -> Result<Self, Failure> {
        Self::component(bytes, version, Component::Application)
    }
    /// Sign a component-specific target without changing its locally supplied engine facts.
    pub(crate) fn component(
        bytes: &[u8],
        version: u32,
        component: Component,
    ) -> Result<Self, Failure> {
        let key = key(92)?;
        let digest = format!("{:x}", Sha256::digest(bytes));
        let descriptor = serde_json::to_vec(&serde_json::json!({
            "schema_version":1, "product":"sugarsubstitute", "channel":"release",
            "platform":"windows-x86_64", "component":component, "sequence":version,
            "version":format!("{version}.0.0"), "package_sha256":digest, "package_bytes":bytes.len(),
            "package_schema":1, "updater_protocol":1, "minimum_updater_release":1,
        }))?;
        let request = Request {
            channel: Channel::Release,
            platform: Platform::WindowsX86_64,
            component,
            sequence_floor: 0,
            sequence_digest: None,
            updater_release: ENGINE_RELEASE,
            updater_protocol: ENGINE_PROTOCOL,
            package_schema: package::SCHEMA_VERSION,
            intent: PreparationIntent::Automatic,
        };
        let metadata = Set::targets(
            &key,
            [version; 3],
            &[(&request.target_name(), &descriptor), (&format!("{digest}.ssupdate"), bytes)],
            EXPIRES,
        )?;
        Ok(Self { root: root(&key, 1, None)?, metadata, descriptor, request })
    }
    /// Establish explicit fresh fixture trust without claiming a healthy application.
    pub(crate) fn initialize(
        &self,
        root: &Path,
        context: &JobContext,
    ) -> Result<TrustStore, Failure> {
        SelectionStore::initialize(
            root,
            &InstallationRecord::empty(InstallationId::new("e".repeat(32))?),
            context,
        )?;
        Ok(TrustStore::initialize(root, [92; 32], &self.root, context)?)
    }
    /// Borrow verification inputs while the actual publication path owns authorization.
    pub(crate) fn authorization<'a>(&'a self, trust: &'a TrustStore) -> UpdateAuthorization<'a> {
        UpdateAuthorization {
            trust,
            metadata: self.metadata.metadata(),
            request: &self.request,
            descriptor: &self.descriptor,
        }
    }
}
