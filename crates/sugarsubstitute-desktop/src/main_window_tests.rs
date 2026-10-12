// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check bounded native-host input and exact origin admission without creating windows.
use super::*;
use crate::main_navigation::{admits_url, root_url};
use std::{
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use sugarsubstitute_contracts::{execution::Generation, lifecycle::RunKey};
use sugarsubstitute_contracts::{
    execution::MemoryRelease,
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration},
};
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
struct Release(Arc<AtomicU64>);
impl MemoryRelease for Release {
    fn release(&self, bytes: u64) {
        self.0.fetch_add(bytes, Ordering::SeqCst);
    }
}
fn settings() -> MainWindowSettings {
    MainWindowSettings {
        logical_width: 1280,
        logical_height: 900,
        dark: true,
        labels: MainCaptionLabels {
            minimize: "Minimize",
            maximize: "Maximize",
            restore: "Restore",
            close: "Close",
        },
    }
}
fn absolute() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(r"D:\owned-profile")
    }
    #[cfg(not(target_os = "windows"))]
    {
        PathBuf::from("/owned-profile")
    }
}
fn request(
    settings: MainWindowSettings,
    path: PathBuf,
    bytes: u64,
    released: Arc<AtomicU64>,
) -> Result<std::result::Result<MainWindowRequest, MainRequestError>> {
    let identity = MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: sugarsubstitute_contracts::lifecycle::RunGeneration::new(1)?,
        },
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    };
    Ok(MainWindowRequest::new(
        identity,
        LoopbackOrigin::parse("http://127.0.0.1:8188/", 8188)?,
        ProfileId::new("desktop-1".into())?,
        path,
        ServedFrontendVersions::new("1.47.11".into(), "0.15.0".into())?,
        settings,
        MemoryLease::new(bytes, Box::new(Release(released))),
    ))
}

#[test]
fn native_request_owns_existing_credit_without_granting_profile_or_process_authority() -> Result {
    let released = Arc::new(AtomicU64::new(0));
    let request =
        request(settings(), absolute(), MainWindowRequest::KNOWN_STORAGE_BYTES, released.clone())??;
    assert_eq!(request.identity().run.generation, RunGeneration::new(1)?);
    assert_eq!(request.profile().as_str(), "desktop-1");
    assert_eq!(request.profile_directory(), absolute());
    assert_eq!(request.storage_lease().bytes(), MainWindowRequest::KNOWN_STORAGE_BYTES);
    assert_eq!(request.versions().frontend(), "1.47.11");
    let remaining = MainWindowRequest::KNOWN_STORAGE_BYTES - request.allocated_metadata_bytes();
    let claim = request.storage_lease().claim(remaining)?;
    drop(request);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(claim);
    assert_eq!(released.load(Ordering::SeqCst), MainWindowRequest::KNOWN_STORAGE_BYTES);
    Ok(())
}
#[test]
fn invalid_size_label_path_or_credit_rejects_before_native_effect() -> Result {
    for dimension in [0, 8193, u32::MAX] {
        let mut invalid = settings();
        invalid.logical_width = dimension;
        assert!(matches!(
            request(invalid, absolute(), 4 * 1024 * 1024, Arc::default())?,
            Err(MainRequestError::Dimensions)
        ));
    }
    let mut invalid = settings();
    invalid.labels.close = "bad\0label";
    assert!(matches!(
        request(invalid, absolute(), 4 * 1024 * 1024, Arc::default())?,
        Err(MainRequestError::CaptionLabel)
    ));
    let mut nul = absolute().into_os_string();
    nul.push("\0value");
    for path in [PathBuf::from("relative"), PathBuf::from(nul)] {
        assert!(matches!(
            request(settings(), path, 4 * 1024 * 1024, Arc::default())?,
            Err(MainRequestError::ProfilePath)
        ));
    }
    assert!(matches!(
        request(settings(), absolute(), 4 * 1024 * 1024 - 1, Arc::default())?,
        Err(MainRequestError::InsufficientMemory)
    ));
    Ok(())
}

#[test]
fn short_data_with_oversized_allocations_is_rejected_before_retention() -> Result {
    let mut frontend = String::with_capacity(1024 * 1024);
    frontend.push('1');
    assert!(matches!(
        ServedFrontendVersions::new(frontend, "1".into()),
        Err(MainRequestError::VersionLabel)
    ));
    let mut path = PathBuf::with_capacity(MainWindowRequest::PROFILE_PATH_BYTES + 1);
    path.push(absolute());
    assert!(matches!(
        request(settings(), path, 4 * 1024 * 1024, Arc::default())?,
        Err(MainRequestError::ProfilePath)
    ));
    let mut namespace = String::with_capacity(1024 * 1024);
    namespace.push_str("profile");
    let profile = ProfileId::new(namespace)?;
    let result = MainWindowRequest::new(
        request(settings(), absolute(), 4 * 1024 * 1024, Arc::default())??.identity(),
        LoopbackOrigin::parse("http://127.0.0.1:8188/", 8188)?,
        profile,
        absolute(),
        ServedFrontendVersions::new("1".into(), "1".into())?,
        settings(),
        MemoryLease::new(4 * 1024 * 1024, Box::new(Release(Arc::default()))),
    );
    assert!(matches!(result, Err(MainRequestError::ProfileCapacity)));
    Ok(())
}

#[test]
fn existing_claims_prevent_retaining_metadata_even_when_reservation_total_is_sufficient() -> Result
{
    let lease = MemoryLease::new(4 * 1024 * 1024, Box::new(Release(Arc::default())));
    let blocked = lease.claim(lease.bytes())?;
    let result = MainWindowRequest::new(
        request(settings(), absolute(), 4 * 1024 * 1024, Arc::default())??.identity(),
        LoopbackOrigin::parse("http://127.0.0.1:8188/", 8188)?,
        ProfileId::new("profile".into())?,
        absolute(),
        ServedFrontendVersions::new("1".into(), "1".into())?,
        settings(),
        lease,
    );
    let error = result.err().ok_or("Oversubscribed metadata retained")?;
    assert!(matches!(error, MainRequestError::MetadataBudget(_)));
    assert!(error.source().is_some());
    drop(blocked);
    Ok(())
}
#[test]
fn diagnostics_never_echo_profile_paths_or_version_payloads() -> Result {
    let failure =
        request(settings(), PathBuf::from("sensitive-profile"), 4 * 1024 * 1024, Arc::default())?
            .err()
            .ok_or("Invalid path accepted")?;
    assert!(!failure.to_string().contains("sensitive-profile"));
    for label in [String::new(), "x".repeat(65), "hidden\0value".into()] {
        let error = ServedFrontendVersions::new(label.clone(), "0.15.0".into())
            .err()
            .ok_or("Invalid label accepted")?;
        assert!(!error.to_string().contains(&label) || label.is_empty());
    }
    Ok(())
}
#[test]
fn exact_loopback_origin_preserves_served_paths_queries_and_fragments() -> Result {
    for (root, port) in
        [("http://127.0.0.1:8188/", 8188), ("http://localhost/", 80), ("http://[::1]:8188/", 8188)]
    {
        let origin = LoopbackOrigin::parse(root, port)?;
        assert!(admits_url(origin, &root_url(origin)));
        assert!(admits_url(
            origin,
            &format!("{}assets/frontend.js?revision=1#part", root_url(origin))
        ));
    }
    Ok(())
}
#[test]
fn url_normalization_cannot_authorize_alternate_hosts_schemes_ports_or_credentials() -> Result {
    let origin = LoopbackOrigin::parse("http://127.0.0.1:8188/", 8188)?;
    for value in [
        "http://localhost:8188/",
        "http://127.1:8188/",
        "http://2130706433:8188/",
        "http://127.0.0.1:08188/",
        "http://user@127.0.0.1:8188/",
        "http://127.0.0.1:8188@external/",
        "https://127.0.0.1:8188/",
        "file:///D:/profile",
        "javascript:alert(1)",
        "http://127.0.0.1:8189/",
        "http://127.0.0.1:8188\\@external/",
        "http://127.0.0.1:8188/\0",
    ] {
        assert!(!admits_url(origin, value), "Invalid URL admitted");
    }
    assert!(!admits_url(origin, &format!("{}{}", root_url(origin), "x".repeat(4096))));
    Ok(())
}
