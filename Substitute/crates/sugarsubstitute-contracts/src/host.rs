// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Describe what this host serves on a SugarCubes panel channel.
//!
//! A host answers every panel `ready` with its description first and its configure second. The
//! description lists the protocols the host answers on that channel and the capabilities it
//! declares; the page treats anything absent as unsupported. It is fixed for one panel session.

use crate::panel::{PanelError, PanelSession};

mod wire;

#[cfg(test)]
mod tests;

/// Maximum encoded description, in UTF-8 bytes.
pub const MAX_HOST_DESCRIPTION_BYTES: usize = 4096;
const PROTOCOL: &str = "sugarcubes.host";
const VERSION: u64 = 1;
const MAX_PROTOCOLS: usize = 32;
const MAX_CAPABILITIES: usize = 64;

/// One panel-channel protocol, named by its schema's message name at its fixed envelope version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostProtocol {
    name: &'static str,
    version: u16,
}

impl HostProtocol {
    /// `sugarcubes.panel` version 3: presentation handshake and connectivity heartbeat.
    pub const PANEL: Self = Self { name: "sugarcubes.panel", version: 3 };
    /// `sugarcubes.canvas` version 3: canvas surface discovery, native canvas requests and replies.
    pub const CANVAS: Self = Self { name: "sugarcubes.canvas", version: 3 };
    /// `sugarcubes.memory` version 1: the application memory budget for the page's caches.
    pub const MEMORY: Self = Self { name: "sugarcubes.memory", version: 1 };
    /// `sugarcubes.updates` version 1: the host's update state, check now and restart to apply.
    pub const UPDATES: Self = Self { name: "sugarcubes.updates", version: 1 };

    /// Borrow the protocol's message name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Return the protocol's fixed envelope version.
    pub const fn version(self) -> u16 {
        self.version
    }
}

/// One behaviour the host offers; a changed behaviour receives a new name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostCapability(&'static str);

impl HostCapability {
    /// The configured accent is the system accent, offered beside a custom one.
    pub const SYSTEM_ACCENT: Self = Self("presentation.system-accent");
    /// The host presents output images itself; the page never falls back to browser images.
    pub const HOST_OUTPUT_SURFACE: Self = Self("output.host-surface");

    /// Borrow the capability name.
    pub const fn name(self) -> &'static str {
        self.0
    }
}

/// What one host serves and declares for one panel session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostDescription {
    protocols: Vec<HostProtocol>,
    capabilities: Vec<HostCapability>,
}

impl HostDescription {
    /// Describe a host that serves `protocols` and declares `capabilities`.
    ///
    /// # Errors
    /// Rejects a description without the panel protocol, a repeated protocol or capability,
    /// and lists beyond the schema's bounds.
    pub fn new(
        protocols: Vec<HostProtocol>,
        capabilities: Vec<HostCapability>,
    ) -> Result<Self, PanelError> {
        let repeated_protocol = protocols
            .iter()
            .enumerate()
            .any(|(index, protocol)| protocols[..index].iter().any(|p| p.name == protocol.name));
        let repeated_capability = capabilities
            .iter()
            .enumerate()
            .any(|(index, capability)| capabilities[..index].contains(capability));
        if repeated_protocol
            || repeated_capability
            || !protocols.contains(&HostProtocol::PANEL)
            || protocols.len() > MAX_PROTOCOLS
            || capabilities.len() > MAX_CAPABILITIES
        {
            return Err(PanelError::Invalid);
        }
        Ok(Self { protocols, capabilities })
    }

    /// Return whether the host answers `protocol` on its panel channel.
    pub fn serves(&self, protocol: HostProtocol) -> bool {
        self.protocols.contains(&protocol)
    }

    /// Return whether the host declares `capability`.
    pub fn declares(&self, capability: HostCapability) -> bool {
        self.capabilities.contains(&capability)
    }

    /// Encode the description addressed to `session`.
    ///
    /// # Errors
    /// Rejects generated-contract drift, encoding failure or an exceeded byte bound.
    pub fn encode(&self, session: &PanelSession) -> Result<Vec<u8>, PanelError> {
        let value = wire::HostDescriptionMessage {
            protocol: PROTOCOL.into(),
            version: VERSION,
            session: session.as_str().into(),
            r#type: "description".into(),
            protocols: self
                .protocols
                .iter()
                .map(|protocol| wire::HostProtocol {
                    name: protocol.name.into(),
                    version: u64::from(protocol.version),
                })
                .collect(),
            capabilities: self
                .capabilities
                .iter()
                .map(|capability| wire::HostCapability { name: capability.0.into() })
                .collect(),
        };
        if !value.valid() {
            return Err(PanelError::Invalid);
        }
        let encoded = serde_json::to_vec(&value).map_err(PanelError::Malformed)?;
        if encoded.len() > MAX_HOST_DESCRIPTION_BYTES {
            return Err(PanelError::TooLarge);
        }
        Ok(encoded)
    }
}
