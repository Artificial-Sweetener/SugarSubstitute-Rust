// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Role selection uses authenticated bytes before native process creation.
use super::*;

#[test]
fn historical_application_role_remains_launchable_without_inventing_an_updater() -> TestResult {
    let mut h = Harness::new()?;
    let (fixture, input, executable) = preparation_with(&mut h, None, None)?;
    assert_eq!(executable.file_name(), Some(std::ffi::OsStr::new("sugarsubstitute.exe")));
    let ticket = submit(&mut h, FamilyBinding::Preparation(owner()?), input)?;
    h.created(ticket.process())?;
    assert!(!writable(&executable));
    h.fake.root_alive.store(false, Ordering::Release);
    h.exit(ticket.process(), ROOT, true, 0)?;
    h.stop(ticket.process())?;
    h.cleanup(ticket.process())?;
    assert!(h.registry.snapshot(ticket.process())?.settled);
    assert!(writable(&executable));
    fixture.preserved().map_err(|cause| cause as Box<dyn Error>)?;
    Ok(())
}

#[test]
fn incompatible_or_malformed_updater_never_falls_back_to_the_application() -> TestResult {
    for declaration in [
        br#"{"schema_version":1,"preparation_protocols":[3],"installation_schemas":[3]}"#.as_slice(),
        br#"{"schema_version":1,"preparation_protocols":[2],"installation_schemas":[4]}"#,
        br#"{"schema_version":1,"preparation_protocols":[2],"installation_schemas":[3],"command":"foreign.exe"}"#,
    ] {
        let mut h = Harness::new()?;
        assert!(preparation_with(&mut h, Some(declaration), None).is_err());
        assert_eq!(h.fake.creates.load(Ordering::Acquire), 0);
        assert!(h.registry.is_empty());
    }
    Ok(())
}
