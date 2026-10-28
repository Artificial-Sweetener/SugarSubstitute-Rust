// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Verify exact, expiring ownership assessments and structural limits.

use std::{collections::BTreeSet, path::Path};

use time::Date;

use crate::{
    error::{Failure, Result, read, require},
    metrics::code_lines,
    policy::{Architecture, Debt, Records, Waiver, WaiverKind},
    source::{fingerprint, resolve, rust_files},
    vendor::Manifest,
};

/// Reject expired review records using an explicit date for deterministic tests.
pub(crate) fn check_review(date: &str, today: Date) -> Result<()> {
    let format = time::format_description::parse("[year]-[month]-[day]")
        .map_err(|error| Failure::external("parse review date format", error))?;
    let expiry = Date::parse(date, &format)
        .map_err(|error| Failure::external(format!("parse review expiry {date}"), error))?;
    require(expiry >= today, format!("review expired on {date}; reassess its authoritative owner"))
}

/// Validate assessed debt and require exact dispositions for hard-limit files.
pub(crate) fn check(
    root: &Path,
    policy: &Architecture,
    debt: &Records<Debt>,
    waivers: &Records<Waiver>,
    today: Date,
    vendors: &Manifest,
) -> Result<()> {
    crate::vendor::check(root, vendors)?;
    require(
        debt.schema_version == 1 && waivers.schema_version == 1,
        "structural registries require schema one",
    )?;
    let files = policy
        .source_roots
        .iter()
        .map(|relative| rust_files(&resolve(root, relative)?))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<BTreeSet<_>>();
    let mut ids = BTreeSet::new();
    let mut debt_paths = BTreeSet::new();
    for record in &debt.records {
        require(
            !record.id.trim().is_empty()
                && ids.insert(&record.id)
                && debt_paths.insert(&record.path),
            "architecture debt identifiers and paths must be unique",
        )?;
        check_identity(root, policy, &files, &record.path, &record.owner, &record.source_sha256)?;
        check_review(&record.review_until, today)?;
        require(
            record.responsibilities.len() >= 2
                && record.responsibilities.iter().all(|item| !item.trim().is_empty())
                && !record.tracking_issue.trim().is_empty()
                && !record.next_extraction.trim().is_empty(),
            format!(
                "{}: name distinct responsibilities, tracking issue and next extraction",
                record.path
            ),
        )?;
    }
    let mut waived_paths = BTreeSet::new();
    for record in &waivers.records {
        require(
            waived_paths.insert(&record.path),
            format!("duplicate structural disposition: {}", record.path),
        )?;
        check_identity(root, policy, &files, &record.path, &record.owner, &record.source_sha256)?;
        check_review(&record.review_until, today)?;
        let lines = code_lines(&read(&resolve(root, &record.path)?)?)?;
        check_waiver(record, lines, policy.hard_lines, &debt.records)?;
    }
    for path in files {
        let relative = path
            .strip_prefix(root)
            .map_err(|error| Failure::external("relativize source", error))?
            .to_string_lossy()
            .replace('\\', "/");
        let source = read(&path)?;
        if let Some(license) = vendors.source_license(&relative) {
            require(
                source.lines().next()
                    == Some(format!("// SPDX-License-Identifier: {license}").as_str())
                    && source
                        .lines()
                        .nth(1)
                        .and_then(|line| line.strip_prefix("// Copyright (C) "))
                        .is_some_and(|copyright| !copyright.trim().is_empty()),
                format!(
                    "{relative}: preserve its manifested third-party license and copyright headers"
                ),
            )?;
        } else {
            require(
                source.lines().next() == Some("// SPDX-License-Identifier: GPL-3.0-or-later")
                    && source.lines().nth(1).is_some_and(|line| {
                        line.starts_with("// Copyright (C) ")
                            && line.ends_with("Artificial Sweetener and contributors")
                    }),
                format!("{relative}: add first-party SPDX and copyright headers"),
            )?;
        }
        let lines = code_lines(&source)?;
        if lines > policy.hard_lines {
            require(
                waived_paths.contains(&relative),
                format!(
                    "{relative}: {lines} code lines exceed {}; extract owners or register a reviewed disposition",
                    policy.hard_lines
                ),
            )?;
        } else if lines > policy.soft_lines {
            eprintln!(
                "ownership review: {relative} has {lines} code lines (soft ceiling {})",
                policy.soft_lines
            );
        }
    }
    Ok(())
}

/// Bind a review to an existing governed file and its single crate owner.
fn check_identity(
    root: &Path,
    policy: &Architecture,
    files: &BTreeSet<std::path::PathBuf>,
    path: &str,
    owner: &str,
    expected_hash: &str,
) -> Result<()> {
    let absolute = resolve(root, path)?;
    require(
        files.contains(&absolute),
        format!("{path}: unused or out-of-scope structural record"),
    )?;
    require(
        policy
            .owners
            .iter()
            .any(|entry| entry.package == owner && absolute.starts_with(root.join(&entry.root))),
        format!("{path}: incorrect assessed owner {owner}"),
    )?;
    require(
        fingerprint(root, &[path.to_owned()])? == expected_hash,
        format!("{path}: stale reviewed source fingerprint"),
    )
}

/// Require cohesive or debt-linked exceptions with exact shrinking bounds.
pub(crate) fn check_waiver(
    record: &Waiver,
    lines: usize,
    hard_limit: usize,
    debt: &[Debt],
) -> Result<()> {
    require(
        lines > hard_limit && record.line_limit == lines,
        format!(
            "{}: waiver must equal current hard-limit overage; remove unused records",
            record.path
        ),
    )?;
    require(
        !record.rationale.trim().is_empty(),
        format!("{}: waiver needs a source-level ownership rationale", record.path),
    )?;
    match record.kind {
        WaiverKind::Structural => require(
            record.debt_id.is_none()
                && record.next_limit.is_none()
                && !debt.iter().any(|item| item.path == record.path),
            format!("{}: mixed ownership cannot receive a structural waiver", record.path),
        ),
        WaiverKind::Remediation => require(
            record.next_limit.is_some_and(|next| next > 0 && next < lines)
                && debt.iter().any(|item| {
                    Some(&item.id) == record.debt_id.as_ref()
                        && item.path == record.path
                        && item.owner == record.owner
                }),
            format!(
                "{}: remediation needs matching debt and a lower positive next limit",
                record.path
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercise full-file enforcement for hard overages and stale assessed source.
    #[test]
    fn full_structure_gate_rejects_unassessed_growth_and_stale_reviews() -> Result<()> {
        let fixture = crate::fixtures::Repository::new()?;
        let date = Date::from_calendar_date(2026, time::Month::October, 10)
            .map_err(|error| Failure::external("construct date", error))?;
        let debt = Records { schema_version: 1, records: Vec::<Debt>::new() };
        let waivers = Records { schema_version: 1, records: Vec::<Waiver>::new() };
        let vendors = Manifest { schema_version: 1, components: vec![] };
        check(fixture.root(), &fixture.architecture, &debt, &waivers, date, &vendors)?;
        let path = "crates/host/src/lib.rs";
        let original = read(&fixture.root().join(path))?;
        let oversized = format!(
            "{original}{}",
            (0..501)
                .map(|number| format!("const V{number}: u32 = {number};\n"))
                .collect::<String>()
        );
        crate::fixtures::write(fixture.root(), path, &oversized)?;
        assert!(
            check(fixture.root(), &fixture.architecture, &debt, &waivers, date, &vendors).is_err()
        );
        let record = Debt {
            id: "mixed".into(),
            path: path.into(),
            owner: "host".into(),
            responsibilities: vec!["one".into(), "two".into()],
            tracking_issue: "issue".into(),
            next_extraction: "two".into(),
            source_sha256: "sha256:stale".into(),
            review_until: "2027-01-01".into(),
        };
        let debt = Records { schema_version: 1, records: vec![record] };
        assert!(
            check(fixture.root(), &fixture.architecture, &debt, &waivers, date, &vendors).is_err()
        );
        Ok(())
    }

    /// A copied Rust module retains its manifested license without escaping any source audit.
    #[test]
    fn vendor_headers_require_exact_manifest_and_license() -> Result<()> {
        let fixture = crate::fixtures::Repository::new()?;
        let date = Date::from_calendar_date(2026, time::Month::October, 10)
            .map_err(|error| Failure::external("construct date", error))?;
        let debt = Records { schema_version: 1, records: Vec::<Debt>::new() };
        let waivers = Records { schema_version: 1, records: Vec::<Waiver>::new() };
        let path = "crates/host/src/vendor.rs";
        crate::fixtures::write(
            fixture.root(),
            path,
            "// SPDX-License-Identifier: AGPL-3.0-or-later\n// Copyright (C) 2026 Example\npub struct Value;\n",
        )?;
        let empty = Manifest { schema_version: 1, components: vec![] };
        assert!(
            check(fixture.root(), &fixture.architecture, &debt, &waivers, date, &empty).is_err()
        );
        crate::fixtures::write(fixture.root(), "third_party/licenses/example.txt", "license")?;
        crate::fixtures::write(fixture.root(), "third_party/NOTICE.md", "notice")?;
        let unmarked: Manifest = toml::from_str(&format!(
            "schema_version=1\n[[components]]\nid='example'\nsource_url='https://example.test'\nlicense='AGPL-3.0-or-later'\nlicense_file='third_party/licenses/example.txt'\nfiles=['{path}']"
        )).map_err(|cause| Failure::external("parse vendor fixture", cause))?;
        assert!(
            check(fixture.root(), &fixture.architecture, &debt, &waivers, date, &unmarked).is_err()
        );
        let vendors: Manifest = toml::from_str(&format!(
            "schema_version=1\n[[components]]\nid='example'\nsource_url='https://example.test'\nlicense='AGPL-3.0-or-later'\nlicense_file='third_party/licenses/example.txt'\nlicense_header_files=['{path}']\nfiles=['{path}']"
        )).map_err(|cause| Failure::external("parse vendor fixture", cause))?;
        check(fixture.root(), &fixture.architecture, &debt, &waivers, date, &vendors)?;
        crate::fixtures::write(
            fixture.root(),
            path,
            "// SPDX-License-Identifier: MIT\n// Copyright (C) 2026 Example\npub struct Value;\n",
        )?;
        assert!(
            check(fixture.root(), &fixture.architecture, &debt, &waivers, date, &vendors).is_err()
        );
        Ok(())
    }

    /// Reject expired reviews but accept the exact expiry day.
    #[test]
    fn reviews_expire_after_the_review_day() -> Result<()> {
        let today = Date::from_calendar_date(2026, time::Month::October, 10)
            .map_err(|error| Failure::external("construct date", error))?;
        check_review("2026-10-10", today)?;
        assert!(check_review("2026-10-09", today).is_err());
        assert!(check_review("not-a-date", today).is_err());
        Ok(())
    }

    /// Reject unused waivers, changed exact limits and unbounded remediation.
    #[test]
    fn waivers_require_current_source_bounds_and_matching_debt() -> Result<()> {
        let mut record = Waiver {
            path: "file.rs".into(),
            owner: "host".into(),
            kind: WaiverKind::Structural,
            rationale: "one atomic contract".into(),
            source_sha256: "hash".into(),
            review_until: "2027-01-01".into(),
            line_limit: 501,
            debt_id: None,
            next_limit: None,
        };
        check_waiver(&record, 501, 500, &[])?;
        assert!(check_waiver(&record, 500, 500, &[]).is_err());
        assert!(check_waiver(&record, 502, 500, &[]).is_err());
        record.kind = WaiverKind::Remediation;
        record.next_limit = Some(501);
        assert!(check_waiver(&record, 501, 500, &[]).is_err());
        record.next_limit = Some(450);
        record.debt_id = Some("mixed".into());
        let debt = Debt {
            id: "mixed".into(),
            path: record.path.clone(),
            owner: record.owner.clone(),
            responsibilities: vec!["one".into(), "two".into()],
            tracking_issue: "issue".into(),
            next_extraction: "two".into(),
            source_sha256: "hash".into(),
            review_until: "2027-01-01".into(),
        };
        check_waiver(&record, 501, 500, &[debt])?;
        Ok(())
    }
}
