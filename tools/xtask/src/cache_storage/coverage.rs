// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Require complete authored-root discovery and exact reviews for checker input namespace data.
use super::*;
use crate::source::fingerprint;

/// Tooling may contain literal rejection-fixture data only under an exact owned source review.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Review {
    pub(crate) path: String,
    pub(crate) owner: String,
    pub(crate) source_sha256: String,
    pub(crate) rationale: String,
}

/// Validate exact tooling identity and current source before allowing namespace test/checker data.
pub(super) fn reviews<'a>(
    root: &Path,
    architecture: &Architecture,
    records: &'a [Review],
) -> Result<BTreeSet<&'a str>> {
    let mut reviewed = BTreeSet::new();
    for record in records {
        require(
            !record.rationale.trim().is_empty() && reviewed.insert(record.path.as_str()),
            "cache: duplicate or unexplained namespace data review",
        )?;
        let path = resolve(root, &record.path)?;
        let owners: Vec<_> =
            architecture.owners.iter().filter(|owner| owner.owns(root, &path)).collect();
        require(
            owners.len() == 1 && owners[0].package == record.owner && owners[0].layer == "tooling",
            "cache: namespace data review requires its exact tooling owner",
        )?;
        require(
            fingerprint(root, std::slice::from_ref(&record.path))? == record.source_sha256,
            format!("cache: stale namespace data review {}", record.path),
        )?;
    }
    Ok(reviewed)
}
