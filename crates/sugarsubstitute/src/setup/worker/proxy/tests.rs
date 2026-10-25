// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe admitted lookup completion, output overload and repeated close without losing custody.
use super::*;
use std::{num::NonZeroUsize, sync::Condvar, time::Instant};
use sugarsubstitute_contracts::execution::{CompletionWake, ExecutionError};
use sugarsubstitute_execution::{ExecutionOwner, MonotonicClock};

#[test]
fn native_routes_publish_the_version_one_worker_contract_in_order() {
    assert_eq!(
        route_reply(vec![Route::Proxy { url: "https://proxy.test:8443/".into() }, Route::Direct]),
        serde_json::json!({"routes":[{"kind":"proxy","url":"https://proxy.test:8443/"},{"kind":"direct"}]})
    );
}

#[derive(Default)]
struct Wake(Mutex<u64>, Condvar);
impl CompletionWake for Wake {
    fn wake(&self) -> std::result::Result<(), ExecutionError> {
        *self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        self.1.notify_all();
        Ok(())
    }
}
fn observe(wake: &Wake, mut step: impl FnMut() -> Result<bool>) -> Result<()> {
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        let revision = *wake.0.lock().map_err(|_| "Proxy fixture poisoned")?;
        if step()? {
            return Ok(());
        }
        let remaining =
            until.checked_duration_since(Instant::now()).ok_or("Proxy fixture expired")?;
        let (_guard, timeout) = wake
            .1
            .wait_timeout_while(
                wake.0.lock().map_err(|_| "Proxy fixture poisoned")?,
                remaining,
                |value| *value == revision,
            )
            .map_err(|_| "Proxy fixture poisoned")?;
        if timeout.timed_out() {
            return Err("Proxy fixture expired".into());
        }
    }
}

#[test]
fn overloaded_reply_retains_completion_until_close_retires_it() -> Result<()> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(MonotonicClock::new(), wake.clone())?;
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
    let owner = ExecutionOwner::new(MonotonicClock::new(), wake.clone())?;
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
