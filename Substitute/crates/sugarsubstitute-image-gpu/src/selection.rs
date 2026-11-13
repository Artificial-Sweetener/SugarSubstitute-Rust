// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve explicit hardware-model selectors without relying on enumeration order.
use crate::{AdapterCatalog, AdapterDescriptor, AdapterId, Backend, GpuError};
use std::{fmt, str::FromStr};

/// Reported backend/vendor/device identity for a startup selection.
/// A catalog rejects ambiguous matches; identical adapters use their live discovery IDs.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AdapterKey {
    /// Native graphics API.
    pub backend: Backend,
    /// Reported vendor identifier.
    pub vendor: u32,
    /// Reported device identifier.
    pub device: u32,
}
impl fmt::Display for AdapterKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let backend = match self.backend {
            Backend::Vulkan => "vulkan",
            Backend::Metal => "metal",
        };
        write!(formatter, "{backend}:{:04x}:{:04x}", self.vendor, self.device)
    }
}
impl FromStr for AdapterKey {
    type Err = GpuError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut fields = text.split(':');
        let backend = match fields.next() {
            Some("vulkan") => Backend::Vulkan,
            Some("metal") => Backend::Metal,
            _ => return Err(GpuError::invalid("Unknown GPU selector backend")),
        };
        let mut number = || -> Result<u32, GpuError> {
            let text = fields.next().ok_or_else(|| GpuError::invalid("Incomplete GPU selector"))?;
            if text.is_empty()
                || text.len() > 8
                || !text.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(GpuError::invalid(
                    "GPU selector requires bounded hexadecimal identifiers",
                ));
            }
            u32::from_str_radix(text, 16)
                .map_err(|cause| GpuError::new("gpu_selector_number", cause))
        };
        let vendor = number()?;
        let device = number()?;
        if fields.next().is_some() {
            return Err(GpuError::invalid("GPU selector has excess fields"));
        }
        Ok(Self { backend, vendor, device })
    }
}
impl AdapterDescriptor {
    /// Model selector reported for this discovery entry; it is not a physical-device UUID.
    pub const fn key(&self) -> AdapterKey {
        AdapterKey { backend: self.backend, vendor: self.vendor, device: self.device }
    }
}
impl AdapterCatalog {
    /// Resolve one model selector in the current inventory, independent of enumeration order.
    /// # Errors
    /// Rejects absent and ambiguous matches before creating any device.
    pub fn resolve(&self, key: AdapterKey) -> Result<AdapterId, GpuError> {
        resolve(self.adapters(), key)
    }
}
pub(crate) fn resolve<'a>(
    adapters: impl Iterator<Item = &'a AdapterDescriptor>,
    key: AdapterKey,
) -> Result<AdapterId, GpuError> {
    let mut matches = adapters.filter(|adapter| adapter.key() == key);
    let matched = matches
        .next()
        .ok_or_else(|| GpuError::invalid("Selected GPU is absent from the current inventory"))?;
    if matches.next().is_some() {
        return Err(GpuError::invalid(
            "GPU model selector is ambiguous; select a live discovery identity",
        ));
    }
    Ok(matched.id)
}
