// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject missing, stale and misclassified upstream networking registrations.
use super::*;
use crate::fixtures::{self, Repository};
fn fixture() -> Result<(Repository, Boundary)> {
    let repository = Repository::new()?;
    for path in SOURCES {
        fixtures::write(repository.root(), path, "import {net} from 'electron'; net.request(url)")?;
    }
    fixtures::write(
        repository.root(),
        "third_party/manifest.toml",
        &format!(
            "schema_version=1\n[[components]]\nid='{COMPONENT}'\nfiles=['{}','{}']",
            SOURCES[0], SOURCES[1]
        ),
    )?;
    let sources: Vec<String> = SOURCES.iter().map(|source| (*source).into()).collect();
    let record = Boundary {
        source_sha256: fingerprint(repository.root(), &sources)?,
        sources,
        component: COMPONENT.into(),
        rationale: "Consumed upstream HTTP implementation".into(),
    };
    Ok((repository, record))
}
#[test]
fn missing_foreign_stale_or_unregistered_upstream_boundaries_reject() -> Result<()> {
    for variant in 0..8 {
        let (repository, mut record) = fixture()?;
        check(repository.root(), std::slice::from_ref(&record))?;
        match variant {
            0 => {
                assert!(check(repository.root(), &[]).is_err());
                continue;
            }
            1 => record.component = "foreign".into(),
            2 => record.rationale.clear(),
            3 => record.source_sha256 = "stale".into(),
            4 => {
                record.sources.pop();
            }
            5 => record.sources[1] = record.sources[0].clone(),
            6 => fixtures::write(
                repository.root(),
                "third_party/manifest.toml",
                "schema_version=1\n[[components]]\nid='comfy-desktop-provisioning'\nfiles=[]",
            )?,
            _ => fixtures::write(repository.root(), SOURCES[0], "export const inert = true")?,
        }
        assert!(check(repository.root(), &[record]).is_err());
    }
    Ok(())
}

#[test]
fn removing_component_registration_cannot_hide_existing_upstream_network_implementations()
-> Result<()> {
    let (repository, _) = fixture()?;
    fixtures::write(
        repository.root(),
        "third_party/manifest.toml",
        "schema_version=1\ncomponents=[]",
    )?;
    assert!(check(repository.root(), &[]).is_err());
    Ok(())
}
