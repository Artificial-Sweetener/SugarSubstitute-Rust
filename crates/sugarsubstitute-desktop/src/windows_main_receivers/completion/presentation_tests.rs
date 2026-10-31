// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify independent notification storage through actual CDP callbacks and retirement.

use super::*;
use crate::{
    SurfacePresentation,
    main_native_delivery::{DeliveryCorrelation, NativeDelivery},
};
use std::error::Error;
use sugarsubstitute_contracts::{
    canvas_surface::{
        CanvasSurfaceAddress, CanvasSurfacePresentationChange, CanvasSurfacePresentationDisplay,
        CanvasSurfacePresentationStatus,
    },
    execution::{Generation, MemoryLease, MemoryRelease},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    startup::{LaunchId, MainIdentity},
};

struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}

fn delivery(lease: &MemoryLease) -> Result<OwnedDelivery, Box<dyn Error>> {
    let identity = MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([7; 32]),
                epoch: CoordinatorEpoch::new([8; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    };
    let address = CanvasSurfaceAddress::decode(br#"{"sessionId":"canvas:w","viewId":"view:a","windowId":"window:a","attachmentId":"attachment:a","generation":1}"#)?;
    let change = CanvasSurfacePresentationChange::new(
        address,
        1,
        "preview:one".into(),
        1,
        CanvasSurfacePresentationStatus::Failed,
        CanvasSurfacePresentationDisplay::Empty,
        "".into(),
    )?;
    Ok(OwnedDelivery {
        context: 17,
        correlation: DeliveryCorrelation::Presentation,
        delivery: NativeDelivery::Presentation(SurfacePresentation::new(
            identity,
            &change,
            lease.claim(SurfacePresentation::STORAGE_BYTES)?,
        )?),
    })
}

#[test]
fn late_successful_notification_callback_keeps_storage_until_actor_or_close_observes_return()
-> Result<(), Box<dyn Error>> {
    for closing in [false, true] {
        let events_lease = MemoryLease::new(4 * 1024 * 1024, Box::new(Release));
        let events = Events::new(&events_lease, || {})?;
        let operations = Operations::new(events.clone())?;
        let lease = MemoryLease::new(SurfacePresentation::STORAGE_BYTES, Box::new(Release));
        let id = operations.arm(Some(delivery(&lease)?))?;
        operations.retire_completed();
        assert_eq!(operations.pending(), 1);
        assert!(lease.claim(1).is_err());
        assert!(operations.take_delivery(id).is_err());
        let handler: ICoreWebView2CallDevToolsProtocolMethodCompletedHandler = Completed {
            owner: operations.clone(),
            id,
            kind: Command::OwnedPanelDelivery { context: 17, id },
        }
        .into();
        let response: Vec<_> =
            r#"{"result":{"type":"boolean","value":true}}"#.encode_utf16().chain([0]).collect();
        // SAFETY: This real callback borrows the retained valid NUL-terminated test result.
        unsafe { handler.Invoke(HRESULT(0), PCWSTR(response.as_ptr())) }?;
        assert!(!events.in_callback());
        assert_eq!(operations.pending(), 1);
        assert!(lease.claim(1).is_err());
        if closing {
            operations.retire_completed();
        } else {
            let completed = operations.take_delivery(id)?;
            assert!(matches!(completed.correlation, DeliveryCorrelation::Presentation));
            assert_eq!(completed.delivery.lane(), None);
            assert_eq!(completed.delivery.request_id(), None);
            assert!(lease.claim(1).is_err());
            drop(completed);
        }
        assert_eq!(operations.pending(), 0);
        assert!(lease.claim(SurfacePresentation::STORAGE_BYTES).is_ok());
    }
    Ok(())
}

#[test]
fn failed_notification_callback_reports_cause_and_retires_only_after_actual_return()
-> Result<(), Box<dyn Error>> {
    let events_lease = MemoryLease::new(4 * 1024 * 1024, Box::new(Release));
    let events = Events::new(&events_lease, || {})?;
    let operations = Operations::new(events.clone())?;
    let lease = MemoryLease::new(SurfacePresentation::STORAGE_BYTES, Box::new(Release));
    let id = operations.arm(Some(delivery(&lease)?))?;
    let handler: ICoreWebView2CallDevToolsProtocolMethodCompletedHandler = Completed {
        owner: operations.clone(),
        id,
        kind: Command::OwnedPanelDelivery { context: 17, id },
    }
    .into();
    // SAFETY: A failed HRESULT makes the callback ignore the absent result pointer.
    unsafe { handler.Invoke(windows::Win32::Foundation::E_FAIL, PCWSTR::null()) }?;
    assert!(!events.in_callback());
    assert!(events.take_failure().is_some());
    assert_eq!(operations.pending(), 1);
    assert!(lease.claim(1).is_err());
    operations.retire_completed();
    assert_eq!(operations.pending(), 0);
    assert!(lease.claim(SurfacePresentation::STORAGE_BYTES).is_ok());
    Ok(())
}
