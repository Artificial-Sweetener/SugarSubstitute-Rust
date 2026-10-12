// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Actual modal custody requires independent callback and timer retirement observations.
use super::*;
use crate::report::windows::{buffer::Text, save_dialog::tests::state};
use std::{
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use sugarsubstitute_contracts::execution::{MemoryLease, MemoryRelease};
use windows::Win32::UI::Controls::Dialogs::OPENFILENAMEW;
struct Release(Arc<AtomicU64>);
impl MemoryRelease for Release {
    fn release(&self, bytes: u64) {
        self.0.fetch_add(bytes, Ordering::SeqCst);
    }
}
#[test]
fn report_uncertain_picker_holds_actual_storage_until_destroy_and_checked_timer_retirement()
-> Result<(), Box<dyn Error>> {
    let released = Arc::new(AtomicU64::new(0));
    let lease = MemoryLease::new(4096, Box::new(Release(released.clone())));
    let mut hook = state()?;
    hook.initialized = true;
    hook.installed = true;
    hook.poller = None;
    hook.timer = 1;
    let dialog = Dialog {
        hook: Box::new(hook),
        options: Box::new(OPENFILENAMEW::default()),
        _title: Text::new("Save", &lease)?,
        _filter: Vec::new(),
        destination: Vec::new(),
        _filter_claim: lease.claim(16)?,
        claim: lease.claim(512)?,
    };
    let mut owner = UnretiredDialog::new(dialog);
    drop(lease);
    assert!(!owner.observe_retirement());
    assert_eq!(released.load(Ordering::SeqCst), 0);
    owner.dialog.as_mut().ok_or("custody lost")?.hook.destroyed = true;
    assert!(!owner.observe_retirement());
    owner.dialog.as_mut().ok_or("custody lost")?.hook.timer = 0;
    owner.dialog.as_mut().ok_or("custody lost")?.hook.active = true;
    assert!(!owner.observe_retirement());
    owner.dialog.as_mut().ok_or("custody lost")?.hook.active = false;
    assert!(owner.observe_retirement());
    assert!(owner.dialog.is_none());
    assert_eq!(released.load(Ordering::SeqCst), 4096);
    assert!(owner.observe_retirement());
    Ok(())
}
