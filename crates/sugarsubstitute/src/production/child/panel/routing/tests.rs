// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual child fallback routing without a ready policy or a native surface capability.

use super::*;
use sugarsubstitute_desktop::{CanvasNativeReply, SurfaceReply};

struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}

fn surface(identity: MainIdentity, request_id: u64, cancel: bool) -> Result<CanvasSurfaceMessage> {
    let session = PanelSession::for_main(identity)?;
    let value = if cancel {
        serde_json::json!({"protocol":"sugarcubes.canvas", "version":3,
            "session":session.as_str(), "requestId":request_id, "type":"cancel"})
    } else {
        serde_json::json!({"protocol":"sugarcubes.canvas", "version":3,
            "session":session.as_str(), "requestId":request_id, "type":"surface.prepare",
            "revision":1, "address":{"sessionId":"canvas:w", "viewId":"view:a",
                "windowId":"window:a", "attachmentId":"attachment:a", "generation":1}})
    };
    Ok(CanvasSurfaceMessage::decode(&serde_json::to_vec(&value)?)?)
}

fn context(identity: MainIdentity) -> Result<CanvasNativeRequest> {
    Ok(CanvasNativeRequest::decode(&serde_json::to_vec(&serde_json::json!({
        "protocol":"sugarcubes.canvas", "version":3,
        "session":PanelSession::for_main(identity)?.as_str(),
        "requestId":9, "type":"context.request"
    }))?)?)
}

#[test]
fn current_unready_and_failed_policies_both_return_correlated_ordinary_refusals() -> Result<()> {
    for failed in [false, true] {
        let (mut child, _owner, _clock) = crate::production::child::test_support::child()?;
        let identity = child.identity;
        let lease = MemoryLease::new(OWNER_BYTES + SurfaceReply::STORAGE_BYTES, Box::new(Release));
        if failed {
            let mut panel = PanelOwner::new(identity, lease.clone(), Moment::from_nanos(0), None)?
                .ok_or("Panel owner was not admitted")?;
            panel.tick(Moment::from_nanos(30_000_000_000))?;
            assert_eq!(panel.policy.state(), PanelState::Failed);
            child.panel = Some(panel);
        }
        child.receive_surface_message(
            identity,
            surface(identity, 7, false)?,
            lease.clone(),
            lease.claim(SurfaceReply::STORAGE_BYTES)?,
        )?;
        let Some(DesktopStartupCommand::SendSurface(reply)) = child.take_command()? else {
            return Err("Current unavailable surface request was silently dropped".into());
        };
        let value: serde_json::Value = serde_json::from_slice(reply.payload())?;
        assert_eq!(value["requestId"], 7);
        assert_eq!(value["status"], "unavailable");
        drop(reply);
        child.receive_native_request(
            identity,
            &context(identity)?,
            lease.clone(),
            lease.claim(CanvasNativeReply::STORAGE_BYTES)?,
        )?;
        let Some(DesktopStartupCommand::SendCanvasNative(reply)) = child.take_command()? else {
            return Err("Current unavailable context request was silently dropped".into());
        };
        let value: serde_json::Value = serde_json::from_slice(reply.payload())?;
        assert_eq!(value["requestId"], 9);
        assert_eq!(value["context"]["canvas"]["capabilities"]["displayOutput"], false);
        assert!(child.commands.iter().all(Option::is_none));
    }
    Ok(())
}

#[test]
fn consumed_control_and_cancel_return_receipts_without_ordinary_or_lifecycle_capacity() -> Result<()>
{
    let (mut child, _owner, _clock) = crate::production::child::test_support::child()?;
    let identity = child.identity;
    let bytes = PanelIngressReceipt::STORAGE_BYTES;
    let lease = MemoryLease::new(3 * bytes, Box::new(Release));
    child.receive_surface_message(
        identity,
        surface(identity, 3, false)?,
        lease.clone(),
        lease.claim(bytes)?,
    )?;
    child.receive_panel_control(
        identity,
        PanelMessage::Ready(PanelSession::for_main(identity)?),
        lease.clone(),
        lease.claim(bytes)?,
        false,
    )?;
    child.receive_surface_message(
        identity,
        surface(identity, 3, true)?,
        lease.clone(),
        lease.claim(bytes)?,
    )?;
    assert!(child.panel.is_none());
    assert!(child.commands.iter().all(Option::is_none));
    assert!(lease.claim(1).is_err());
    child.commands[0] = Some(DesktopStartupCommand::ConfirmMainAbsent(identity));
    assert!(
        matches!(child.take_command()?, Some(DesktopStartupCommand::ConfirmMainAbsent(id)) if id == identity)
    );
    assert!(matches!(child.take_command()?, Some(DesktopStartupCommand::SendSurface(_))));
    assert!(matches!(child.take_command()?, Some(DesktopStartupCommand::CompletePanelIngress(_))));
    assert!(matches!(child.take_command()?, Some(DesktopStartupCommand::CompletePanelIngress(_))));
    assert!(lease.claim(3 * bytes).is_ok());
    Ok(())
}

#[test]
fn foreign_and_retired_ingress_cannot_bind_the_current_child_delivery_owner() -> Result<()> {
    for retired in [false, true] {
        let (mut child, _owner, _clock) = crate::production::child::test_support::child()?;
        let current = child.identity;
        let observed = if retired {
            child.shutdown.main_retired = true;
            current
        } else {
            MainIdentity { navigation: Generation::new(2)?, ..current }
        };
        let bytes = SurfaceReply::STORAGE_BYTES;
        let lease = MemoryLease::new(bytes, Box::new(Release));
        child.receive_surface_message(
            observed,
            surface(observed, 4, false)?,
            lease.clone(),
            lease.claim(bytes)?,
        )?;
        child.receive_panel_control(
            observed,
            PanelMessage::Ready(PanelSession::for_main(observed)?),
            lease.clone(),
            lease.claim(bytes)?,
            false,
        )?;
        assert!(child.panel_deliveries.take_command(current).is_none());
        assert!(lease.claim(bytes).is_ok());
        assert!(child.panel.is_none());
    }
    Ok(())
}

#[test]
fn policy_processing_finishes_before_control_credit_is_returned_and_duplicate_admission_is_atomic()
-> Result<()> {
    let (mut child, _owner, _clock) = crate::production::child::test_support::child()?;
    let identity = child.identity;
    let bytes = PanelIngressReceipt::STORAGE_BYTES;
    let lease =
        MemoryLease::new(OWNER_BYTES + PanelCommand::STORAGE_BYTES + 2 * bytes, Box::new(Release));
    let session = PanelSession::for_main(identity)?;
    child.receive_panel_control(
        identity,
        PanelMessage::Ready(session.clone()),
        lease.clone(),
        lease.claim(bytes)?,
        true,
    )?;
    let panel = child.panel.as_ref().ok_or("Ready control did not construct its policy")?;
    assert!(panel.configuration.is_some());
    let state = panel.policy.state();
    assert!(
        child
            .receive_panel_control(
                identity,
                PanelMessage::Rejected {
                    session,
                    reason: sugarsubstitute_contracts::panel::PanelRejection::PresentationFailed,
                },
                lease.clone(),
                lease.claim(bytes)?,
                true,
            )
            .is_err()
    );
    assert_eq!(child.panel.as_ref().ok_or("Panel disappeared")?.policy.state(), state);
    assert!(matches!(child.take_command()?, Some(DesktopStartupCommand::SendPanel(_))));
    assert!(matches!(child.take_command()?, Some(DesktopStartupCommand::CompletePanelIngress(_))));
    assert!(lease.claim(2 * bytes).is_ok());
    Ok(())
}
