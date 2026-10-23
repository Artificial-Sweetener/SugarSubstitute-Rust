// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve drive aliases while native ancestry pins prevent directory replacement.
use super::*;
use std::path::Prefix;
impl Directory {
    /// Normalize a pinned local directory and verify the resolved spelling names the same object.
    pub(in crate::setup_storage) fn resolved(self) -> io::Result<Self> {
        let canonical = std::fs::canonicalize(&self.path)?;
        let mut components = canonical.components();
        let Some(Component::Prefix(prefix)) = components.next() else {
            return Err(invalid("Resolved directory has no local drive"));
        };
        let drive = match prefix.kind() {
            Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => drive,
            _ => return Err(invalid("Resolved directory must remain on a local drive")),
        };
        let mut path = PathBuf::from(format!("{}:", char::from(drive)));
        for component in components {
            path.push(component);
        }
        let resolved = Self::open(&path, false)?;
        if resolved.identity()? != self.identity()? {
            return Err(invalid("Resolved directory identity changed"));
        }
        Ok(resolved)
    }
    /// Cached payload paths require their original lease, including through drive aliases.
    pub(in crate::setup_storage) fn unmanaged_payload(self) -> io::Result<Self> {
        let resolved = self.resolved()?;
        for part in resolved.path.components() {
            if let Component::Normal(name) = part {
                let name = name.to_string_lossy().to_ascii_lowercase();
                if name.strip_prefix("runtime-cache-v").is_some_and(|version| {
                    !version.is_empty() && version.bytes().all(|byte| byte.is_ascii_digit())
                }) {
                    return Err(invalid("Cached payload requires its original storage lease"));
                }
            }
        }
        Ok(resolved)
    }
}
