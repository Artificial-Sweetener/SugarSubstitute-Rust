// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe admitted lookup completion, output overload and repeated close without losing custody.
use super::*;
use crate::{completion_wake::Wake, paused_clock::PausedClock};
use std::num::NonZeroUsize;
use sugarsubstitute_execution::ExecutionOwner;

#[test]
fn native_routes_publish_the_version_one_worker_contract_in_order() {
    assert_eq!(
        route_reply(vec![Route::Proxy { url: "https://proxy.test:8443/".into() }, Route::Direct]),
        serde_json::json!({"routes":[{"kind":"proxy","url":"https://proxy.test:8443/"},{"kind":"direct"}]})
    );
}

/// Re-examine `step` after each execution wake; slow lookups only make the observation longer.
fn observe(wake: &Wake, mut step: impl FnMut() -> Result<bool>) -> Result<()> {
    wake.until(|| step().map(|done| done.then_some(())).transpose())
}

#[test]
fn overloaded_reply_retains_completion_until_close_retires_it() -> Result<()> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(PausedClock::new(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let jobs = JobSequence::new();
    let (transport, _runner) = Transport::pair();
    let mut proxies = Proxies::default();
    let request = serde_json::json!({"id":"lookup", "event":"proxy_resolve", "data":{"url":"file:///rejected"}});
    proxies.receive(&request, &client, &scope, &jobs, &transport)?;
    assert!(proxies.receive(&request, &client, &scope, &jobs, &transport).is_err());
    // Fill the actual transport queue, proving failed publication retains the original reply.
    while transport.send("{}".into()).is_ok() {}
    observe(&wake, || {
        client.drain_completions(NonZeroUsize::new(32).ok_or("Invalid fixture batch")?)?;
        match proxies.advance(&client, &scope, &jobs, Some(&transport)) {
            Ok(()) => Ok(false),
            Err(_) => Ok(true),
        }
    })?;
    assert!(!proxies.settled());
    let pending = proxies.pending.get("lookup").ok_or("Lookup lost")?;
    assert!(pending.job.is_none());
    assert!(pending.reply.as_ref().is_some_and(|value| value["error"].is_string()));
    proxies.close();
    proxies.close();
    proxies.advance(&client, &scope, &jobs, None)?;
    assert!(proxies.settled());
    Ok(())
}

#[test]
fn cancellation_retires_all_admitted_lookups_and_identity_validation_precedes_admission()
-> Result<()> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(PausedClock::new(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let jobs = JobSequence::new();
    let (transport, _runner) = Transport::pair();
    let mut proxies = Proxies::default();
    assert!(
        proxies
            .receive(&serde_json::json!({"id":"bad/id"}), &client, &scope, &jobs, &transport)
            .is_err()
    );
    assert!(proxies.settled());
    for index in 0..CAPACITY {
        proxies.receive(&serde_json::json!({"id":format!("lookup_{index}"),"event":"proxy_resolve","data":{"url":"file:///rejected"}}), &client, &scope, &jobs, &transport)?;
    }
    proxies.close();
    assert!(!proxies.settled());
    observe(&wake, || {
        client.drain_completions(NonZeroUsize::new(32).ok_or("Invalid fixture batch")?)?;
        proxies.advance(&client, &scope, &jobs, None)?;
        Ok(proxies.settled())
    })
}
