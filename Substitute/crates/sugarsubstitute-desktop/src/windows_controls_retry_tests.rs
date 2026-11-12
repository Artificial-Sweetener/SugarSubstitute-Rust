// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fake only native button effects; exercise actual transition ordering and fixed layout accounting.
use super::*;
use crate::{MainBackendState, MainRecoveryLabels, main_backend::Recovery};
use std::cell::Cell;
use sugarsubstitute_contracts::{execution::Generation, lifecycle::*, startup::LaunchId};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn current() -> Result<(crate::MainIdentity, LeaseKey), Box<dyn std::error::Error>> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let run = RunKey { owner, instance: InstanceId::new(1)?, generation: RunGeneration::new(1)? };
    Ok((
        crate::MainIdentity { run, launch: LaunchId::new(1)?, navigation: Generation::new(1)? },
        LeaseKey {
            run,
            resource: ResourceKey {
                owner,
                id: ResourceId::new(1)?,
                generation: ResourceGeneration::new(1)?,
            },
            connection: ConnectionId::new(1)?,
            id: LeaseId::new(1)?,
        },
    ))
}
struct Port {
    live: Cell<bool>,
    calls: Vec<char>,
    action: Option<RetryAction>,
    enabled: bool,
    visible: bool,
    fail: Option<char>,
    retire_after: Option<char>,
}
impl Port {
    fn new() -> Self {
        Self {
            live: Cell::new(true),
            calls: Vec::new(),
            action: None,
            enabled: false,
            visible: false,
            fail: None,
            retire_after: None,
        }
    }
    fn effect(&mut self, kind: char) -> Result<(), DesktopError> {
        self.calls.push(kind);
        if self.fail == Some(kind) {
            return Err(DesktopError::new(
                "fake_retry_native",
                std::io::Error::other(kind.to_string()),
            ));
        }
        if self.retire_after == Some(kind) {
            self.live.set(false);
        }
        Ok(())
    }
}
impl RetryPort for Port {
    fn live(&self) -> Result<(), DesktopError> {
        if self.live.get() {
            Ok(())
        } else {
            Err(DesktopError::message("fake_retry_live", "original registration retired"))
        }
    }
    fn action(&mut self, action: Option<RetryAction>) -> Result<(), DesktopError> {
        self.effect(if action.is_some() { 'A' } else { 'D' })?;
        self.action = action;
        Ok(())
    }
    fn enabled(&mut self, enabled: bool) -> Result<(), DesktopError> {
        self.effect(if enabled { 'E' } else { 'd' })?;
        self.enabled = enabled;
        Ok(())
    }
    fn label(&mut self, _: &'static str) -> Result<(), DesktopError> {
        self.effect('L')
    }
    fn visible(&mut self, visible: bool) -> Result<(), DesktopError> {
        self.effect('V')?;
        self.visible = visible;
        Ok(())
    }
}
#[test]
fn initial_fourth_native_peer_has_no_visible_or_enabled_style_and_others_are_unchanged()
-> TestResult {
    let (identity, _) = current()?;
    let hidden = initial_style(
        PaintMode::NativeButtons(identity),
        4,
        sugarsubstitute_ui::availability::Availability { visible: false, enabled: true },
    );
    assert_eq!(hidden & WS_VISIBLE, WINDOW_STYLE::default());
    assert_eq!(hidden & WS_DISABLED, WS_DISABLED);
    assert_eq!(hidden & WS_CHILD, WS_CHILD);
    for id in 1..=3 {
        let existing = initial_style(
            PaintMode::NativeButtons(identity),
            id,
            sugarsubstitute_ui::availability::Availability { visible: true, enabled: true },
        );
        assert_eq!(existing & WS_VISIBLE, WS_VISIBLE);
        assert_eq!(existing & WS_DISABLED, WINDOW_STYLE::default());
    }
    assert_eq!(
        initial_style(
            PaintMode::PreparedPixels,
            4,
            sugarsubstitute_ui::availability::Availability { visible: true, enabled: true }
        ) & WS_VISIBLE,
        WS_VISIBLE
    );
    Ok(())
}
#[test]
fn actual_ready_click_transition_disables_mapping_and_peer_before_observation_delivery()
-> TestResult {
    let (identity, failed) = current()?;
    let mut owner = Recovery::new(identity);
    let labels = MainRecoveryLabels { retry: "Retry", retry_ready: "Retry ready" };
    let mut port = Port::new();
    owner.present(identity, failed, MainBackendState::RetryReady)?;
    apply(&mut port, owner.presentation(labels))?;
    assert!(port.enabled && port.visible);
    assert_eq!(port.calls, ['D', 'd', 'L', 'V', 'E', 'A']);
    let captured = port.action.ok_or("Enabled current native mapping missing")?;
    let observation = owner.consume(captured)?.ok_or("Genuine native click missing")?;
    port.calls.clear();
    apply(&mut port, owner.presentation(labels))?;
    assert!(!port.enabled && port.visible && port.action.is_none());
    assert_eq!(port.calls, ['D', 'd', 'L', 'V', 'd', 'D']);
    assert!(
        matches!(observation, crate::DesktopStartupObservation::BackendRetryRequested { identity: actual, failed: key }
        if actual == identity && key == failed)
    );
    Ok(())
}
#[test]
fn native_effect_failure_or_reentrant_retirement_keeps_mapping_disarmed_and_original_error()
-> TestResult {
    use std::error::Error;
    let (identity, failed) = current()?;
    let mut owner = Recovery::new(identity);
    owner.present(identity, failed, MainBackendState::RetryReady)?;
    let ready = owner.presentation(MainRecoveryLabels { retry: "Retry", retry_ready: "Ready" });
    for kind in ['d', 'L', 'V', 'E'] {
        let mut port = Port::new();
        port.fail = Some(kind);
        let cause = apply(&mut port, ready).err().ok_or("Native effect failure lost")?;
        assert_eq!(
            cause.source().ok_or("Original native cause missing")?.to_string(),
            kind.to_string()
        );
        assert!(port.action.is_none());
        assert!(!port.calls.contains(&'A'));
    }
    let mut invalid = ready;
    invalid.visible = false;
    let mut untouched = Port::new();
    assert!(apply(&mut untouched, invalid).is_err());
    assert!(untouched.calls.is_empty());
    let mut port = Port::new();
    port.retire_after = Some('L');
    assert!(apply(&mut port, ready).is_err());
    assert!(port.action.is_none());
    assert!(!port.calls.contains(&'V'));
    assert!(!port.enabled);
    Ok(())
}
#[test]
fn compiled_largest_retry_layout_fits_original_metadata_and_overflow_rejects() {
    let bytes = Controls::storage_bytes(crate::main_caption::PEERS)
        .and_then(|controls| controls.checked_add(size_of::<crate::main_actor::MainActor>()));
    assert!(bytes.is_some_and(|bytes| bytes <= 512 * 1024));
    assert!(Controls::storage_bytes(usize::MAX).is_none());
    assert_eq!(crate::MainWindowRequest::KNOWN_STORAGE_BYTES, 4 * 1024 * 1024);
}

/// The generic resize boundary cannot construct a Main retry action; only Recovery supplies it.
impl sugarsubstitute_ui::availability::AvailabilityPort for Port {
    type Error = DesktopError;
    fn live(&self) -> Result<(), DesktopError> {
        RetryPort::live(self)
    }
    fn action(&mut self, _available: bool) -> Result<(), DesktopError> {
        RetryPort::action(self, None)
    }
    fn enabled(&mut self, enabled: bool) -> Result<(), DesktopError> {
        RetryPort::enabled(self, enabled)
    }
    fn visible(&mut self, visible: bool) -> Result<(), DesktopError> {
        RetryPort::visible(self, visible)
    }
}

#[test]
fn initially_disabled_visible_control_keeps_its_visible_native_peer() {
    use sugarsubstitute_ui::availability::Availability;
    let visible =
        initial_style(PaintMode::PreparedPixels, 5, Availability { visible: true, enabled: false });
    assert_eq!(visible & WS_VISIBLE, WS_VISIBLE);
    assert_eq!(visible & WS_DISABLED, WS_DISABLED);
    let hidden =
        initial_style(PaintMode::PreparedPixels, 5, Availability { visible: false, enabled: true });
    assert_eq!(hidden & WS_VISIBLE, WINDOW_STYLE::default());
    assert_eq!(hidden & WS_DISABLED, WS_DISABLED);
}

#[test]
fn actual_recovery_caption_resize_keeps_ready_action_and_retrying_disabled_peer() -> TestResult {
    let (identity, failed) = current()?;
    let mut owner = Recovery::new(identity);
    let labels = MainRecoveryLabels { retry: "Retry", retry_ready: "Retry ready" };
    let captions = crate::MainCaptionLabels {
        minimize: "Minimize",
        maximize: "Maximize",
        restore: "Restore",
        close: "Close",
    };
    let mut port = Port::new();
    owner.present(identity, failed, MainBackendState::RetryReady)?;
    let ready = owner.presentation(labels);
    apply(&mut port, ready)?;
    let actual_action = port.action.ok_or("Actual Main retry mapping absent")?;
    let mut retained = caption(1280.0, false, captions, ready);
    // The existing present_retry commits actual native visibility into this retained peer snapshot.
    retained[3].visible = port.visible;
    port.calls.clear();
    for (width, maximized) in [(1600.0, true), (960.0, false)] {
        let resized = caption(width, maximized, captions, owner.presentation(labels));
        super::super::state::validate_inventory(&retained, &resized)?;
        sugarsubstitute_ui::availability::reconcile(
            retained[3].availability(),
            resized[3].availability(),
            &mut port,
        )?;
        assert!(port.visible && port.enabled);
        assert_eq!(port.action, Some(actual_action));
        assert!(port.calls.is_empty());
        retained = resized;
    }
    assert!(owner.consume(actual_action)?.is_some());
    apply(&mut port, owner.presentation(labels))?;
    assert!(port.visible && !port.enabled && port.action.is_none());
    retained[3].visible = port.visible;
    port.calls.clear();
    let resized = caption(1920.0, true, captions, owner.presentation(labels));
    super::super::state::validate_inventory(&retained, &resized)?;
    sugarsubstitute_ui::availability::reconcile(
        retained[3].availability(),
        resized[3].availability(),
        &mut port,
    )?;
    assert!(port.visible && !port.enabled && port.action.is_none());
    assert!(port.calls.is_empty());
    assert!(owner.consume(actual_action)?.is_none());
    Ok(())
}

/// The caption with no update, saving or message showing.
fn caption(
    width: f64,
    maximized: bool,
    captions: crate::MainCaptionLabels,
    retry: crate::main_backend::RetryPresentation,
) -> [crate::AccessibleControl; crate::main_caption::PEERS] {
    let updates = crate::main_updates::CaptionUpdates::default()
        .view(sugarsubstitute_contracts::localization::messages("en"));
    let measure = |text: &str| Ok::<_, std::convert::Infallible>(text.len() as f64 * 6.0);
    match crate::main_caption::caption_controls(
        width, maximized, captions, retry, &updates, measure,
    ) {
        Ok(controls) => controls,
        Err(never) => match never {},
    }
}
