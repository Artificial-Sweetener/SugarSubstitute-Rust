// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit pending browser loans and reject superseded delivery authority.
use super::*;
#[cfg(windows)]
use crate::workspace_session::NativeOriginalResolution;
#[test]
fn pending_readonly_proposal_accepts_fresh_runtime_zero_and_revokes_stale_delivery() -> TestResult {
    let (mut driver, mut fence) = fixture()?;
    let (request, transfer) = encoding(ready(&mut driver, fence.clone())?)?;
    driver.encoding_stopped(&request, &transfer)?;
    let checkpoint = &driver
        .restored
        .as_ref()
        .and_then(|value| value.checkpoint.as_ref())
        .ok_or("Missing saved checkpoint")?
        .1;
    let (entry, output) = checkpoint
        .entries
        .iter()
        .find_map(|entry| {
            entry
                .outputs
                .iter()
                .find(|output| output.asset_digest.is_some())
                .map(|output| (entry, output))
        })
        .ok_or("Missing retained output")?;
    #[cfg(windows)]
    let file = sugarsubstitute_contracts::canvas_surface::CanvasFileReference::decode(
        &serde_json::to_vec(
            &serde_json::json!({"filename":output.file.filename,"subfolder":output.file.subfolder,"type":output.file.storage}),
        )?,
    )?;
    let output_id = output.id.clone();
    let mut binding = fence
        .bindings
        .iter()
        .find(|binding| binding.entry_id == entry.id)
        .ok_or("Missing saved entry binding")?
        .clone();
    let asset = checkpoint
        .assets
        .iter()
        .find(|asset| Some(&asset.digest) == output.asset_digest.as_ref())
        .ok_or("Missing retained asset")?
        .clone();
    let checkpoint_id = checkpoint.id.clone();
    let revision = checkpoint.revision;
    assert!(!driver.asset_valid_for_delivery(&fence.webview, &output_id, &binding, &asset));
    let acknowledgement = driver.message(
        request.clone(),
        SessionOperation::Acknowledged {
            checkpoint_id: checkpoint_id.clone(),
            revision,
            phase: "materialized".into(),
            bindings: fence.bindings.clone(),
        },
    );
    assert!(driver.receive(acknowledgement, Moment::from_nanos(1)).is_err());
    driver.restore.as_mut().ok_or("Restore disappeared")?.sent = true;
    assert!(!driver.asset_valid_for_delivery(&fence.webview, &output_id, &binding, &asset));
    fence.revision += 1;
    for binding in &mut fence.bindings {
        binding.runtime_session = format!("workflow:{}", binding.entry_id);
        binding.generation = 0;
    }
    ready_state(&mut driver, fence.clone(), false)?;
    binding = fence
        .bindings
        .iter()
        .find(|fresh| fresh.entry_id == binding.entry_id)
        .ok_or("Fresh binding absent")?
        .clone();
    assert!(driver.asset_valid_for_delivery(&fence.webview, &output_id, &binding, &asset));
    assert!(!driver.asset_valid_for_delivery("foreign-webview", &output_id, &binding, &asset));
    assert!(!driver.asset_valid_for_delivery(
        &fence.webview,
        "00000000-0000-4000-8000-000000000098",
        &binding,
        &asset,
    ));
    let mut stale = binding.clone();
    stale.generation += 1;
    assert!(!driver.asset_valid_for_delivery(&fence.webview, &output_id, &stale, &asset));
    assert!(driver.asset(&output_id, &binding).is_none());
    #[cfg(windows)]
    {
        let address = canvas_address(&binding)?;
        assert!(driver.pending_native_original(&output_id, &address));
        assert_eq!(
            driver.resolve_native_original(&output_id, &address, &file),
            NativeOriginalResolution::Unavailable
        );
    }
    fence.revision += 1;
    ready(&mut driver, fence.clone())?;
    let acknowledgement = driver.message(
        request,
        SessionOperation::Acknowledged {
            checkpoint_id,
            revision,
            phase: "materialized".into(),
            bindings: fence.bindings.clone(),
        },
    );
    driver.receive(acknowledgement, Moment::from_nanos(2))?;
    assert!(driver.asset_valid_for_delivery(&fence.webview, &output_id, &binding, &asset));
    #[cfg(windows)]
    {
        let address = canvas_address(&binding)?;
        assert!(matches!(driver.resolve_native_original(&output_id, &address, &file),
            NativeOriginalResolution::Verified { digest, bytes, asset: Some(retained), .. }
                if digest == asset.digest && bytes == asset.bytes && retained == asset));
        let mut foreign = binding.clone();
        foreign.runtime_session = "workflow:foreign".into();
        assert_eq!(
            driver.resolve_native_original(&output_id, &canvas_address(&foreign)?, &file),
            NativeOriginalResolution::Unavailable
        );
    }
    fence.revision += 1;
    fence
        .bindings
        .iter_mut()
        .find(|current| current.entry_id == binding.entry_id)
        .ok_or("Selected binding absent")?
        .navigation_revision += 1;
    ready(&mut driver, fence.clone())?;
    assert!(driver.asset_valid_for_delivery(&fence.webview, &output_id, &binding, &asset));
    fence.revision += 1;
    fence
        .bindings
        .iter_mut()
        .find(|current| current.entry_id == binding.entry_id)
        .ok_or("Selected binding absent")?
        .generation += 1;
    ready(&mut driver, fence.clone())?;
    #[cfg(windows)]
    assert_eq!(
        driver.resolve_native_original(&output_id, &canvas_address(&binding)?, &file),
        NativeOriginalResolution::Unavailable
    );
    fence.revision += 1;
    fence.bindings.retain(|current| current.entry_id != binding.entry_id);
    ready(&mut driver, fence.clone())?;
    assert!(!driver.asset_valid_for_delivery(&fence.webview, &output_id, &binding, &asset));
    #[cfg(windows)]
    assert_eq!(
        driver.resolve_native_original(&output_id, &canvas_address(&binding)?, &file),
        NativeOriginalResolution::Unavailable
    );
    Ok(())
}
#[test]
fn cancelling_pending_restore_revokes_same_binding_media_before_delivery() -> TestResult {
    let (mut driver, mut fence) = fixture()?;
    let (request, transfer) = encoding(ready(&mut driver, fence.clone())?)?;
    driver.encoding_stopped(&request, &transfer)?;
    driver.restore.as_mut().ok_or("Restore disappeared")?.sent = true;
    fence.revision += 1;
    for binding in &mut fence.bindings {
        binding.generation += 1;
    }
    ready(&mut driver, fence.clone())?;
    let checkpoint = &driver
        .restored
        .as_ref()
        .and_then(|value| value.checkpoint.as_ref())
        .ok_or("Missing checkpoint")?
        .1;
    let asset = checkpoint.assets.first().ok_or("Missing asset")?.clone();
    let (entry, output) = checkpoint
        .entries
        .iter()
        .find_map(|entry| {
            entry
                .outputs
                .iter()
                .find(|output| output.asset_digest.as_deref() == Some(asset.digest.as_str()))
                .map(|output| (entry, output))
        })
        .ok_or("Missing retained output")?;
    let output = output.id.clone();
    let binding = fence
        .bindings
        .iter()
        .find(|binding| binding.entry_id == entry.id)
        .ok_or("Missing binding")?
        .clone();
    assert!(driver.asset_valid_for_delivery(&fence.webview, &output, &binding, &asset));
    let cancel = driver.message(
        "00000000-0000-4000-8000-000000000095".into(),
        SessionOperation::Cancel { target_request_id: request },
    );
    driver.receive(cancel, Moment::from_nanos(0))?;
    assert!(!driver.asset_valid_for_delivery(&fence.webview, &output, &binding, &asset));
    #[cfg(windows)]
    {
        let address = canvas_address(&binding)?;
        let file = sugarsubstitute_contracts::canvas_surface::CanvasFileReference::decode(
            br#"{"filename":"retained.png","subfolder":"","type":"temp"}"#,
        )?;
        assert_eq!(
            driver.resolve_native_original(&output, &address, &file),
            NativeOriginalResolution::Unavailable
        );
        assert!(!driver.pending_native_original(&output, &address));
        let unknown = "00000000-0000-4000-8000-000000000094";
        assert!(driver.requires_verified_original(&output));
        assert!(!driver.requires_verified_original(unknown));
        assert_eq!(
            driver.resolve_native_original(unknown, &address, &file),
            NativeOriginalResolution::Live
        );
        for index in 0..65_536u64 {
            driver.originals.remember_output(&format!("ffffffff-0000-4000-8000-{index:012x}"))?;
        }
        assert!(driver.requires_verified_original(unknown));
        assert_eq!(
            driver.resolve_native_original(unknown, &address, &file),
            NativeOriginalResolution::Unavailable
        );
        assert_eq!(
            driver.resolve_native_original(&output, &address, &file),
            NativeOriginalResolution::Unavailable
        );
    }
    Ok(())
}

#[cfg(windows)]
fn canvas_address(
    binding: &SessionBinding,
) -> Result<sugarsubstitute_contracts::canvas_surface::CanvasSurfaceAddress, Box<dyn Error>> {
    Ok(sugarsubstitute_contracts::canvas_surface::CanvasSurfaceAddress::decode(
        &serde_json::to_vec(
            &serde_json::json!({"sessionId":format!("canvas:{}",binding.runtime_session),"viewId":"view:test","windowId":"window:test","attachmentId":"attachment:test","generation":1}),
        )?,
    )?)
}
