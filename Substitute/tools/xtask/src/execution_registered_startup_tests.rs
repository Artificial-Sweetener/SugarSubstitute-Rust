// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Confine sealed registered startup producers while allowing typed consumers.
use super::*;
/// Closed original specification producers reject aliases/macros while typed consumers remain valid.
#[test]
fn registered_startup_producer_remains_with_execution_owner() -> Result<()> {
    let fixture = Repository::new()?;
    let policy = Policy { schema_version: 1, owner: None };
    for source in [
        "fn forge() { sugarsubstitute_execution::RegisteredStartupSource::new(); }",
        "fn forge() { sugarsubstitute_execution::coordinator_entry::RegisteredRestartAdmission::new(); }",
        "use sugarsubstitute_execution::coordinator_entry::RegisteredRestartAdmission as Witness; fn forge() { Witness::new(); }",
        "macro_rules! forged { ($t:ident,$m:ident) => { $t::$m() }; } fn forge() { forged!(RegisteredRestartAdmission,new); }",
        "use sugarsubstitute_execution::RegisteredStartupSource as Source; fn forge() { Source::new(); }",
        "type Snapshot = sugarsubstitute_execution::PreparedStartupSnapshot; fn forge() { Snapshot::new(); }",
        "macro_rules! forged { () => { RegisteredStartupSource::new() }; }",
        "macro_rules! forged { ($t:ident,$m:ident) => { $t::$m() }; } fn forge() { forged!(PreparedStartupSnapshot,new); }",
        "#[cfg(test)] mod tests { fn forge() { RegisteredStartupSource::new(); } }",
    ] {
        assert!(!crate::execution_source::resource_producer_findings(source)?.is_empty());
        write(fixture.root(), "crates/host/src/restart.rs", source)?;
        assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    }
    let valid = "use sugarsubstitute_execution::RegisteredStartupSource; fn use_source(source:&RegisteredStartupSource) { source.view(); client.restart_startup_snapshot(source,previous,run,original,&actual_context); client.borrow_registered_restart_admission(snapshot,source,completed); }";
    assert!(crate::execution_source::resource_producer_findings(valid)?.is_empty());
    assert!(crate::execution_source::findings(valid)?.is_empty());
    Ok(())
}
