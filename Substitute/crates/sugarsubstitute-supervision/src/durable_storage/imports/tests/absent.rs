// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! An import that was never recorded reads as absent, not as a native storage failure.
use super::*;

#[test]
fn verify_completed_reports_not_found_when_only_the_originals_folder_exists() -> TestResult {
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context)?;
        // An earlier import step already created the originals folder but never the records one.
        user_paths::create_parents(
            &fixture.root,
            &fixture.root.path().join("user/migration-originals"),
            context,
        )?;
        let error = verify_completed(
            &fixture.root,
            ImportAssociation::Installed(&fixture.identity),
            &fixture.target,
            context,
        )
        .err()
        .ok_or("A never-recorded import verified as completed")?;
        assert_eq!(error.kind(), io::ErrorKind::NotFound, "{error:?}");
        Ok(())
    })
}
