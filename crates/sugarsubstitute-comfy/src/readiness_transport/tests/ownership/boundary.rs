// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fake only native HTTP acquisition, independent response bytes and checked per-slot closure.
use super::*;
use std::{
    sync::{
        Condvar,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
pub(super) struct Boundary {
    clock: Arc<super::harness::TestClock>,
    slots: usize,
    late: bool,
    gated: bool,
    gate: Mutex<(bool, bool)>,
    changed: Condvar,
    fail_close: AtomicBool,
    pub(super) closed: Mutex<Vec<usize>>,
    pub(super) phases: Mutex<Vec<(&'static str, u64)>>,
    pub(super) requests: Mutex<Vec<String>>,
}
impl Boundary {
    pub(super) fn new(
        clock: Arc<super::harness::TestClock>,
        slots: usize,
        late: bool,
        gated: bool,
        fail_close: bool,
    ) -> Self {
        Self {
            clock,
            slots,
            late,
            gated,
            gate: Mutex::new((false, false)),
            changed: Condvar::new(),
            fail_close: AtomicBool::new(fail_close),
            closed: Mutex::new(Vec::new()),
            phases: Mutex::new(Vec::new()),
            requests: Mutex::new(Vec::new()),
        }
    }
    fn phase(&self, name: &'static str, context: &JobContext) {
        lock(&self.phases).push((name, context.deadline().map_or(0, Moment::as_nanos)));
    }
    pub(super) fn entered(&self) -> TestResult {
        let until = Instant::now() + Duration::from_secs(5);
        let mut guard = lock(&self.gate);
        while !guard.0 {
            let remaining =
                until.checked_duration_since(Instant::now()).ok_or("HTTP boundary not entered")?;
            let (next, status) = self
                .changed
                .wait_timeout(guard, remaining)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            guard = next;
            if status.timed_out() {
                return Err("HTTP boundary not entered".into());
            }
        }
        Ok(())
    }
    pub(super) fn release(&self) {
        lock(&self.gate).1 = true;
        self.changed.notify_all();
    }
}
impl HttpPort for Boundary {
    fn begin(
        &self,
        state: &mut Transaction,
        _: u16,
        context: &JobContext,
    ) -> Result<(), ReadinessTransportError> {
        self.phase("begin", context);
        state.fixture_slots[0] = 1;
        if self.slots != 1 {
            state.fixture_slots[1] = 2;
        }
        if !matches!(self.slots, 1 | 2) {
            state.fixture_slots[2] = 3;
        }
        if self.gated {
            let mut guard = lock(&self.gate);
            guard.0 = true;
            self.changed.notify_all();
            while !guard.1 {
                guard = self.changed.wait(guard).unwrap_or_else(std::sync::PoisonError::into_inner);
            }
        }
        if self.late {
            self.clock.0.store(500, Ordering::Release);
        }
        if self.slots > 0 {
            return Err(ReadinessTransportError::new(
                "fake_begin",
                std::io::Error::other("original-begin-cause"),
            ));
        }
        Ok(())
    }
    fn get(
        &self,
        _: &mut Transaction,
        _: &ReadinessProbeInput,
        endpoint: super::super::super::probe::Endpoint<'_>,
        context: &JobContext,
        consume: &mut dyn FnMut(&[u8]) -> Result<(), ReadinessTransportError>,
    ) -> Result<(), ReadinessTransportError> {
        self.phase("get", context);
        lock(&self.requests).push(endpoint.path().into());
        let body:&[u8]=match endpoint.path() {
            "/system_stats"=>br#"{"system":{"os":"win32","ram_total":68719476736,"ram_free":40000000000,"comfyui_version":"0.29.0","required_frontend_version":"1.47.11","installed_templates_version":"1.0.0","required_templates_version":"1.0.0","comfy_package_versions":[{"name":"comfyui-frontend-package","installed":"1.47.11","required":"1.47.11"}],"python_version":"3.12.0","pytorch_version":"2.9.0","embedded_python":false,"deploy_environment":"local","argv":["unowned.py"]},"devices":[]}"#,
            "/extensions"=>br#"["/extensions/SugarCubes/main.js"]"#,
            "/index.html"=>b"fixture index",
            "/extensions/SugarCubes/main.js"=>b"fixture cube",
            _=>return Err(ReadinessTransportError::invalid("unexpected_fixture_endpoint")),
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
        for slot in state.fixture_slots.iter_mut().rev() {
            if *slot != 0 {
                lock(&self.closed).push(*slot);
                *slot = 0;
                if self.fail_close.swap(false, Ordering::AcqRel) {
                    return Err(ReadinessTransportError::new(
                        "fake_close",
                        std::io::Error::other("original-close-cause"),
                    ));
                }
            }
        }
        Ok(())
    }
}
