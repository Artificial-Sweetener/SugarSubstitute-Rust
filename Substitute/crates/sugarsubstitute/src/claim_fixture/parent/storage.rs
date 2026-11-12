// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit finite artifact setup and bounded diagnostic reads through real execution.
use super::*;
use std::io::Read;
impl Fixture {
    pub(super) fn setup_workspace(&mut self) -> Result<bool> {
        if self.setup.is_some() || self.setup_finished {
            return Ok(false);
        }
        let workspace = self.options.workspace.clone();
        let receipt = self.options.receipt.clone();
        let spec = self.spec(JobClass::InfrastructureLifecycle, CLAIM_OPERATION_BYTES, true)?;
        let job = self.client.as_ref().ok_or("Claim campaign client missing")?.submit_blocking(
            self.scope.as_ref().ok_or("Claim campaign scope missing")?,
            spec,
            move |context| {
                let _scratch =
                    context.memory().claim(CLAIM_OPERATION_BYTES).map_err(std::io::Error::other)?;
                check_parent_drive(&workspace)?;
                check_parent_drive(&receipt)?;
                std::fs::create_dir(&workspace)?;
                for name in ["installation-a", "installation-b", "installation-stale", "backend"] {
                    std::fs::create_dir(workspace.join(name))?;
                }
                Ok(())
            },
        )?;
        // Paths remain retained by this owner even when a late cancelled result
        // is discarded. This fixture never deletes partially created artifacts.
        self.setup = Some(job);
        Ok(true)
    }
    pub(super) fn read_child_receipt(&mut self) -> Result<bool> {
        let case = self.child.as_ref().ok_or("Claim campaign child missing")?;
        if case.abortive
            || case.read.is_some()
            || case.receipt.is_some()
            || case.family.as_ref().is_none_or(|family| family.progress.exited != Some(37))
        {
            return Ok(false);
        }
        let path = self.options.workspace.join(format!("child-{}.json", case.index));
        let spec = self.spec(JobClass::InfrastructureLifecycle, CLAIM_OPERATION_BYTES, true)?;
        let job = self.client.as_ref().ok_or("Claim campaign client missing")?.submit_blocking(
            self.scope.as_ref().ok_or("Claim campaign scope missing")?,
            spec,
            move |context| {
                let _scratch =
                    context.memory().claim(CLAIM_OPERATION_BYTES).map_err(std::io::Error::other)?;
                let mut data = Vec::new();
                data.try_reserve_exact(65537).map_err(std::io::Error::other)?;
                std::fs::File::open(path)?.take(65537).read_to_end(&mut data)?;
                if data.len() > 65536 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Claim child receipt bound exceeded",
                    ));
                }
                serde_json::from_slice::<ChildReceipt>(&data)
                    .map_err(|cause| std::io::Error::new(std::io::ErrorKind::InvalidData, cause))
            },
        )?;
        self.child.as_mut().ok_or("Claim campaign child missing")?.read = Some(job);
        Ok(true)
    }
}
fn check_parent_drive(path: &std::path::Path) -> std::io::Result<()> {
    let parent =
        path.parent().ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    let canonical = std::fs::canonicalize(parent)?;
    use std::path::{Component, Prefix};
    if !matches!(canonical.components().next(),Some(Component::Prefix(prefix)) if matches!(prefix.kind(),Prefix::Disk(b'D')|Prefix::VerbatimDisk(b'D')))
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Claim artifact parent must resolve to D",
        ));
    }
    Ok(())
}
