// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bounded proxy resolution jobs belong to the provisioning worker's existing execution scope.
#[cfg(test)]
mod tests;
use super::*;
use std::{
    collections::BTreeMap,
    io,
    sync::{Arc, Mutex},
};
use sugarsubstitute_networking::system_proxy::{RESERVATION_BYTES, Resolver, Route};

const CAPACITY: usize = 8;
/// Retain original native custody independently of discarded or undelivered job outcomes.
struct Pending {
    resolver: Arc<Mutex<Resolver>>,
    job: Option<OwnedJob<Vec<Route>, io::Error>>,
    reply: Option<serde_json::Value>,
    retiring: bool,
    cancelled: bool,
}
#[derive(Default)]
/// The worker's finite admission set, including replies waiting for transport capacity.
pub(super) struct Proxies {
    pending: BTreeMap<String, Pending>,
}
impl Proxies {
    /// Validate the worker's bounded URL request before granting a native resolution job.
    pub(super) fn receive(
        &mut self,
        value: &serde_json::Value,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        transport: &Transport,
    ) -> Result<()> {
        let id = value["id"]
            .as_str()
            .filter(|id| {
                !id.is_empty()
                    && id.len() <= 80
                    && id
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            })
            .ok_or("Invalid proxy request identity")?;
        if value["event"] == "proxy_cancel" {
            if let Some(pending) = self.pending.get_mut(id) {
                pending.cancelled = true;
                if !pending.retiring
                    && let Some(job) = &pending.job
                {
                    job.cancel();
                }
            }
            return Ok(());
        }
        if self.pending.contains_key(id) {
            return Err("Duplicate proxy request identity".into());
        }
        if self.pending.len() >= CAPACITY {
            return send(
                transport,
                id,
                serde_json::json!({"error":"Proxy resolution capacity reached"}),
            );
        }
        let address = value["data"]["url"]
            .as_str()
            .filter(|value| value.len() <= 8192)
            .ok_or("Invalid proxy request URL")?
            .to_owned();
        let resolver = Arc::new(Mutex::new(Resolver::new()));
        let owned = resolver.clone();
        let job =
            client.submit_blocking(scope, specification(client, jobs, false)?, move |context| {
                owned
                    .lock()
                    .map_err(|_| io::Error::other("Proxy resolver custody poisoned"))?
                    .resolve(&address, &context)
            })?;
        self.pending.insert(
            id.to_owned(),
            Pending { resolver, job: Some(job), reply: None, retiring: false, cancelled: false },
        );
        Ok(())
    }
    /// Observe completion and native retirement before replying or releasing admission.
    pub(super) fn advance(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        transport: Option<&Transport>,
    ) -> Result<()> {
        let mut finished = Vec::new();
        let mut failure = None;
        for (id, pending) in &mut self.pending {
            match pending.advance(id, client, scope, jobs, transport) {
                Ok(true) => finished.push(id.clone()),
                Ok(false) => {}
                Err(cause) => {
                    failure = Some(cause);
                    break;
                }
            }
        }
        for id in finished {
            self.pending.remove(&id);
        }
        if let Some(cause) = failure {
            return Err(cause);
        }
        Ok(())
    }
    /// Cancellation stops lookup, while retirement jobs retain their original custody.
    pub(super) fn close(&mut self) {
        for pending in self.pending.values_mut() {
            pending.cancelled = true;
            if !pending.retiring
                && let Some(job) = &pending.job
            {
                job.cancel();
            }
        }
    }
    /// No lookup, pending publication or native retirement remains owned by this worker.
    pub(super) fn settled(&self) -> bool {
        self.pending.is_empty()
    }
}
impl Pending {
    /// Keep a completed reply or unresolved native custody across failed delivery/admission.
    fn advance(
        &mut self,
        id: &str,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        transport: Option<&Transport>,
    ) -> Result<bool> {
        if let Some(job) = &mut self.job {
            let Some(completion) = job.try_take_completion() else {
                return Ok(false);
            };
            self.job = None;
            if !self.retiring {
                self.reply = Some(match completion.outcome {
                    JobOutcome::Success(routes) => route_reply(routes),
                    JobOutcome::Failure(cause) => {
                        serde_json::json!({"error":format!("Windows proxy resolution failed: {cause}")})
                    }
                    _ => {
                        serde_json::json!({"error":"Windows proxy resolution was cancelled or expired"})
                    }
                });
            } else {
                match completion.outcome {
                    JobOutcome::Success(_) => {}
                    JobOutcome::Failure(cause) => return Err(cause.into()),
                    _ => {
                        return Err(
                            "Windows proxy retirement did not complete; custody retained".into()
                        );
                    }
                }
            }
        }
        let settled =
            self.resolver.lock().unwrap_or_else(std::sync::PoisonError::into_inner).settled();
        if !settled {
            let owned = self.resolver.clone();
            self.job = Some(client.submit_blocking(
                scope,
                specification(client, jobs, true)?,
                move |_| {
                    owned.lock().unwrap_or_else(std::sync::PoisonError::into_inner).retire()?;
                    Ok(Vec::new())
                },
            )?);
            self.retiring = true;
            return Ok(false);
        }
        if !self.cancelled
            && let Some(transport) = transport
        {
            send(transport, id, self.reply.as_ref().ok_or("Proxy reply missing")?.clone())?;
        }
        Ok(true)
    }
}
/// The host owns the wire representation; native routing decisions contain no protocol types.
fn route_reply(routes: Vec<Route>) -> serde_json::Value {
    let routes: Vec<_> = routes
        .into_iter()
        .map(|route| match route {
            Route::Direct => serde_json::json!({"kind":"direct"}),
            Route::Proxy { url } => serde_json::json!({"kind":"proxy","url":url}),
        })
        .collect();
    serde_json::json!({"routes":routes})
}
/// Lookup deadlines cover discovery; cleanup keeps the original native owner until retirement.
fn specification(
    client: &ExecutionClient,
    jobs: &JobSequence,
    retirement: bool,
) -> Result<JobSpec> {
    Ok(JobSpec {
        id: jobs.allocate()?,
        operation: Operation::Infrastructure,
        class: if retirement {
            JobClass::InfrastructureLifecycle
        } else {
            JobClass::InfrastructureOrdinary
        },
        deadline: if retirement {
            None
        } else {
            Some(client.now()?.checked_add(Duration::from_secs(60))?)
        },
        reservation_bytes: RESERVATION_BYTES,
        replacement: None,
    })
}
/// Publish one correlated result without blocking the UI thread.
fn send(transport: &Transport, id: &str, input: serde_json::Value) -> Result<()> {
    transport.send(serde_json::to_string(
        &serde_json::json!({"version":1,"id":id,"operation":"proxy_result","input":input}),
    )?)?;
    Ok(())
}

impl Worker {
    /// Admit private proxy messages only after the worker's authenticated protocol handshake.
    pub(super) fn proxy_event(&mut self, value: &serde_json::Value) -> Result<bool> {
        if !matches!(value["event"].as_str(), Some("proxy_resolve" | "proxy_cancel")) {
            return Ok(false);
        }
        if !self.closing {
            if !self.ready {
                return Err("Proxy request preceded worker readiness".into());
            }
            let transport = self.transport.as_ref().ok_or("Provisioning transport absent")?;
            self.proxies.receive(value, &self.client, &self.scope, &self.jobs, transport)?;
        }
        Ok(true)
    }
}
