// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify exact loose-file ownership beside separately governed source directories.
use crate::{
    architecture,
    error::Result,
    fixtures::{Repository, write},
};

/// Explicit loose files have an owner without transferring a sibling runtime subtree.
#[test]
fn loose_files_have_exact_nonoverlapping_authored_ownership() -> Result<()> {
    let mut repository = Repository::new()?;
    write(repository.root(), "installer/build.mjs", "export const ready=true")?;
    repository.architecture.owners[0].auxiliary_files.push("installer/build.mjs".into());
    architecture::check(repository.root(), &repository.architecture, &repository.workspace)?;
    let owner = &repository.architecture.owners[0];
    assert!(owner.owns(repository.root(), &repository.root().join("installer/build.mjs")));
    assert!(!owner.owns(repository.root(), &repository.root().join("installer/worker/main.ts")));
    Ok(())
}

/// File registration cannot conceal Rust, reuse an existing owner, name directories or escape scope.
#[test]
fn invalid_auxiliary_file_dispositions_are_rejected() -> Result<()> {
    for path in [
        "crates/host/src/lib.rs",
        "installer",
        "installer/missing.mjs",
        "../outside.mjs",
        "installer/hidden.rs",
    ] {
        let mut repository = Repository::new()?;
        write(repository.root(), "installer/hidden.rs", "fn hidden(){}")?;
        repository.architecture.owners[0].auxiliary_files.push(path.into());
        assert!(
            architecture::check(repository.root(), &repository.architecture, &repository.workspace)
                .is_err(),
            "accepted {path}"
        );
    }
    let mut repository = Repository::new()?;
    write(repository.root(), "installer/build.mjs", "export const ready=true")?;
    repository.architecture.owners[0].auxiliary_files =
        vec!["installer/build.mjs".into(), "installer/build.mjs".into()];
    assert!(
        architecture::check(repository.root(), &repository.architecture, &repository.workspace)
            .is_err()
    );
    Ok(())
}
