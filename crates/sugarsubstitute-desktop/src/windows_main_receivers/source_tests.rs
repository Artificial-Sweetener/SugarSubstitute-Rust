// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Protect actual native subscription admission independently of frontend script diagnostics.

use super::*;
use std::error::Error;
use sugarsubstitute_contracts::execution::MemoryRelease;

struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}

#[test]
fn native_subscription_inventory_cannot_route_unowned_runtime_exceptions_into_readiness()
-> Result<(), Box<dyn Error>> {
    let lease = MemoryLease::new(4 * 1024 * 1024, Box::new(Release));
    let events = Events::new(&lease, || {})?;
    let unowned = "Runtime.exceptionThrown";
    for payload in [
        r#"{"timestamp":1,"exceptionDetails":{"exceptionId":1,"text":"unhandled frontend diagnostic","lineNumber":0,"columnNumber":0,"executionContextId":7}}"#,
        r#"{"timestamp":1,"exceptionDetails":{"exceptionId":2,"text":"another extension diagnostic","lineNumber":0,"columnNumber":0,"executionContextId":99}}"#,
    ] {
        for (method, kind) in SOURCES {
            if method == unowned {
                events.record(kind, |storage| {
                    storage.push_str(payload);
                    Ok(())
                });
            }
        }
        assert_eq!(events.remaining(), 0);
        assert!(!events.consume(|_| Err(DesktopError::message(
            "test_subscription",
            "Unexpected unowned exception admission"
        )))?);
    }
    assert_eq!(SOURCES.len(), 6);
    for (index, (method, kind)) in SOURCES.into_iter().enumerate() {
        assert!(!SOURCES[..index].iter().any(|(prior, _)| *prior == method));
        events.record(kind, |_| Ok(()));
        assert!(events.consume(|observation| {
            assert_eq!(observation.kind, kind);
            Ok(())
        })?);
    }
    assert_eq!(events.remaining(), 0);
    assert!(
        SOURCES
            .iter()
            .any(|(method, kind)| *method == "Runtime.consoleAPICalled" && *kind == Kind::Console)
    );
    assert!(SOURCES.iter().any(|(method, kind)| *method == "Runtime.executionContextDestroyed"
        && *kind == Kind::ContextDestroyed));
    assert!(SOURCES.iter().any(|(method, kind)| *method == "Runtime.executionContextsCleared"
        && *kind == Kind::ContextsCleared));
    Ok(())
}
