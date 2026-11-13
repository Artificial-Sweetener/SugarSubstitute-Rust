// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Obtain download authority through the public prepared-launch contract with inert trusted fixtures.
use super::*;
use sha2::{Digest, Sha256};
use sugarsubstitute_supervision::{
    LaunchEnvironment, ProvisioningEndpoint,
    setup_storage::{
        VerifiedBundle,
        cache::{CachePolicy, RuntimeCache},
    },
};

/// Keep source, cache and native preparation pins alive until actual storage work has retired.
pub(super) struct Fixture {
    pub(super) input: WindowsLaunchInput,
    pub(super) cache: RuntimeCache,
    pub(super) root: tempfile::TempDir,
}
/// Retain the actual source-materialization admission separately from launch preparation.
pub(super) struct Source {
    bundle: Arc<VerifiedBundle>,
    cache: RuntimeCache,
    root: tempfile::TempDir,
}
impl Source {
    /// Fixture hashes are explicit local trust anchors; inert executable bytes are never launched.
    pub(super) fn new(context: &JobContext) -> io::Result<Self> {
        let root = tempfile::tempdir()?;
        let bundle = root.path().join("distribution");
        let mut entries = Vec::new();
        for name in [
            "app/sugarsubstitute.exe",
            "app/sugarsubstitute-release-v1.manifest",
            "app/sugarsubstitute-release-v1.sig",
            "app/crashpad_handler.exe",
            "app/sugarsubstitute_crashpad_client.dll",
            "electron/electron.exe",
            "worker/package.json",
            "worker/out/main/index.cjs",
            sugarsubstitute_supervision::pack_install::SUGARCUBES_PACKAGE_ARCHIVE,
        ] {
            let bytes = format!("Inert native download fixture: {name}").into_bytes();
            let path = bundle.join(name);
            std::fs::create_dir_all(
                path.parent().ok_or_else(|| io::Error::other("Fixture parent absent"))?,
            )?;
            std::fs::write(&path, &bytes)?;
            entries.push(serde_json::json!({"path":name,"bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(&bytes))}));
        }
        let name = format!("app/{}", sugarsubstitute_contracts::application_launch::CONTRACT_FILE);
        let bytes = sugarsubstitute_contracts::application_launch::CURRENT_CONTRACT;
        std::fs::write(bundle.join(&name), bytes)?;
        entries.push(serde_json::json!({"path":name,"bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(bytes))}));
        let manifest = serde_json::to_vec(&serde_json::json!({"schema_version":1,"release_id":"native-download-fixture","files":entries})).map_err(io::Error::other)?;
        std::fs::write(bundle.join("setup-manifest.json"), &manifest)?;
        let bundle = Arc::new(VerifiedBundle::open(
            &bundle,
            &format!("{:x}", Sha256::digest(&manifest)),
            context,
        )?);
        let cache =
            RuntimeCache::open(&root.path().join("cache-糖😀"), CachePolicy::default(), context)?;
        std::fs::write(root.path().join("authored"), b"preserve")?;
        std::fs::create_dir(root.path().join("authored-directory"))?;
        std::fs::write(root.path().join("authored-directory/keep"), b"preserve")?;
        Ok(Self { bundle, cache, root })
    }

    /// Use the production preparation budget while source and cache retain their original budget.
    pub(super) fn prepare(
        self,
        port: u16,
        token: String,
        context: &JobContext,
    ) -> io::Result<Fixture> {
        let environment =
            LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[]).map_err(io::Error::other)?;
        let input = WindowsLaunchInput::provisioning(
            CoordinatorKey {
                namespace: NamespaceId::new([0x61; 32]),
                epoch: CoordinatorEpoch::new([0x62; 16]).map_err(io::Error::other)?,
            },
            self.bundle,
            &self.cache,
            self.root.path(),
            ProvisioningEndpoint { port, token },
            environment,
            context,
        )?;
        Ok(Fixture { input, cache: self.cache, root: self.root })
    }
}

/// Create a real junction through a static local OS command; only child environment carries paths.
pub(super) fn junction(link: &std::path::Path, target: &std::path::Path) -> io::Result<()> {
    use std::{os::windows::process::CommandExt, process::Command};
    let windows = std::env::var_os("SystemRoot")
        .ok_or_else(|| io::Error::other("Windows system root absent"))?;
    let output = Command::new(PathBuf::from(windows).join("System32").join("cmd.exe"))
        .raw_arg("/D /V:OFF /C mklink /J \"%SUGARSUBSTITUTE_TEST_LINK%\" \"%SUGARSUBSTITUTE_TEST_TARGET%\"")
        .env("SUGARSUBSTITUTE_TEST_LINK", link).env("SUGARSUBSTITUTE_TEST_TARGET", target)
        .creation_flags(0x0800_0000).output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "Native junction fixture failed: {}; stdout={:?}; stderr={:?}; link={link:?}; target={target:?}",
            output.status,
            String::from_utf8_lossy(&output.stdout[..output.stdout.len().min(8192)]),
            String::from_utf8_lossy(&output.stderr[..output.stderr.len().min(8192)]),
        )));
    }
    Ok(())
}
