// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Model only external HTTP acquisition, served bytes and actual per-slot closure.
use super::{
    ReadinessTransportError,
    owner::{HttpPort, Transaction, lock},
    probe::Endpoint,
    types::ReadinessProbeInput,
};
use std::{
    error::Error,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::{JobContext, Moment};

/// External HTTP behavior supplied to a real probe owner during crate tests.
#[derive(Clone, Copy)]
pub(crate) enum Behavior {
    Success,
    Unavailable,
    Malformed,
    WrongAsset,
    Panicked,
    CleanupPanicked,
}
/// Fixed external boundary with causal acquisition/closure gates.
pub(crate) struct FixtureHttp {
    pub(crate) behavior: Behavior,
    pub(crate) closed: Mutex<Vec<usize>>,
    pub(crate) phases: Mutex<Vec<(&'static str, u64)>>,
    fail_close: AtomicBool,
    panic_close: AtomicBool,
    gate: Mutex<(bool, bool)>,
    changed: Condvar,
    gated_phase: Option<&'static str>,
    clock: Arc<AtomicU64>,
    finish_at: Option<Moment>,
}
impl FixtureHttp {
    pub(crate) fn new(
        behavior: Behavior,
        gated_phase: Option<&'static str>,
        fail_close: bool,
        clock: Arc<AtomicU64>,
        finish_at: Option<Moment>,
    ) -> Arc<Self> {
        Arc::new(Self {
            behavior,
            closed: Mutex::new(Vec::new()),
            phases: Mutex::new(Vec::new()),
            fail_close: AtomicBool::new(fail_close),
            panic_close: AtomicBool::new(matches!(behavior, Behavior::CleanupPanicked)),
            gate: Mutex::new((false, false)),
            changed: Condvar::new(),
            gated_phase,
            clock,
            finish_at,
        })
    }
    pub(crate) fn entered(&self) -> Result<(), Box<dyn Error>> {
        let until = Instant::now() + Duration::from_secs(5);
        let mut guard = lock(&self.gate);
        while !guard.0 {
            let remaining =
                until.checked_duration_since(Instant::now()).ok_or("Missing causal HTTP entry")?;
            let (next, status) = self
                .changed
                .wait_timeout(guard, remaining)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            guard = next;
            if status.timed_out() {
                return Err("Missing causal HTTP entry".into());
            }
        }
        Ok(())
    }
    pub(crate) fn release(&self) {
        lock(&self.gate).1 = true;
        self.changed.notify_all();
    }
    fn phase(&self, name: &'static str, context: &JobContext) {
        lock(&self.phases).push((name, context.deadline().map_or(0, Moment::as_nanos)));
        if self.gated_phase == Some(name) {
            let mut guard = lock(&self.gate);
            guard.0 = true;
            self.changed.notify_all();
            while !guard.1 {
                guard = self.changed.wait(guard).unwrap_or_else(std::sync::PoisonError::into_inner);
            }
        }
    }
}
impl HttpPort for FixtureHttp {
    fn begin(
        &self,
        state: &mut Transaction,
        _: u16,
        context: &JobContext,
    ) -> Result<(), ReadinessTransportError> {
        state.session = 1;
        state.connect = 2;
        state.request = 3;
        self.phase("begin", context);
        if matches!(self.behavior, Behavior::Panicked) {
            std::panic::resume_unwind(Box::new("original-campaign-panic"));
        }
        if matches!(self.behavior, Behavior::Unavailable | Behavior::CleanupPanicked) {
            return Err(ReadinessTransportError::unavailable(
                "fixture_connection",
                std::io::Error::other("original-unavailable-cause"),
            ));
        }
        Ok(())
    }
    fn get(
        &self,
        state: &mut Transaction,
        _: &ReadinessProbeInput,
        endpoint: Endpoint<'_>,
        context: &JobContext,
        consume: &mut dyn FnMut(&[u8]) -> Result<(), ReadinessTransportError>,
    ) -> Result<(), ReadinessTransportError> {
        self.phase("get", context);
        state.peer = Some(
            sugarsubstitute_supervision::backend_listener::boundary_fixture::fixed_http_peer(),
        );
        let body: &[u8] = match endpoint.path() {
            "/system_stats" if matches!(self.behavior, Behavior::Malformed) => b"{broken",
            "/system_stats" => br#"{"system":{"os":"win32","ram_total":68719476736,"ram_free":40000000000,"comfyui_version":"0.29.0","required_frontend_version":"1.47.11","installed_templates_version":"1.0.0","required_templates_version":"1.0.0","comfy_package_versions":[{"name":"comfyui-frontend-package","installed":"1.47.11","required":"1.47.11"}],"python_version":"3.12.0","pytorch_version":"2.9.0","embedded_python":false,"deploy_environment":"local","argv":["unowned.py"]},"devices":[]}"#,
            "/extensions" => br#"["/extensions/SugarCubes/main.js"]"#,
            "/index.html" if matches!(self.behavior, Behavior::WrongAsset) => b"altered index",
            "/index.html" => b"fixture index",
            "/extensions/SugarCubes/main.js" => b"fixture cube",
            _ => return Err(ReadinessTransportError::invalid("unexpected_fixture_endpoint")),
        };
        for chunk in body.chunks(7) {
            consume(chunk)?;
        }
        Ok(())
    }
    fn close(
        &self,
        state: &mut Transaction,
        context: &JobContext,
    ) -> Result<(), ReadinessTransportError> {
        self.phase("close", context);
        for slot in [&mut state.request, &mut state.connect, &mut state.session] {
            if *slot != 0 {
                lock(&self.closed).push(*slot);
                *slot = 0;
                if self.panic_close.swap(false, Ordering::AcqRel) {
                    std::panic::resume_unwind(Box::new("original-cleanup-panic"));
                }
                if self.fail_close.swap(false, Ordering::AcqRel) {
                    return Err(ReadinessTransportError::new(
                        "fixture_close",
                        std::io::Error::other("original-close-cause"),
                    ));
                }
            }
        }
        if let Some(at) = self.finish_at {
            self.clock.store(at.as_nanos(), Ordering::Release);
        }
        Ok(())
    }
}
