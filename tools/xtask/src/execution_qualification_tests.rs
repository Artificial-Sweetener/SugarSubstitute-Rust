// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject registered creator fabrication even inside a configured execution owner.
use super::*;

#[test]
fn qualification_constructor_cannot_hide_in_aliases_macros_or_tests() -> Result<()> {
    let fixture = Repository::new()?;
    let policy = Policy { schema_version: 1, owner: None };
    for source in [
        "fn forge() { AuthenticatedDesktopCreator::from_authenticated(binding); }",
        "use owner::AuthenticatedDesktopCreator as Seal; fn forge() { Seal::from_authenticated(binding); }",
        "type Seal = owner::AuthenticatedDesktopCreator; fn forge() { Seal::from_authenticated(binding); }",
        "macro_rules! forge { () => { AuthenticatedDesktopCreator::from_authenticated(binding) }; }",
        "macro_rules! forge { ($t:ident,$m:ident) => { $t::$m(binding) }; } fn x() { forge!(AuthenticatedDesktopCreator,from_authenticated); }",
        "#[cfg(test)] mod tests { fn forge() { AuthenticatedDesktopCreator::from_authenticated(binding); } }",
    ] {
        assert!(!crate::execution_source::qualification_producer_findings(source)?.is_empty());
        write(fixture.root(), "crates/host/src/fault.rs", source)?;
        assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    }
    let consumer = "fn qualify(client: ExecutionClient, capture: &DesktopCaptureSession) { client.submit_registered_desktop_fault(scope, spec, capture, run); }";
    assert!(crate::execution_source::qualification_producer_findings(consumer)?.is_empty());
    assert!(crate::execution_source::resource_producer_findings(consumer)?.is_empty());
    assert!(crate::execution_source::findings(consumer)?.is_empty());
    Ok(())
}

#[test]
fn even_registered_execution_owner_cannot_move_creator_issuance_to_another_file() -> Result<()> {
    let mut fixture = Repository::new()?;
    fixture.architecture.owners[0].layer = "adapter".into();
    fixture.architecture.owners[0].concern = "Application execution and task lifetime".into();
    let policy = Policy { schema_version: 1, owner: Some("host".into()) };
    write(
        fixture.root(),
        "crates/host/src/alternate.rs",
        "fn forge() { AuthenticatedDesktopCreator::from_authenticated(binding); }",
    )?;
    let failure = check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)
        .err()
        .map(|cause| cause.to_string())
        .unwrap_or_default();
    assert!(failure.contains("outside the authenticated native child producer"), "{failure}");
    Ok(())
}

#[test]
fn preview_fixture_producers_reject_alias_macro_and_test_escapes() -> Result<()> {
    let fixture = Repository::new()?;
    let policy = Policy { schema_version: 1, owner: None };
    for source in [
        "fn forge() { AuthenticatedDesktopCreator::owner_fixture(core,scope,run); }",
        "use owner::AuthenticatedDesktopCreator as Seal; fn forge() { Seal::owner_fixture(core,scope,run); }",
        "macro_rules! forge { () => { AuthenticatedDesktopCreator::owner_fixture(core,scope,run) }; }",
        "#[cfg(test)] mod tests { fn forge() { client.borrow_preview_owner_fixture(session,scope,run,counter); } }",
        "macro_rules! forge { ($m:ident) => { client.$m(session,scope,run,counter) }; } fn x() { forge!(borrow_preview_owner_fixture); }",
    ] {
        assert!(!crate::execution_source::qualification_fixture_findings(source)?.is_empty());
        write(fixture.root(), "crates/host/src/forge.rs", source)?;
        assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    }
    assert!(crate::execution_source::qualification_fixture_findings("fn persist() { client.borrow_preview_source(session,scope,run,counter); client.bind_preview_source(context,loan,destination,image); seal.check(context,image); }")?.is_empty());
    Ok(())
}

#[test]
fn preview_exact_workspace_source_access_and_public_exports_remain_enforced() -> Result<()> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .ok_or_else(|| crate::error::Failure::Policy("Missing workspace parent".into()))?;
    let policy: Policy = crate::policy::load(root, "governance/execution/policy.toml")?;
    let architecture: crate::policy::Architecture =
        crate::policy::load(root, "governance/architecture/policy.toml")?;
    let workspace = crate::workspace::Workspace::load(root)?;
    check(root, &policy, &architecture, &workspace)
}
