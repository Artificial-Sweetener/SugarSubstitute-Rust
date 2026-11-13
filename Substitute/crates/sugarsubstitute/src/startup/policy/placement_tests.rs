// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fence placement data and failed captures at the actual native-lifetime policy boundary.

use super::*;
use std::num::NonZeroU64;
use sugarsubstitute_contracts::desktop_session::wire::{SessionPlacement, SessionRect};
#[cfg(windows)]
use sugarsubstitute_desktop::DesktopError;
use sugarsubstitute_desktop::MainPlacementObservation;

fn placement(identity: MainIdentity, failed: bool) -> DesktopStartupObservation {
    #[cfg(windows)]
    if failed {
        return DesktopStartupObservation::MainPlacementCaptureFailed {
            identity,
            capture_id: Some(NonZeroU64::MIN),
            cause: DesktopError::from(sugarsubstitute_ui::windows::NativeUiError::new(
                "fixture_capture",
                std::io::Error::other("native placement query failed"),
            )),
        };
    }
    let _ = failed;
    {
        DesktopStartupObservation::MainPlacementObserved(MainPlacementObservation {
            identity,
            sequence: NonZeroU64::MIN,
            capture_id: Some(NonZeroU64::MIN),
            placement: SessionPlacement {
                version: 1,
                bounds: SessionRect { x: 120.0, y: 160.0, width: 1280.0, height: 800.0 },
                maximized: false,
                monitor: "fixture-monitor".into(),
                work_area: SessionRect { x: 0.0, y: 40.0, width: 1920.0, height: 1040.0 },
                dpi: 96.0,
            },
        })
    }
}

#[test]
fn placement_native_policy_preserves_current_payload_and_capture_failure() -> TestResult {
    let (mut owner, mut session) = policy(false)?;
    let identity = owner.identity(&session);
    for failed in if cfg!(windows) { &[false, true][..] } else { &[false][..] } {
        let failed = *failed;
        let route = owner.native(placement(identity, failed), Some(&mut session))?;
        match route {
            StartupRoute::Native(DesktopStartupObservation::MainPlacementObserved(sample)) => {
                assert!(!failed);
                assert_eq!(sample.identity, identity);
                assert_eq!(sample.sequence, NonZeroU64::MIN);
                assert_eq!(sample.capture_id, Some(NonZeroU64::MIN));
                assert_eq!(sample.placement.bounds.width, 1280.0);
            }
            StartupRoute::Native(DesktopStartupObservation::MainPlacementCaptureFailed {
                identity: actual,
                capture_id,
                cause,
            }) => {
                assert!(failed);
                assert_eq!(actual, identity);
                assert_eq!(capture_id, Some(NonZeroU64::MIN));
                assert!(cause.to_string().contains("fixture_capture"));
            }
            _ => return Err("placement data changed route or lost its original payload".into()),
        }
    }
    Ok(())
}

#[test]
fn placement_native_policy_rejects_foreign_launch_navigation_run_and_absent_session() -> TestResult
{
    let (mut owner, mut session) = policy(false)?;
    let current = owner.identity(&session);
    let foreign_run = RunKey { generation: RunGeneration::new(2)?, ..current.run };
    let foreign = [
        MainIdentity { launch: LaunchId::new(2)?, ..current },
        MainIdentity { navigation: Generation::new(2)?, ..current },
        MainIdentity { run: foreign_run, ..current },
    ];
    for failed in if cfg!(windows) { &[false, true][..] } else { &[false][..] } {
        let failed = *failed;
        for identity in foreign {
            assert!(owner.native(placement(identity, failed), Some(&mut session)).is_err());
        }
        assert!(owner.native(placement(current, failed), None).is_err());
    }
    Ok(())
}
