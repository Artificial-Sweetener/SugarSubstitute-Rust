// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check bounded native-host input and exact origin admission without creating windows.
use super::*;
use crate::main_navigation::{admits_url, panel_root_url, root_url};
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
        recovery: crate::MainRecoveryLabels {
            retry: "Retry",
            retry_ready: "Connection failed. Select here to retry",
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
        request(settings(), absolute(), MainWindowRequest::RESERVATION_BYTES, released.clone())??;
    assert_eq!(request.identity().run.generation, RunGeneration::new(1)?);
    assert_eq!(request.profile().as_str(), "desktop-1");
    assert_eq!(request.profile_directory(), absolute());
    assert_eq!(request.storage_lease().bytes(), MainWindowRequest::RESERVATION_BYTES);
    assert_eq!(request.versions().frontend(), "1.47.11");
    let remaining = MainWindowRequest::RESERVATION_BYTES - request.allocated_metadata_bytes();
    let claim = request.storage_lease().claim(remaining)?;
    drop(request);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(claim);
    assert_eq!(released.load(Ordering::SeqCst), MainWindowRequest::RESERVATION_BYTES);
    Ok(())
}

#[test]
fn native_placement_request_retains_valid_state_and_releases_replaced_monitor_credit() -> Result {
    use sugarsubstitute_contracts::desktop_session::wire::{SessionPlacement, SessionRect};
    let released = Arc::new(AtomicU64::new(0));
    let mut request =
        request(settings(), absolute(), MainWindowRequest::RESERVATION_BYTES, released)??;
    let initial_bytes = request.allocated_metadata_bytes();
    assert!(request.initial_placement().is_none());
    let placement = SessionPlacement {
        version: 1,
        bounds: SessionRect { x: -1800.0, y: 100.0, width: 1000.0, height: 700.0 },
        maximized: true,
        monitor: "display-left".into(),
        work_area: SessionRect { x: -1920.0, y: 0.0, width: 1920.0, height: 1040.0 },
        dpi: 96.0,
    };
    let remaining = MainWindowRequest::RESERVATION_BYTES - initial_bytes;
    let occupied = request.storage_lease().claim(remaining)?;
    assert!(matches!(
        request.set_initial_placement(Some(placement.clone())),
        Err(MainRequestError::MetadataBudget(_))
    ));
    assert!(request.initial_placement().is_none());
    drop(occupied);
    request.set_initial_placement(Some(placement.clone()))?;
    assert_eq!(request.initial_placement(), Some(&placement));
    assert!(request.allocated_metadata_bytes() > initial_bytes);
    let mut invalid = placement.clone();
    invalid.monitor = "invalid\0monitor".into();
    assert!(matches!(
        request.set_initial_placement(Some(invalid)),
        Err(MainRequestError::Placement)
    ));
    assert_eq!(request.initial_placement(), Some(&placement));
    let mut oversized = placement.clone();
    oversized.monitor.reserve_exact(2048);
    assert!(matches!(
        request.set_initial_placement(Some(oversized)),
        Err(MainRequestError::Placement)
    ));
    assert_eq!(request.initial_placement(), Some(&placement));
    request.set_initial_placement(None)?;
    assert!(request.initial_placement().is_none());
    assert_eq!(request.allocated_metadata_bytes(), initial_bytes);
    assert_eq!(request.storage_lease().claim(remaining)?.bytes(), remaining);
    Ok(())
}
#[test]
fn invalid_size_label_path_or_credit_rejects_before_native_effect() -> Result {
    for dimension in [0, 8193, u32::MAX] {
        let mut invalid = settings();
        invalid.logical_width = dimension;
        assert!(matches!(
            request(invalid, absolute(), MainWindowRequest::RESERVATION_BYTES, Arc::default())?,
            Err(MainRequestError::Dimensions)
        ));
    }
    let mut invalid = settings();
    invalid.labels.close = "bad\0label";
    assert!(matches!(
        request(invalid, absolute(), MainWindowRequest::RESERVATION_BYTES, Arc::default())?,
        Err(MainRequestError::CaptionLabel)
    ));
    let mut nul = absolute().into_os_string();
    nul.push("\0value");
    for path in [PathBuf::from("relative"), PathBuf::from(nul)] {
        assert!(matches!(
            request(settings(), path, MainWindowRequest::RESERVATION_BYTES, Arc::default())?,
            Err(MainRequestError::ProfilePath)
        ));
    }
    assert!(matches!(
        request(settings(), absolute(), MainWindowRequest::RESERVATION_BYTES - 1, Arc::default())?,
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
        request(settings(), path, MainWindowRequest::RESERVATION_BYTES, Arc::default())?,
        Err(MainRequestError::ProfilePath)
    ));
    let mut namespace = String::with_capacity(1024 * 1024);
    namespace.push_str("profile");
    let profile = ProfileId::new(namespace)?;
    let result = MainWindowRequest::new(
        request(settings(), absolute(), MainWindowRequest::RESERVATION_BYTES, Arc::default())??
            .identity(),
        LoopbackOrigin::parse("http://127.0.0.1:8188/", 8188)?,
        profile,
        absolute(),
        ServedFrontendVersions::new("1".into(), "1".into())?,
        settings(),
        MemoryLease::new(MainWindowRequest::RESERVATION_BYTES, Box::new(Release(Arc::default()))),
    );
    assert!(matches!(result, Err(MainRequestError::ProfileCapacity)));
    Ok(())
}

#[test]
fn existing_claims_prevent_retaining_metadata_even_when_reservation_total_is_sufficient() -> Result
{
    let lease =
        MemoryLease::new(MainWindowRequest::RESERVATION_BYTES, Box::new(Release(Arc::default())));
    let blocked = lease.claim(lease.bytes())?;
    let result = MainWindowRequest::new(
        request(settings(), absolute(), MainWindowRequest::RESERVATION_BYTES, Arc::default())??
            .identity(),
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
    let failure = request(
        settings(),
        PathBuf::from("sensitive-profile"),
        MainWindowRequest::RESERVATION_BYTES,
        Arc::default(),
    )?
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
fn normal_main_navigation_identifies_the_current_panel_without_changing_the_owned_origin() -> Result
{
    let request =
        request(settings(), absolute(), MainWindowRequest::RESERVATION_BYTES, Arc::default())??;
    let identity = request.identity();
    let expected = sugarsubstitute_contracts::panel::PanelSession::for_main(identity)?;
    for (root, port) in
        [("http://127.0.0.1:8188/", 8188), ("http://localhost/", 80), ("http://[::1]:8188/", 8188)]
    {
        let origin = LoopbackOrigin::parse(root, port)?;
        let value = panel_root_url(origin, identity)?;
        assert!(admits_url(origin, &value));
        let parsed = url::Url::parse(&value)?;
        assert_eq!(parsed.path(), "/");
        let fields: Vec<_> = parsed.query_pairs().collect();
        assert_eq!(fields.len(), 3);
        assert_eq!(
            fields[0],
            ("sc-panel-host".into(), "ai.artificialsweetener.sugarsubstitute".into())
        );
        assert_eq!(fields[1].0, "sc-panel-session");
        assert_eq!(fields[1].1, expected.as_str());
        assert_eq!(fields[2], ("sc-panel-transport".into(), "native".into()));
        let next = MainIdentity {
            navigation: Generation::new(identity.navigation.get() + 1)?,
            ..identity
        };
        assert_ne!(panel_root_url(origin, next)?, value);
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

#[test]
fn recovery_labels_are_bounded_at_request_admission_before_native_initialization() -> Result {
    for label in [
        "",
        "bad\0label",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ] {
        let mut invalid = settings();
        invalid.recovery.retry_ready = label;
        assert!(matches!(
            request(invalid, absolute(), MainWindowRequest::RESERVATION_BYTES, Arc::default())?,
            Err(MainRequestError::CaptionLabel)
        ));
    }
    Ok(())
}
#[test]
fn every_canonical_locale_retry_pair_fits_the_original_native_request_budget() -> Result {
    for messages in sugarsubstitute_contracts::localization::LOCALES {
        let mut localized = settings();
        localized.recovery =
            MainRecoveryLabels { retry: messages.retry, retry_ready: messages.retry_ready };
        let actual =
            request(localized, absolute(), MainWindowRequest::RESERVATION_BYTES, Arc::default())??;
        assert_eq!(actual.settings().recovery.retry_ready, messages.retry_ready);
        assert_eq!(actual.storage_lease().bytes(), MainWindowRequest::RESERVATION_BYTES);
    }
    Ok(())
}
