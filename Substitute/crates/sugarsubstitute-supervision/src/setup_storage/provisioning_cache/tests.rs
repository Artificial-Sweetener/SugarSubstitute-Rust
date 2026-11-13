// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise delegated cache recovery against actual retained native containment and original admission.
use super::*;
use crate::{
    native_process::cache_fixture::CacheFamily,
    setup_storage::test_worker::{self as worker, TestResult, pipeline::Pipeline},
};

#[test]
fn delegated_tools_override_inherited_profile_cache_and_temporary_paths() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let cache = RuntimeCache::open(root.path(), cache::CachePolicy::default(), context)?;
        let mut grants = ProvisioningCaches::prepare(&cache, &"e".repeat(64), context)?;
        let snapshot =
            "tEmP=C:\\outside\0UV_CACHE_DIR=C:\\outside\0HTTP_PROXY=http://proxy.invalid\0\0"
                .encode_utf16()
                .collect::<Vec<_>>();
        let original = crate::LaunchEnvironment::from_snapshot_utf16(&snapshot, &[])?;
        let environment = grants.environment(&original)?;
        let rows = String::from_utf16(environment.as_utf16())?;
        assert!(!rows.contains(r"C:\outside"));
        for name in [
            "TEMP",
            "TMP",
            "HOME",
            "USERPROFILE",
            "APPDATA",
            "LOCALAPPDATA",
            "UV_CACHE_DIR",
            "PIP_CACHE_DIR",
            "npm_config_cache",
            "HF_HOME",
            "TORCH_HOME",
        ] {
            assert!(
                rows.split('\0')
                    .any(|row| row == format!("{name}={}", grants.runtime.path().display()))
            );
        }
        for setting in [
            "UV_NO_CACHE=1",
            "PIP_NO_CACHE_DIR=1",
            "PYTHONDONTWRITEBYTECODE=1",
            "PYTHONNOUSERSITE=1",
            "HTTP_PROXY=http://proxy.invalid",
        ] {
            assert!(rows.split('\0').any(|row| row == setting));
        }
        assert!(String::from_utf16(original.as_utf16())?.contains(r"C:\outside"));
        grants.finish_settled(Default::default(), context)?;
        Ok(())
    })
}

#[test]
fn native_provisioning_grants_survive_host_pin_loss_and_recover_on_an_eight_mib_worker()
-> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (root, cache, mut family, path) = pipeline.run(32 * 1024 * 1024, |context| {
        let root = tempfile::tempdir()?;
        let cache = RuntimeCache::open(root.path(), cache::CachePolicy::default(), &context)?;
        let grants = ProvisioningCaches::prepare(&cache, &"a".repeat(64), &context)?;
        let family = CacheFamily::named(grants.family(), &context)?;
        let path = grants.downloads.path()?.join("partial");
        std::fs::write(&path, b"resumable bytes")?;
        drop(grants);
        Ok((root, cache, family, path))
    })?;
    let (root, cache, family, path) = pipeline.run(8 * 1024 * 1024, move |context| {
        recover_provisioning_cache(&cache, &context)?;
        for namespace in
            [Namespace::SetupDownloads, Namespace::UpstreamMetadata, Namespace::SetupRuntime]
        {
            let mut entries = cache.pending(namespace, &context)?;
            assert!(entries.next(&context)?.ok_or("Live native grant lost")?.producer().is_some());
            assert!(entries.next(&context)?.is_none());
        }
        assert_eq!(std::fs::read(&path)?, b"resumable bytes");
        family.close()?;
        Ok((root, cache, family, path))
    })?;
    pipeline.run(8 * 1024 * 1024, move |context| {
        recover_provisioning_cache(&cache, &context)?;
        for namespace in [Namespace::SetupDownloads, Namespace::UpstreamMetadata] {
            assert!(cache.pending(namespace, &context)?.next(&context)?.is_none());
        }
        assert_eq!(std::fs::read(&path)?, b"resumable bytes");
        assert_eq!(cache.maintain(cache::utc_seconds()?, &context)?.unfinished_entries, 0);
        drop((family, cache, root));
        Ok(())
    })?;
    Ok(())
}

#[test]
fn damaged_provisioning_recovery_facts_preserve_bytes_and_do_not_claim_retirement() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let cache = RuntimeCache::open(root.path(), cache::CachePolicy::default(), context)?;
        let mut lease = cache.clone().acquire(
            Namespace::SetupDownloads,
            &cache::storage_key(b"damaged descriptor"),
            16,
            cache::utc_seconds()?,
            context,
        )?;
        lease.begin_generated("setup-worker-v1:damaged", context)?;
        let path = lease.path().join("partial");
        std::fs::write(&path, b"preserve")?;
        drop(lease);
        assert!(recover_provisioning_cache(&cache, context).is_err());
        assert_eq!(std::fs::read(path)?, b"preserve");
        assert_eq!(cache.maintain(cache::utc_seconds()?, context)?.unfinished_entries, 1);
        Ok(())
    })
}

#[test]
fn pressure_retires_only_selected_grants_after_native_settlement() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let cache = RuntimeCache::open(root.path(), cache::CachePolicy::default(), context)?;
        let mut grants = ProvisioningCaches::prepare(&cache, &"b".repeat(64), context)?;
        let mut family = CacheFamily::named(grants.family(), context)?;
        let download = grants.downloads.path()?;
        let metadata = grants.metadata.path()?;
        let runtime = grants.runtime.path();
        std::fs::write(download.join("partial"), b"reclaim")?;
        std::fs::write(metadata.join("catalogue"), b"keep")?;
        std::fs::write(runtime.join("cache"), b"session")?;
        family.close()?;
        grants.finish_settled(
            ProvisioningCachePressure { downloads: true, metadata: false },
            context,
        )?;
        assert_eq!(cache.maintain(cache::utc_seconds()?, context)?.removed_entries, 0);
        drop(grants);
        assert_eq!(cache.maintain(cache::utc_seconds()?, context)?.removed_entries, 2);
        assert!(!download.exists());
        assert!(!runtime.exists());
        assert_eq!(std::fs::read(metadata.join("catalogue"))?, b"keep");
        Ok(())
    })
}
