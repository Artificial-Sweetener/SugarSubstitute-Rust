// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fault the actual clipboard boundary while exercising the production custody owner.
use super::*;
use std::{
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use sugarsubstitute_contracts::execution::MemoryRelease;
struct Release(Arc<AtomicU64>);
impl MemoryRelease for Release {
    fn release(&self, bytes: u64) {
        self.0.fetch_add(bytes, Ordering::SeqCst);
    }
}
#[derive(Default)]
struct Fake {
    fail: Option<&'static str>,
    calls: Vec<&'static str>,
    text: Vec<u16>,
    transferred: bool,
}
impl Fake {
    fn call(&mut self, name: &'static str) -> Result<(), ReportSurfaceError> {
        self.calls.push(name);
        if self.fail == Some(name) {
            Err(ReportSurfaceError::new(name, std::io::Error::other(name)))
        } else {
            Ok(())
        }
    }
}
impl Boundary for Fake {
    fn allocate(&mut self, _: usize) -> Result<HGLOBAL, ReportSurfaceError> {
        self.call("allocate")?;
        Ok(HGLOBAL(std::ptr::dangling_mut::<u8>().cast()))
    }
    fn write(&mut self, _: HGLOBAL, text: &[u16]) -> Result<(), ReportSurfaceError> {
        self.call("write")?;
        self.text.extend_from_slice(text);
        Ok(())
    }
    fn unlock(&mut self, _: HGLOBAL) -> Result<(), ReportSurfaceError> {
        self.call("unlock")
    }
    fn open(&mut self) -> Result<(), ReportSurfaceError> {
        self.call("open")
    }
    fn empty(&mut self) -> Result<(), ReportSurfaceError> {
        self.call("empty")
    }
    fn transfer(&mut self, global: HGLOBAL) -> Result<HGLOBAL, ReportSurfaceError> {
        self.call("transfer")?;
        self.transferred = true;
        Ok(global)
    }
    fn close(&mut self) -> Result<(), ReportSurfaceError> {
        self.call("close")
    }
    fn free(&mut self, _: HGLOBAL) -> Result<(), ReportSurfaceError> {
        self.call("free")
    }
}
fn owner(fail: Option<&'static str>) -> (Clipboard<Fake>, MemoryLease) {
    let boundary = Fake { fail, ..Fake::default() };
    let owner = Clipboard { boundary, global: None, open: false, locked: false, claim: None };
    (owner, MemoryLease::new(8, Box::new(Release(Arc::new(AtomicU64::new(0))))))
}
#[test]
fn clipboard_success_transfers_exact_unicode_and_never_frees_os_memory()
-> Result<(), Box<dyn Error>> {
    let (mut owner, lease) = owner(None);
    owner.copy(&[0x4e2d, 0xd83d, 0xde00, 0], &lease)?;
    assert_eq!(owner.boundary.text, [0x4e2d, 0xd83d, 0xde00, 0]);
    assert_eq!(
        owner.boundary.calls,
        ["allocate", "write", "unlock", "open", "empty", "transfer", "close"]
    );
    owner.retire()?;
    assert!(!owner.boundary.calls.contains(&"free"));
    let claim = lease.claim(8)?;
    drop(claim);
    Ok(())
}
#[test]
fn every_pretransfer_failure_retains_exact_block_and_cleanup_obligations()
-> Result<(), Box<dyn Error>> {
    for failure in ["write", "unlock", "open", "empty", "transfer"] {
        let (mut owner, lease) = owner(Some(failure));
        let error = owner.copy(&[65, 66, 67, 0], &lease).err().ok_or("fault not observed")?;
        assert_eq!(error.operation(), failure);
        assert!(error.source().is_some());
        assert!(owner.global.is_some());
        assert!(lease.claim(1).is_err());
        assert!(owner.copy(&[65, 0], &lease).is_err());
        owner.boundary.fail = None;
        owner.retire()?;
        assert!(owner.global.is_none());
        assert!(owner.claim.is_none());
        assert_eq!(owner.boundary.calls.last(), Some(&"free"));
        let claim = lease.claim(8)?;
        drop(claim);
    }
    Ok(())
}
#[test]
fn posttransfer_close_failure_keeps_open_custody_but_os_owns_memory() -> Result<(), Box<dyn Error>>
{
    let (mut owner, lease) = owner(Some("close"));
    assert!(owner.copy(&[65, 66, 67, 0], &lease).is_err());
    assert!(owner.open);
    assert!(owner.global.is_none());
    assert!(owner.claim.is_none());
    assert!(owner.boundary.transferred);
    assert!(owner.copy(&[65, 0], &lease).is_err());
    owner.boundary.fail = None;
    owner.retire()?;
    assert!(!owner.open);
    assert!(!owner.boundary.calls.contains(&"free"));
    Ok(())
}
#[test]
fn native_allocation_is_not_attempted_before_credit_and_framing_preflight()
-> Result<(), Box<dyn Error>> {
    let (mut owner, lease) = owner(None);
    for text in [&[65, 0, 66, 0][..], &[65, 66][..], &[65, 66, 67, 68, 0][..], &[][..]] {
        assert!(owner.copy(text, &lease).is_err());
        assert!(owner.boundary.calls.is_empty());
    }
    owner.boundary.fail = Some("allocate");
    assert!(owner.copy(&[65, 66, 67, 0], &lease).is_err());
    assert!(owner.global.is_none());
    let claim = lease.claim(8)?;
    drop(claim);
    Ok(())
}
#[test]
fn failed_native_free_retains_credit_without_reopening_clipboard() -> Result<(), Box<dyn Error>> {
    let (mut owner, lease) = owner(Some("open"));
    assert!(owner.copy(&[65, 66, 67, 0], &lease).is_err());
    owner.boundary.fail = Some("free");
    assert!(owner.retire().is_err());
    assert!(owner.global.is_some());
    assert!(lease.claim(1).is_err());
    owner.boundary.fail = None;
    owner.retire()?;
    assert!(owner.global.is_none());
    Ok(())
}
