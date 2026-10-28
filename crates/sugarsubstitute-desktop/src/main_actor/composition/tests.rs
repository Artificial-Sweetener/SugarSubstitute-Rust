// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise retirement after failed native visual-tree effects.

use super::*;

#[test]
fn retirement_preserves_failed_mutations_and_commits_after_both_detachments() {
    let mut ledger =
        Ledger { browser_bound: true, tree_bound: true, dirty: false, committed: false };
    let mut actions = Vec::new();
    for expected in [Retire::DetachBrowser, Retire::DetachTree, Retire::Commit] {
        let failed = ledger.step(|action| {
            actions.push(action);
            Err::<(), _>("injected failure")
        });
        assert_eq!(failed, Err("injected failure"));
        assert_eq!(ledger.next(), expected);
        assert_eq!(
            ledger.step(|action| {
                actions.push(action);
                Ok::<_, &str>(())
            }),
            Ok(false)
        );
    }
    assert_eq!(ledger.step(|_| Err::<(), _>("must not run")), Ok(true));
    assert_eq!(
        actions,
        [
            Retire::DetachBrowser,
            Retire::DetachBrowser,
            Retire::DetachTree,
            Retire::DetachTree,
            Retire::Commit,
            Retire::Commit
        ]
    );
}
