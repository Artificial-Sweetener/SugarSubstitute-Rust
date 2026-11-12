// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain only actual server fields used for readiness and installed version comparison.
use super::ReadinessError;
use serde::Deserialize;
use std::fmt;
use sugarsubstitute_contracts::configuration::{
    MAX_ARGUMENT_BYTES, MAX_ARGUMENTS, MAX_TOTAL_ARGUMENT_BYTES,
};
/// One observed `comfy_package_versions` entry, without package installation authority.
#[derive(Clone, Deserialize, Eq, PartialEq)]
pub struct PackageVersion {
    /// Distribution name reported by ComfyUI's installed-package enumeration.
    pub name: String,
    /// Null denotes an uninstalled package.
    #[serde(deserialize_with = "required_nullable")]
    pub installed: Option<String>,
    /// Requirement reported from ComfyUI requirements; null denotes no requirement.
    #[serde(deserialize_with = "required_nullable")]
    pub required: Option<String>,
}
fn required_nullable<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(d)
}
impl fmt::Debug for PackageVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PackageVersion { .. }")
    }
}
/// Observed readiness metadata admitted by the bounded decoder.
/// It cannot construct a trusted backend connection.
pub struct SystemStats {
    wire: WireStats,
}
#[derive(Deserialize)]
pub(super) struct WireStats {
    comfyui_version: String,
    required_frontend_version: String,
    python_version: String,
    pytorch_version: String,
    comfy_package_versions: Vec<PackageVersion>,
    argv: Vec<String>,
}
impl fmt::Debug for SystemStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SystemStats")
            .field("package_count", &self.wire.comfy_package_versions.len())
            .field("argv_count", &self.wire.argv.len())
            .finish_non_exhaustive()
    }
}
impl SystemStats {
    /// Borrow the server's ComfyUI version claim.
    pub fn comfyui_version(&self) -> &str {
        &self.wire.comfyui_version
    }
    /// Borrow the server's expected frontend version claim.
    pub fn required_frontend_version(&self) -> &str {
        &self.wire.required_frontend_version
    }
    /// Borrow the observed distribution version entries.
    pub fn packages(&self) -> &[PackageVersion] {
        &self.wire.comfy_package_versions
    }
    /// Borrow exact observed script argv, without executable or listener authority.
    pub fn argv(&self) -> &[String] {
        &self.wire.argv
    }
}
impl WireStats {
    pub(super) fn validate(self) -> Result<SystemStats, ReadinessError> {
        for text in [&self.comfyui_version, &self.required_frontend_version, &self.pytorch_version]
        {
            if !bounded_text(text, 1024) {
                return Err(ReadinessError::Shape);
            }
        }
        // Python's sys.version may include build/compiler line breaks. It is
        // retained as bounded metadata and never rendered by diagnostics.
        if self.python_version.is_empty()
            || self.python_version.len() > 1024
            || self
                .python_version
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
        {
            return Err(ReadinessError::Shape);
        }
        if self.comfy_package_versions.len() > 64
            || self.argv.is_empty()
            || self.argv.len() > MAX_ARGUMENTS
        {
            return Err(ReadinessError::Shape);
        }
        for (index, p) in self.comfy_package_versions.iter().enumerate() {
            if !bounded_text(&p.name, 256)
                || self.comfy_package_versions[..index].iter().any(|other| other.name == p.name)
                || [&p.installed, &p.required]
                    .iter()
                    .any(|v| v.as_ref().is_some_and(|s| !bounded_text(s, 256)))
            {
                return Err(ReadinessError::Shape);
            }
        }
        let mut total = 0;
        for arg in &self.argv {
            if arg.contains('\0') || arg.len() > MAX_ARGUMENT_BYTES {
                return Err(ReadinessError::Shape);
            }
            total += arg.len();
        }
        if total > MAX_TOTAL_ARGUMENT_BYTES {
            return Err(ReadinessError::Shape);
        }
        Ok(SystemStats { wire: self })
    }
}
pub(super) fn bounded_text(s: &str, limit: usize) -> bool {
    !s.is_empty() && s.len() <= limit && !s.chars().any(char::is_control)
}
#[derive(Deserialize)]
pub(super) struct Response {
    pub system: WireStats,
    // Require the actual top-level collection, without overfitting GPU hardware.
    pub devices: Vec<serde::de::IgnoredAny>,
}
