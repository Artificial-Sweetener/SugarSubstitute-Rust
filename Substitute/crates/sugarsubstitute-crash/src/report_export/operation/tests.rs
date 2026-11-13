// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Actual native destination and failure custody under finite central execution.
mod support;
use super::C;
use sha2::{Digest, Sha256};
use support::*;
#[test]
fn report_export_native_text_is_complete_and_receipt_follows_checked_cleanup() -> ResultT {
    let path = destination()?;
    let observed = export(&path, false, false, false, false)?;
    assert_eq!(observed.outcome, Ok(()));
    assert!(observed.retained.stage && observed.retained.activated);
    assert!(observed.retained.native_outputs > 0);
    let bytes = std::fs::read(&path)?;
    assert_eq!(bytes, observed.canonical);
    let receipt = observed.receipt.ok_or("checked receipt absent")?;
    assert_eq!(receipt.bytes, bytes.len() as u64);
    assert_eq!(receipt.sha256, <[u8; 32]>::from(Sha256::digest(&bytes)));
    Ok(())
}
#[test]
fn report_export_native_unicode_destination_uses_shared_receiving_owner() -> ResultT {
    let path = destination()?.with_file_name("糖😀.txt");
    let observed = export(&path, false, false, false, false)?;
    assert_eq!(observed.outcome, Ok(()));
    assert_eq!(std::fs::read(&path)?, observed.canonical);
    assert!(observed.receipt.is_some());
    assert_eq!(std::fs::read_dir(path.parent().ok_or("parent absent")?)?.count(), 1);
    Ok(())
}
#[test]
fn report_export_native_existing_file_requires_explicit_overwrite() -> ResultT {
    let path = destination()?;
    std::fs::write(&path, b"original selected bytes")?;
    let rejected = export(&path, false, false, false, false)?;
    assert_eq!(rejected.outcome, Err(C::Consent));
    assert!(rejected.receipt.is_none());
    assert_eq!(std::fs::read(&path)?, b"original selected bytes");
    let selected = export(&path, true, false, false, false)?;
    assert_eq!(selected.outcome, Ok(()));
    assert_eq!(std::fs::read(&path)?, selected.canonical);
    assert_eq!(std::fs::read_dir(path.parent().ok_or("parent absent")?)?.count(), 1);
    Ok(())
}
#[test]
fn report_export_native_stale_dump_consent_and_cancellation_do_not_create_output() -> ResultT {
    for (stale, dump, cancel) in [(true, false, false), (false, true, false), (false, false, true)]
    {
        let path = destination()?;
        let observed = export(&path, false, stale, dump, cancel)?;
        assert!(observed.outcome.is_err());
        assert!(observed.receipt.is_none());
        assert!(!path.exists());
        assert_eq!(std::fs::read_dir(path.parent().ok_or("parent absent")?)?.count(), 0);
    }
    Ok(())
}
#[test]
fn report_export_native_hardlinked_destination_is_preserved() -> ResultT {
    let path = destination()?;
    let peer = path.with_file_name("peer.txt");
    std::fs::write(&path, b"original linked bytes")?;
    std::fs::hard_link(&path, &peer)?;
    let observed = export(&path, true, false, false, false)?;
    assert_eq!(observed.outcome, Err(C::Destination));
    assert!(observed.receipt.is_none());
    assert_eq!(std::fs::read(&peer)?, b"original linked bytes");
    Ok(())
}
#[test]
fn report_export_native_busy_destination_retains_failure_and_preserves_bytes() -> ResultT {
    use std::os::windows::fs::OpenOptionsExt;
    let path = destination()?;
    std::fs::write(&path, b"busy original bytes")?;
    let exclusive = std::fs::OpenOptions::new().read(true).share_mode(0).open(&path)?;
    let observed = export(&path, true, false, false, false)?;
    assert_eq!(observed.outcome, Err(C::Native));
    assert!(observed.receipt.is_none());
    drop(exclusive);
    assert_eq!(std::fs::read(&path)?, b"busy original bytes");
    Ok(())
}
#[test]
fn report_export_native_activation_failure_closes_stage_and_preserves_both_predecessors() -> ResultT
{
    let path = destination()?;
    let backup = path.with_file_name("report.txt.sugar-0000000000000001-0000000000000001-prior");
    std::fs::write(&path, b"selected original bytes")?;
    std::fs::write(&backup, b"unowned collision bytes")?;
    let observed = export(&path, true, false, false, false)?;
    assert_eq!(observed.outcome, Err(C::Native));
    assert!(observed.retained.stage && observed.retained.predecessor);
    assert!(!observed.retained.activated);
    assert!(observed.retained.native_outputs >= 3);
    assert!(observed.receipt.is_none());
    assert_eq!(std::fs::read(&path)?, b"selected original bytes");
    assert_eq!(std::fs::read(&backup)?, b"unowned collision bytes");
    assert_eq!(std::fs::read_dir(path.parent().ok_or("parent absent")?)?.count(), 2);
    Ok(())
}

#[test]
fn report_export_native_changed_pinned_predecessor_is_rejected_before_activation() -> ResultT {
    let path = destination()?;
    std::fs::write(&path, b"original selected bytes")?;
    predecessor_mutation(&path)?;
    assert_eq!(std::fs::read(&path)?, b"original selected bytes");
    assert!(std::fs::metadata(&path)?.permissions().readonly());
    assert_eq!(std::fs::read_dir(path.parent().ok_or("parent absent")?)?.count(), 1);
    Ok(())
}
