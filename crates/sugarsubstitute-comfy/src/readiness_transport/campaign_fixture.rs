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
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::execution::{JobContext, Moment};

/// External HTTP behavior supplied to a real probe owner during crate tests.
#[derive(Clone, Copy)]
pub(crate) enum Behavior {
    Success,
    Unavailable,
    ListenerAbsent,
    Malformed,
    WrongAsset,
    Panicked,
    CleanupPanicked,
    NativeTimeout,
    SubmillisecondBudget,
}
/// Fixed external boundary with causal acquisition/closure gates.
pub(crate) struct FixtureHttp {
    pub(crate) behavior: Behavior,
    pub(crate) closed: Mutex<Vec<usize>>,
    pub(crate) phases: Mutex<Vec<(&'static str, u64)>>,
    /// Actual endpoint observations made by the retained probe worker.
    pub(crate) requests: Mutex<Vec<String>>,
    /// Deterministic external response time; does not schedule work.
    pub(crate) response_nanos: AtomicU64,
    /// Corrupt one selected external body, leaving the expected source unchanged.
    pub(crate) corrupt_asset: AtomicUsize,
    /// Change served metadata without replacing the pinned expected source.
    pub(crate) stale_metadata: AtomicBool,
    /// Change served metadata after an actual asset body has been consumed.
    pub(crate) change_metadata_after_asset: AtomicBool,
    /// Every phase must borrow the identical original claimed transaction.
    claim_identity: AtomicUsize,
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
            requests: Mutex::new(Vec::new()),
            response_nanos: AtomicU64::new(0),
            corrupt_asset: AtomicUsize::new(usize::MAX),
            stale_metadata: AtomicBool::new(false),
            change_metadata_after_asset: AtomicBool::new(false),
            claim_identity: AtomicUsize::new(0),
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
        drop(
            self.changed
                .wait_while(lock(&self.gate), |gate| !gate.0)
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
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
        let identity = std::ptr::from_ref(state) as usize;
        if self
            .claim_identity
            .compare_exchange(0, identity, Ordering::AcqRel, Ordering::Acquire)
            .is_err_and(|original| original != identity)
        {
            return Err(ReadinessTransportError::invalid("fixture_changed_original_claim"));
        }
        if matches!(self.behavior, Behavior::ListenerAbsent) {
            self.phase("listener_absent", context);
            return Err(ReadinessTransportError::unavailable(
                "http_listener_absent",
                std::io::Error::from(std::io::ErrorKind::ConnectionRefused),
            ));
        }
        state.fixture_slots[0] = 1;
        state.fixture_slots[1] = 2;
        state.fixture_slots[2] = 3;
        self.phase("begin", context);
        if matches!(self.behavior, Behavior::SubmillisecondBudget) {
            sugarsubstitute_networking::milliseconds(context.deadline(), context.now(), None)
                .map_err(|cause| {
                    super::native::retry::classify(
                        (
                            "http_submillisecond_deadline",
                            sugarsubstitute_networking::Cause::Io(std::io::Error::new(
                                std::io::ErrorKind::TimedOut,
                                cause,
                            )),
                        ),
                        false,
                        context,
                    )
                })?;
            return Err(ReadinessTransportError::invalid("fixture_expected_submillisecond_budget"));
        }
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
        lock(&self.requests).push(endpoint.path().to_owned());
        state.peer = Some(
            sugarsubstitute_supervision::backend_listener::boundary_fixture::fixed_http_peer(),
        );
        if matches!(self.behavior, Behavior::NativeTimeout) {
            let cause = super::native::retry::classify(
                (
                    "http_body_read",
                    sugarsubstitute_networking::Cause::Native(
                        windows::core::HRESULT::from_win32(12002).into(),
                    ),
                ),
                state.peer.is_some(),
                context,
            );
            self.phase(cause.stage(), context);
            return Err(cause);
        }
        let body: &[u8] = match endpoint.path() {
            "/system_stats" if matches!(self.behavior, Behavior::Malformed) => b"{broken",
            "/system_stats" if self.stale_metadata.load(Ordering::Acquire) => b"{broken",
            "/system_stats" => br#"{"system":{"os":"win32","ram_total":68719476736,"ram_free":40000000000,"comfyui_version":"0.29.0","required_frontend_version":"1.47.11","installed_templates_version":"1.0.0","required_templates_version":"1.0.0","comfy_package_versions":[{"name":"comfyui-frontend-package","installed":"1.47.11","required":"1.47.11"}],"python_version":"3.12.0","pytorch_version":"2.9.0","embedded_python":false,"deploy_environment":"local","argv":["unowned.py"]},"devices":[]}"#,
            "/extensions" => br#"["/extensions/SugarCubes/main.js"]"#,
            "/index.html" if matches!(self.behavior, Behavior::WrongAsset) => b"altered index",
            "/index.html" => b"fixture index",
            "/extensions/SugarCubes/main.js" => b"fixture cube",
            path if path.starts_with("/assets/") => {
                let corrupt = self.corrupt_asset.load(Ordering::Acquire);
                if path == format!("/assets/{corrupt}.bin") { b"corrupt asset" } else { b"fixture asset" }
            }
            _ => return Err(ReadinessTransportError::invalid("unexpected_fixture_endpoint")),
        };
        for chunk in body.chunks(7) {
            consume(chunk)?;
        }
        if matches!(endpoint, Endpoint::Asset(_))
            && self.change_metadata_after_asset.swap(false, Ordering::AcqRel)
        {
            self.stale_metadata.store(true, Ordering::Release);
        }
        self.clock.fetch_add(self.response_nanos.load(Ordering::Acquire), Ordering::AcqRel);
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
