// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve actual callback custody and partial native outputs through normal-loop observation.
use super::*;
use std::error::Error;
use sugarsubstitute_contracts::execution::MemoryRelease;
use windows::Win32::Foundation::{E_FAIL, S_OK};
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}
fn registration() -> Result<Registration> {
    let memory = MemoryLease::new(24 * 1024 * 1024, Box::new(Release));
    Ok(Registration::new(Events::new(&memory, || {})?, &memory)?)
}
fn callback(
    registration: &Registration,
) -> ICoreWebView2AddScriptToExecuteOnDocumentCreatedCompletedHandler {
    registration.state.state.borrow_mut().pending = true;
    Completed { shared: registration.state.clone() }.into()
}
#[test]
fn actual_script_callback_stays_owned_until_observed_and_released() -> Result {
    let mut registration = registration()?;
    let callback = callback(&registration);
    assert!(registration.pending());
    assert!(!registration.completed());
    // SAFETY: The authored COM callback receives this live NUL-terminated identifier on its creating thread.
    unsafe { callback.Invoke(S_OK, &HSTRING::from("native-script-7")) }?;
    assert!(!registration.pending());
    assert!(registration.completed());
    assert!(registration.observe()?);
    assert_eq!(registration.script.as_deref(), Some("native-script-7"));
    assert_eq!(Rc::strong_count(&registration.state), 2);
    drop(callback);
    assert_eq!(Rc::strong_count(&registration.state), 1);
    Ok(())
}
#[test]
fn failed_hresult_preserves_its_actual_identifier_for_checked_removal() -> Result {
    let mut registration = registration()?;
    let callback = callback(&registration);
    // SAFETY: Simulate a native partial output with the real callback interface and bounded live string.
    unsafe { callback.Invoke(E_FAIL, &HSTRING::from("partial-native-script")) }?;
    assert!(registration.observe().is_err());
    assert_eq!(registration.script.as_deref(), Some("partial-native-script"));
    assert!(!registration.uncertain);
    assert!(!registration.pending());
    Ok(())
}
#[test]
fn duplicate_callback_cannot_replace_the_original_native_registration() -> Result {
    let mut registration = registration()?;
    let callback = callback(&registration);
    // SAFETY: Both native-style calls use this exact live creating-thread callback and bounded identifiers.
    unsafe { callback.Invoke(S_OK, &HSTRING::from("original-script")) }?;
    // SAFETY: Exercise an unexpected repeated callback while retaining the actual original output.
    unsafe { callback.Invoke(S_OK, &HSTRING::from("replacement-script")) }?;
    assert!(registration.observe().is_err());
    assert_eq!(registration.script.as_deref(), Some("original-script"));
    Ok(())
}
