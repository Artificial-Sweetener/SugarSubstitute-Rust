// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit trusted bootstrap identity and preserve independent session/storage boundaries.
use super::*;
use crate::{
    MainCaptionLabels, MainRecoveryLabels, MainWindowRequest, MainWindowSettings,
    ServedFrontendVersions,
};
use std::{error::Error, path::PathBuf};
use sugarsubstitute_contracts::{
    configuration::ProfileId,
    execution::{Generation, MemoryRelease},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    startup::LaunchId,
};
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}
pub(super) fn bootstrap() -> Result<SessionBootstrap> {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../third_party/sugarcubes-session/session_vectors.json"
    ))?;
    Ok(SessionBootstrap::new(
        serde_json::from_value(vectors["scope"].clone())?,
        LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?,
        "00000000-0000-4000-8000-000000000001".into(),
    )?)
}
fn identity() -> Result<MainIdentity> {
    Ok(MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    })
}
fn request() -> Result<MainWindowRequest> {
    #[cfg(windows)]
    let path = PathBuf::from(r"D:\owned-profile");
    #[cfg(not(windows))]
    let path = PathBuf::from("/owned-profile");
    Ok(MainWindowRequest::new(
        identity()?,
        LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?,
        ProfileId::new("real".into())?,
        path,
        ServedFrontendVersions::new("1.47.11".into(), "0.15.3".into())?,
        MainWindowSettings {
            logical_width: 1280,
            logical_height: 800,
            dark: true,
            labels: MainCaptionLabels {
                minimize: "Minimize",
                maximize: "Maximize",
                restore: "Restore",
                close: "Close",
            },
            recovery: MainRecoveryLabels { retry: "Retry", retry_ready: "Retry ready" },
        },
        MemoryLease::new(MainWindowRequest::RESERVATION_BYTES, Box::new(Release)),
    )?)
}
#[test]
fn session_bootstrap_keeps_existing_profile_spelling_and_independent_origin_webview() -> Result {
    let original = bootstrap()?;
    assert_eq!(original.scope().profile, "real");
    assert!(!original.media_enabled());
    let mut request = request()?;
    request.set_session_bootstrap(Some(original.clone().with_media(true)))?;
    assert!(request.session_bootstrap().is_some_and(SessionBootstrap::media_enabled));
    let mut scope = original.scope().clone();
    scope.profile = "Real".into();
    let foreign = SessionBootstrap::new(scope, original.origin(), original.webview().into())?;
    assert!(request.set_session_bootstrap(Some(foreign)).is_err());
    assert_eq!(
        request.session_bootstrap().map(|value| value.scope().profile.as_str()),
        Some("real")
    );
    request.set_session_bootstrap(None)?;
    assert!(request.session_bootstrap().is_none());
    Ok(())
}
#[test]
fn bootstrap_refuses_noncanonical_or_overallocated_identities_before_retention() -> Result {
    let original = bootstrap()?;
    let mut scope = original.scope().clone();
    scope.installation = "00000000-0000-4000-8000-000000000001".into();
    assert!(SessionBootstrap::new(scope, original.origin(), original.webview().into()).is_err());
    let mut webview = String::with_capacity(1024 * 1024);
    webview.push_str(original.webview());
    assert!(SessionBootstrap::new(original.scope().clone(), original.origin(), webview).is_err());
    Ok(())
}
#[test]
fn browser_default_port_origin_is_canonical_and_session_partition_is_separate() -> Result {
    assert_eq!(origin_text(LoopbackOrigin::parse("http://localhost:80", 80)?), "http://localhost");
    let mut request = request()?;
    request.set_session_bootstrap(Some(bootstrap()?))?;
    let partition =
        request.storage_lease().claim(MainWindowRequest::SESSION_STORAGE_BYTES)?.into_lease();
    assert!(partition.claim(MainWindowRequest::SESSION_STORAGE_BYTES).is_ok());
    let panel = request.storage_lease().claim(MainWindowRequest::BULK_TRANSFER_BYTES)?;
    assert_eq!(panel.bytes(), MainWindowRequest::BULK_TRANSFER_BYTES);
    assert!(partition.claim(MainWindowRequest::BULK_TRANSFER_BYTES).is_err());
    Ok(())
}
#[test]
fn host_command_checks_exact_scope_origin_and_original_admitted_charge() -> Result {
    let bootstrap = bootstrap()?;
    let message = SessionMessage {
        protocol: "sugarcubes.desktop-session".into(),
        version: 1,
        request_id: bootstrap.webview().into(),
        scope: bootstrap.scope().clone(),
        origin: origin_text(bootstrap.origin()),
        operation: SessionOperation::Capture {
            fence: SessionFence {
                webview: bootstrap.webview().into(),
                revision: 1,
                bindings: vec![],
            },
            checkpoint_id: "00000000-0000-4000-8000-000000000002".into(),
            revision: 3,
        },
    };
    let lease = MemoryLease::new(SessionCommand::STORAGE_BYTES, Box::new(Release));
    let tiny = lease.claim(1)?;
    assert!(
        SessionCommand::new(identity()?, message.clone(), tiny, bootstrap.scope(), &message.origin)
            .is_err()
    );
    let claim = lease.claim(SessionCommand::STORAGE_BYTES)?;
    let mut foreign = message.clone();
    foreign.scope.profile = "other".into();
    assert!(
        SessionCommand::new(identity()?, foreign, claim, bootstrap.scope(), &message.origin)
            .is_err()
    );
    let command = SessionCommand::new(
        identity()?,
        message.clone(),
        lease.claim(SessionCommand::STORAGE_BYTES)?,
        bootstrap.scope(),
        &message.origin,
    )?;
    assert_eq!(command.identity(), identity()?);
    assert_eq!(command.message(), &message);
    Ok(())
}
