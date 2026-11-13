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

#[test]
fn report_copy_fixture_rejects_production_alias_macro_and_counterfeit_test_access() -> Result<()> {
    let fixture = Repository::new()?;
    let policy = Policy { schema_version: 1, owner: None };
    for source in [
        "fn issue() { sugarsubstitute_desktop::report::copy_fixture::with_pending_report_copy(a,b,callback); }",
        "use sugarsubstitute_desktop::report::copy_fixture::with_pending_report_copy as issue; fn main(){ issue(a,b,callback); }",
        "use sugarsubstitute_desktop::report as r; use r::copy_fixture as f; fn main(){ f::with_pending_report_copy(a,b,callback); }",
        "use sugarsubstitute_desktop::report::copy_fixture::*; fn main(){ with_pending_report_copy(a,b,callback); }",
        "fn main(){ let issue = sugarsubstitute_desktop::report::copy_fixture::with_pending_report_copy; issue(a,b,callback); }",
        "macro_rules! issue { () => { sugarsubstitute_desktop::report::copy_fixture::with_pending_report_copy(a,b,callback) }; }",
        "macro_rules! issue { ($m:ident) => {$m(a,b,callback)}; } fn main(){issue!(with_pending_report_copy);}",
        "macro_rules! issue { () => {use sugarsubstitute_desktop::report::copy_fixture::with_pending_report_copy as f; f(a,b,callback);}; }",
        "#[cfg(test)] mod counterfeit { fn issue(){ with_pending_report_copy(a,b,callback); } }",
        "fn issue(){ r#with_pending_report_copy(a,b,callback); }",
    ] {
        assert!(!crate::execution_source::report_copy_fixture_findings(source)?.is_empty());
        write(fixture.root(), "crates/host/src/copy.rs", source)?;
        assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    }
    assert!(crate::execution_source::report_copy_fixture_findings("// with_pending_report_copy\nconst HELP: &str = \"report::copy_fixture::with_pending_report_copy\";")?.is_empty());
    Ok(())
}
fn copy_consumer_fixture() -> Result<Repository> {
    let mut fixture = Repository::new()?;
    fixture.architecture.owners[0].package = "sugarsubstitute".into();
    fixture.architecture.owners[0].root = "crates/sugarsubstitute".into();
    fixture.workspace.packages[0].name = "sugarsubstitute".into();
    write(
        fixture.root(),
        "crates/sugarsubstitute/Cargo.toml",
        "[target.'cfg(windows)'.dev-dependencies]\nsugarsubstitute-desktop = { features = ['boundary-fixtures'] }\n",
    )?;
    write(
        fixture.root(),
        "crates/sugarsubstitute/src/production/reporter_role/runtime/control.rs",
        "#[cfg(test)] #[path=\"control/tests.rs\"] mod tests;",
    )?;
    write(
        fixture.root(),
        "crates/sugarsubstitute/src/production/reporter_role/runtime/control/tests.rs",
        "fn issue(){with_pending_report_copy(a,b,callback);}",
    )?;
    Ok(fixture)
}
#[test]
fn exact_report_copy_consumer_requires_actual_test_module_and_dev_only_edge() -> Result<()> {
    let fixture = copy_consumer_fixture()?;
    let policy = Policy { schema_version: 1, owner: None };
    check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)?;
    for declaration in [
        "#[path=\"control/tests.rs\"] mod tests;",
        "#[cfg(any(test,feature=\"shipping\"))] #[path=\"control/tests.rs\"] mod tests;",
        "#[cfg(test)] #[path=\"other.rs\"] mod tests;",
        "#[cfg(test)] mod tests {}",
        "#[cfg(test)] #[path=\"control/tests.rs\"] mod tests; #[path=\"control/tests.rs\"] mod shipping;",
    ] {
        write(
            fixture.root(),
            "crates/sugarsubstitute/src/production/reporter_role/runtime/control.rs",
            declaration,
        )?;
        assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    }
    write(
        fixture.root(),
        "crates/sugarsubstitute/src/production/reporter_role/runtime/control.rs",
        "#[cfg(test)] #[path=\"control/tests.rs\"] mod tests;",
    )?;
    for manifest in [
        "[dependencies]\nsugarsubstitute-desktop={features=['boundary-fixtures']}\n",
        "[dependencies]\nsugarsubstitute-desktop={features=['boundary-fixtures']}\n[target.'cfg(windows)'.dev-dependencies]\nsugarsubstitute-desktop={features=['boundary-fixtures']}\n",
    ] {
        write(fixture.root(), "crates/sugarsubstitute/Cargo.toml", manifest)?;
        assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    }
    Ok(())
}
#[test]
fn report_copy_consumer_permission_cannot_borrow_private_preview_producer() -> Result<()> {
    let fixture = copy_consumer_fixture()?;
    write(
        fixture.root(),
        "crates/sugarsubstitute/src/production/reporter_role/runtime/control/tests.rs",
        "fn forge(){AuthenticatedDesktopCreator::owner_fixture(core,scope,run);}",
    )?;
    assert!(
        check(
            fixture.root(),
            &Policy { schema_version: 1, owner: None },
            &fixture.architecture,
            &fixture.workspace
        )
        .is_err()
    );
    Ok(())
}
#[test]
fn exact_report_copy_producer_requires_default_off_feature_and_exact_feature_module() -> Result<()>
{
    let mut fixture = Repository::new()?;
    fixture.architecture.owners[0].package = "sugarsubstitute-desktop".into();
    fixture.architecture.owners[0].root = "crates/sugarsubstitute-desktop".into();
    fixture.workspace.packages[0].name = "sugarsubstitute-desktop".into();
    write(
        fixture.root(),
        "crates/sugarsubstitute-desktop/Cargo.toml",
        "[features]\ndefault=[]\nboundary-fixtures=[]\n",
    )?;
    write(
        fixture.root(),
        "crates/sugarsubstitute-desktop/src/report.rs",
        "#[cfg(feature=\"boundary-fixtures\")] pub mod copy_fixture;",
    )?;
    write(
        fixture.root(),
        "crates/sugarsubstitute-desktop/src/report/copy_fixture.rs",
        "pub fn with_pending_report_copy() {}",
    )?;
    let policy = Policy { schema_version: 1, owner: None };
    check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)?;
    write(
        fixture.root(),
        "crates/sugarsubstitute-desktop/Cargo.toml",
        "[features]\ndefault=['boundary-fixtures']\nboundary-fixtures=[]\n",
    )?;
    assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    write(
        fixture.root(),
        "crates/sugarsubstitute-desktop/Cargo.toml",
        "[features]\ndefault=[]\nboundary-fixtures=[]\n",
    )?;
    for declaration in [
        "pub mod copy_fixture;",
        "#[cfg(test)] pub mod copy_fixture;",
        "#[cfg(feature=\"boundary-fixtures\")] pub mod copy_fixture; #[path=\"report/copy_fixture.rs\"] mod shipping;",
        "#[cfg(any(test, feature=\"boundary-fixtures\"))] pub mod copy_fixture;",
    ] {
        write(fixture.root(), "crates/sugarsubstitute-desktop/src/report.rs", declaration)?;
        assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    }
    Ok(())
}

#[test]
fn report_copy_cross_parent_import_is_rejected_even_with_legitimate_original_test_gate()
-> Result<()> {
    let fixture = copy_consumer_fixture()?;
    let policy = Policy { schema_version: 1, owner: None };
    check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)?;
    write(fixture.root(), "crates/sugarsubstitute/src/inline/marker.rs", "fn ordinary() {}")?;
    // The genuine consumer contains the same accepted fixture use throughout this test.
    // Only a new importing parent changes; filename and original cfg(test) stay unchanged.
    for source in [
        "#[path=\"production/reporter_role/runtime/control/tests.rs\"] mod shipping;",
        "#[cfg(test)] #[path=\"production/reporter_role/runtime/control/tests.rs\"] mod counterfeit;",
        "#[cfg_attr(target_os=\"windows\",path=\"production/reporter_role/runtime/control/tests.rs\")] mod shipping;",
        "mod inline { #[path=\"../production/reporter_role/runtime/control/tests.rs\"] mod shipping; }",
    ] {
        write(fixture.root(), "crates/sugarsubstitute/src/lib.rs", source)?;
        let cause = check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)
            .err()
            .ok_or_else(|| {
                crate::error::Failure::Policy("Cross-parent consumer import escaped".into())
            })?;
        assert!(
            cause
                .to_string()
                .contains("imports test-only Report source outside its exact gated parent"),
            "{cause}"
        );
    }
    write(fixture.root(), "crates/sugarsubstitute/src/lib.rs", "fn ordinary() {}")?;
    check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)
}
#[test]
fn report_copy_cross_parent_producer_import_cannot_enable_default_off_fixture() -> Result<()> {
    let mut fixture = Repository::new()?;
    fixture.architecture.owners[0].package = "sugarsubstitute-desktop".into();
    fixture.architecture.owners[0].root = "crates/sugarsubstitute-desktop".into();
    fixture.workspace.packages[0].name = "sugarsubstitute-desktop".into();
    write(
        fixture.root(),
        "crates/sugarsubstitute-desktop/Cargo.toml",
        "[features]\ndefault=[]\nboundary-fixtures=[]\n",
    )?;
    write(
        fixture.root(),
        "crates/sugarsubstitute-desktop/src/report.rs",
        "#[cfg(feature=\"boundary-fixtures\")] pub mod copy_fixture;",
    )?;
    write(
        fixture.root(),
        "crates/sugarsubstitute-desktop/src/report/copy_fixture.rs",
        "pub fn with_pending_report_copy() {}",
    )?;
    let policy = Policy { schema_version: 1, owner: None };
    check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)?;
    write(
        fixture.root(),
        "crates/sugarsubstitute-desktop/src/inline/marker.rs",
        "fn ordinary() {}",
    )?;
    for source in [
        "#[path=\"report/copy_fixture.rs\"] mod shipping;",
        "#[cfg(feature=\"boundary-fixtures\")] #[path=\"report/copy_fixture.rs\"] mod counterfeit;",
        "#[cfg_attr(target_os=\"windows\",path=\"report/copy_fixture.rs\")] mod shipping;",
        "mod inline { #[path=\"../report/copy_fixture.rs\"] mod shipping; }",
    ] {
        write(fixture.root(), "crates/sugarsubstitute-desktop/src/lib.rs", source)?;
        let cause = check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)
            .err()
            .ok_or_else(|| {
                crate::error::Failure::Policy("Cross-parent producer import escaped".into())
            })?;
        assert!(
            cause
                .to_string()
                .contains("imports test-only Report source outside its exact gated parent"),
            "{cause}"
        );
    }
    write(fixture.root(), "crates/sugarsubstitute-desktop/src/lib.rs", "fn ordinary() {}")?;
    check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)
}

#[test]
fn report_copy_consumer_rejects_every_target_and_build_production_feature_edge() -> Result<()> {
    let fixture = copy_consumer_fixture()?;
    let policy = Policy { schema_version: 1, owner: None };
    check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)?;
    let dev = "[target.'cfg(windows)'.dev-dependencies]\nsugarsubstitute-desktop={features=['boundary-fixtures']}\n";
    for table in [
        "dependencies",
        "build-dependencies",
        "target.'cfg(windows)'.dependencies",
        "target.'cfg(windows)'.build-dependencies",
        "target.'cfg(unix)'.dependencies",
        "target.'x86_64-pc-windows-msvc'.build-dependencies",
    ] {
        let manifest =
            format!("{dev}[{table}]\nsugarsubstitute-desktop={{features=['boundary-fixtures']}}\n");
        write(fixture.root(), "crates/sugarsubstitute/Cargo.toml", &manifest)?;
        assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    }
    write(fixture.root(), "crates/sugarsubstitute/Cargo.toml", dev)?;
    check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)
}
#[test]
fn report_copy_consumer_rejects_production_package_aliases_with_accepted_dev_edge() -> Result<()> {
    let fixture = copy_consumer_fixture()?;
    let policy = Policy { schema_version: 1, owner: None };
    let dev = "[target.'cfg(windows)'.dev-dependencies]\nsugarsubstitute-desktop={features=['boundary-fixtures']}\n";
    check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)?;
    for table in [
        "dependencies",
        "build-dependencies",
        "target.'cfg(windows)'.dependencies",
        "target.'cfg(windows)'.build-dependencies",
        "target.'cfg(unix)'.build-dependencies",
    ] {
        let manifest = format!(
            "{dev}[{table}]\ndesktop-alias={{package='sugarsubstitute-desktop',features=['boundary-fixtures']}}\n"
        );
        write(fixture.root(), "crates/sugarsubstitute/Cargo.toml", &manifest)?;
        assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    }
    let unrelated = format!(
        "{dev}[dependencies]\nordinary-adapter={{package='ordinary-adapter',features=['boundary-fixtures']}}\n"
    );
    write(fixture.root(), "crates/sugarsubstitute/Cargo.toml", &unrelated)?;
    check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)
}
