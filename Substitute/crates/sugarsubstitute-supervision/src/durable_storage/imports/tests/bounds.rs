// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Nested sibling inventories share one traversal reservation instead of multiplying it by depth.
use super::*;

#[test]
fn nested_wide_unicode_names_share_one_admission_before_the_tree_is_accepted() -> TestResult {
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context)?;
        let nested = fixture.source.path().join("__nested");
        std::fs::create_dir(&nested)?;
        let prefix = "糖".repeat(245);
        for batch in (0..2600).step_by(64) {
            let _returned_batch =
                tracing::trace_span!(target: "sugarsubstitute::storage", "aggregate_fixture_batch")
                    .entered();
            for index in batch..(batch + 64).min(2600) {
                check(context)?;
                let name = format!("{prefix}-{index:04}");
                std::fs::write(fixture.source.path().join(&name), b"root original")?;
                std::fs::write(nested.join(&name), b"nested original")?;
            }
        }
        let _workspace = context.memory().claim(WORKSPACE_BYTES)?;
        let cause =
            super::super::tree::observe(&fixture.source, RootEntries::Except(&[".lease"]), context)
                .err()
                .ok_or("Unbounded nested inventory was accepted")?;
        assert!(cause.to_string().contains("aggregate traversal admission"), "{cause}");
        let last = format!("{prefix}-2599");
        assert_eq!(std::fs::read(fixture.source.path().join(&last))?, b"root original");
        assert_eq!(std::fs::read(nested.join(&last))?, b"nested original");
        Ok(())
    })
}
