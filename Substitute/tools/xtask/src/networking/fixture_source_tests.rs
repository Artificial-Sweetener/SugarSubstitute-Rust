// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Root graph fixture registration cannot authorize its production neighbors.
use super::fixture_source;

#[test]
fn root_graph_fixture_does_not_grant_runtime_source_a_fixture_disposition() {
    assert!(fixture_source("installer/worker-graph.test.mjs"));
    for path in [
        "installer/worker-graph.mjs",
        "installer/worker/http.ts",
        "installer/worker/proxy.ts",
        "installer/release/windows-imports.mjs",
    ] {
        assert!(!fixture_source(path), "Runtime source admitted as fixture: {path}");
    }
}
