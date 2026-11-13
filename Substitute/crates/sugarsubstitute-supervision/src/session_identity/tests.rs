// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve exact source correlation records across reopen, invalid bytes and directory replacement.
use super::*;
use crate::setup_storage::test_worker::{TestResult, run};
#[test]
fn source_identity_reopens_stably_without_adopting_installation_selection() -> TestResult {
    run(|context, _| {
        let root = tempfile::tempdir()?;
        let user = root.path().join("user");
        let profile = ProfileId::new("real".into())?;
        let first = session_host_identity(&user, &profile, None, context)?;
        let path = user.join("sessions/real").join(RECORD);
        let bytes = std::fs::read(&path)?;
        assert_eq!(session_host_identity(&user, &profile, None, context)?, first);
        assert_eq!(std::fs::read(path)?, bytes);
        assert!(!root.path().join("state").exists());
        Ok(())
    })
}
#[test]
fn invalid_and_relocated_source_identity_originals_cannot_allocate_a_replacement() -> TestResult {
    run(|context, _| {
        let root = tempfile::tempdir()?;
        let user = root.path().join("user");
        let profile = ProfileId::new("real".into())?;
        session_host_identity(&user, &profile, None, context)?;
        let directory = user.join("sessions/real");
        let path = directory.join(RECORD);
        let original = std::fs::read(&path)?;
        for bytes in [b"not-json".as_slice(),br#"{"version":2,"installation":"11111111111111111111111111111111","profile":"real","directory":[1,2,3]}"#] {
            std::fs::write(&path,bytes)?;
            assert!(session_host_identity(&user,&profile,None,context).is_err());
            assert_eq!(std::fs::read(&path)?,bytes);
        }
        std::fs::write(&path, &original)?;
        std::fs::rename(&directory, user.join("sessions/preserved-real"))?;
        // The canonical durable owner creates/protects the replacement directory first.
        let replacement = session_host_identity(&user, &profile, None, context)?;
        assert_eq!(replacement.as_str().len(), 32);
        std::fs::write(&path, &original)?;
        assert!(session_host_identity(&user, &profile, None, context).is_err());
        assert_eq!(std::fs::read(&path)?, original);
        assert_eq!(std::fs::read(user.join("sessions/preserved-real").join(RECORD))?, original);
        Ok(())
    })
}
