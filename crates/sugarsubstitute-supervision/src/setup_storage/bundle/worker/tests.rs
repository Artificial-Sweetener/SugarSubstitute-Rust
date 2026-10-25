// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Runtime selection follows authenticated roles, never executable discovery.
use super::*;
use crate::setup_storage::{materialize, test_package, test_worker};
use sha2::{Digest, Sha256};

#[test]
fn runtime_roles_reject_ambiguity_and_require_the_node_capable_engine() -> test_worker::TestResult {
    let (bytes, _) = test_package::distribution()?;
    let mut value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let files = value["files"].as_array_mut().ok_or("Fixture files absent")?;
    files.retain(|entry| entry["path"] != ELECTRON);
    let base = files.clone();
    for (roles, accepted) in [
        (vec![NODE, LICENSE], true),
        (vec![ELECTRON], true),
        (vec![NODE, LICENSE, ELECTRON], false),
        (vec![NODE], false),
        (vec!["worker/runtime/Node.exe", LICENSE], false),
        (vec!["electron/Electron.exe"], false),
        (vec![], false),
    ] {
        let mut entries = base.clone();
        for name in roles {
            entries.push(serde_json::json!({"path":name,"bytes":0,"sha256":"a".repeat(64)}));
        }
        value["files"] = serde_json::json!(entries);
        let manifest = Manifest::decode(&serde_json::to_vec(&value)?)?;
        assert_eq!(uses_node(&manifest).is_ok(), accepted);
        if accepted {
            assert_eq!(require_engine(&manifest, 1).is_ok(), !uses_node(&manifest)?);
            require_engine(&manifest, 2)?;
        }
    }
    Ok(())
}

#[test]
fn authenticated_node_bundle_prepares_its_exact_entry_with_native_cache_grants()
-> test_worker::TestResult {
    let mut pipeline = test_worker::pipeline::Pipeline::new()?;
    let (temporary, cache, bundle) = pipeline.run(32 * 1024 * 1024, |context| {
        let context = &context;
        let temporary = tempfile::tempdir()?;
        let (bytes, compressed) = test_package::distribution()?;
        let mut value: serde_json::Value = serde_json::from_slice(&bytes)?;
        let mut contents = Vec::new();
        std::io::Read::read_to_end(
            &mut flate2::read::GzDecoder::new(compressed.as_slice()),
            &mut contents,
        )?;
        let entries = value["files"].as_array_mut().ok_or("Fixture files absent")?;
        let mut offset = 0;
        for entry in entries.iter_mut() {
            let length = entry["bytes"].as_u64().ok_or("Fixture size absent")? as usize;
            if entry["path"] == ELECTRON {
                entry["path"] = NODE.into();
            }
            offset += length;
        }
        assert_eq!(offset, contents.len());
        let license = b"inert runtime license";
        entries.push(serde_json::json!({"path":LICENSE,"bytes":license.len(),"sha256":format!("{:x}",Sha256::digest(license))}));
        contents.extend_from_slice(license);
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        std::io::Write::write_all(&mut encoder, &contents)?;
        let manifest = serde_json::to_vec(&value)?;
        let digest = format!("{:x}", Sha256::digest(&manifest));
        let source = temporary.path().join("setup.exe");
        std::fs::write(&source, test_package::seal(&manifest, &encoder.finish()?))?;
        let cache = crate::setup_storage::cache::RuntimeCache::open(
            temporary.path(),
            crate::setup_storage::cache::CachePolicy::default(),
            context,
        )?;
        let bundle = materialize(&source, &cache, &digest, context)?;
        let root = bundle.payload.root.clone();
        let worker = bundle.worker()?;
        assert_eq!(worker.executable(), root.join(NODE));
        assert_eq!(worker.directory(), root);
        assert_eq!(
            worker.arguments(),
            &[
                OsString::from("--use-system-ca"),
                "--use-env-proxy".into(),
                "--no-global-search-paths".into(),
                root.join("worker/out/main/index.cjs").into_os_string()
            ]
        );
        assert!(std::fs::write(worker.executable(), b"replacement").is_err());
        Ok((temporary, cache, std::sync::Arc::new(bundle)))
    })?;
    pipeline.run(16 * 1024 * 1024, move |context| {
        let context = &context;
        let worker = bundle.worker()?;
        let input = crate::WindowsLaunchInput::provisioning(
            sugarsubstitute_contracts::lifecycle::CoordinatorKey {
                namespace: sugarsubstitute_contracts::lifecycle::NamespaceId::new([11; 32]),
                epoch: sugarsubstitute_contracts::lifecycle::CoordinatorEpoch::new([12; 16])?,
            },
            bundle,
            &cache,
            temporary.path(),
            crate::registry_input::preparation::ProvisioningEndpoint {
                port: 12345,
                token: "a".repeat(64),
            },
            crate::LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?,
            context,
        )?;
        use std::os::windows::ffi::OsStrExt;
        assert_eq!(
            input.command.application_name,
            worker.executable().as_os_str().encode_wide().chain(Some(0)).collect::<Vec<_>>()
        );
        let command = String::from_utf16(&input.command.command_line)?;
        let entry = worker.arguments().last().ok_or("Authenticated Node entry absent")?;
        assert!(command.contains(entry.to_string_lossy().as_ref()));
        for argument in [
            "--use-system-ca",
            "--use-env-proxy",
            "--no-global-search-paths",
            "--setup-cache-downloads",
            "--setup-cache-metadata",
            "--setup-cache-runtime",
        ] {
            assert!(command.contains(argument), "Prepared Node command omitted {argument}");
        }
        assert!(input.provisioning_downloads().is_ok());
        assert!(input.provisioning_metadata().is_ok());
        Ok(())
    })
}
