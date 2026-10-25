// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify real CPU/GPU document preparation and retained repair through admitted execution.
use std::{
    error::Error,
    io,
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::{ExecutionOwner, MonotonicClock};
use sugarsubstitute_image_gpu::{AdapterCatalog, AdapterDescriptor, AdapterKey, GpuDevice};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
#[path = "frame_qualification/scene.rs"]
mod scene;
#[derive(Default)]
struct Wake(Mutex<u64>, Condvar);
impl CompletionWake for Wake {
    fn wake(&self) -> std::result::Result<(), ExecutionError> {
        let mut revision = self.0.lock().map_err(|_| {
            ExecutionError::new(
                Operation::Infrastructure,
                Box::new(io::Error::other("GPU qualification wake poisoned")),
            )
        })?;
        *revision += 1;
        self.1.notify_all();
        Ok(())
    }
}
impl Wake {
    fn until<T>(
        &self,
        cadence: Option<Duration>,
        mut observe: impl FnMut() -> Result<Option<T>>,
    ) -> Result<T> {
        let cutoff = Instant::now() + Duration::from_secs(90);
        loop {
            let revision = *self.0.lock().map_err(|_| "GPU qualification wake poisoned")?;
            if let Some(value) = observe()? {
                return Ok(value);
            }
            let remaining = cutoff
                .checked_duration_since(Instant::now())
                .ok_or("GPU qualification observation expired")?;
            let guard = self.0.lock().map_err(|_| "GPU qualification wake poisoned")?;
            let (guard, timed) = self
                .1
                .wait_timeout_while(
                    guard,
                    cadence.map_or(remaining, |cadence| remaining.min(cadence)),
                    |current| *current == revision,
                )
                .map_err(|_| "GPU qualification wake poisoned")?;
            drop(guard);
            if timed.timed_out() && Instant::now() >= cutoff {
                return observe()?.ok_or_else(|| "GPU qualification observation expired".into());
            }
        }
    }
}
struct Report {
    adapters: Vec<AdapterDescriptor>,
    selected: Option<AdapterDescriptor>,
    device: Option<Arc<GpuDevice>>,
    frames: usize,
    _memory: MemoryClaim,
}
fn main() -> Result {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let selected = match arguments.as_slice() {
        [] => None,
        [flag] if flag == "--list" => None,
        [flag, index] if flag == "--adapter" => Some(index.parse::<AdapterKey>()?),
        _ => {
            return Err("Use frame_qualification --list | --adapter <backend:vendor:device>".into());
        }
    };
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(MonotonicClock::new(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let deadline = client.now()?.checked_add(Duration::from_secs(60))?;
    let result = (|| -> Result {
        let mut job = client.submit_async(
            &scope,
            JobSpec {
                id: JobId::new(1)?,
                operation: Operation::Infrastructure,
                class: JobClass::IoOrdinary,
                deadline: Some(deadline),
                reservation_bytes: if selected.is_some() { 160 * 1024 * 1024 } else { 1024 * 1024 },
                replacement: None,
            },
            move |context| async move {
                let metadata = context.memory().claim(256 * 1024).map_err(io::Error::other)?;
                let catalog = AdapterCatalog::discover(&context).await?;
                let mut adapters = Vec::new();
                adapters.try_reserve_exact(catalog.adapters().len()).map_err(io::Error::other)?;
                adapters.extend(catalog.adapters().cloned());
                let chosen = if let Some(key) = selected {
                    let id = catalog.resolve(key)?;
                    let device = catalog.select(id, context.memory(), &context).await?;
                    Some(device)
                } else {
                    None
                };
                let selected = chosen.as_ref().map(|device| device.descriptor().clone());
                Ok::<Report, Box<dyn Error + Send + Sync>>(Report {
                    adapters,
                    selected,
                    device: chosen,
                    frames: 0,
                    _memory: metadata,
                })
            },
        )?;
        let JobCompletion { outcome, .. } = wake.until(None, || {
            client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
            Ok(job.try_take_completion())
        })?;
        let mut report = match outcome {
            JobOutcome::Success(report) => report,
            JobOutcome::Failure(cause) => return Err(cause),
            other => return Err(format!("GPU qualification outcome: {:?}", other.kind()).into()),
        };
        drop(job);
        if report.device.is_some() {
            let mut input = Some(report);
            let mut transfer = client.submit_blocking_owned_input(
                &scope,
                JobSpec {
                    id: JobId::new(2)?,
                    operation: Operation::Infrastructure,
                    class: JobClass::InfrastructureOrdinary,
                    deadline: Some(deadline),
                    reservation_bytes: 32 * 1024 * 1024,
                    replacement: None,
                },
                &mut input,
                |mut report, context| {
                    let device = report
                        .device
                        .take()
                        .ok_or_else(|| io::Error::other("Selected GPU custody absent"))?;
                    report.frames = scene::qualify(device, &context)?;
                    Ok::<Report, Box<dyn Error + Send + Sync>>(report)
                },
            )?;
            let JobCompletion { outcome, .. } = wake.until(None, || {
                client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
                Ok(transfer.try_take_completion())
            })?;
            report = match outcome {
                JobOutcome::Success(report) => report,
                JobOutcome::Failure(cause) => return Err(cause),
                other => return Err(format!("GPU transfer outcome: {:?}", other.kind()).into()),
            };
            drop(transfer);
        }
        for (index, adapter) in report.adapters.iter().enumerate() {
            println!(
                "{index}: key={} backend={:?} class={:?} vendor={:04x} device={:04x} compatible={} name={}",
                adapter.key(),
                adapter.backend,
                adapter.class,
                adapter.vendor,
                adapter.device,
                adapter.compatible,
                adapter.name
            );
        }
        if let Some(adapter) = &report.selected {
            println!(
                "SELECTED: backend={:?} class={:?} vendor={:04x} device={:04x} name={}",
                adapter.backend, adapter.class, adapter.vendor, adapter.device, adapter.name
            );
        }
        println!("CANVAS: qualified_frames={}", report.frames);
        drop(report);
        if scope.snapshot().reserved_bytes != 0 {
            return Err("GPU qualification retained admission after resource retirement".into());
        }
        Ok(())
    })();
    eprintln!("Qualification work returned; scope={:?}", scope.snapshot());
    scope.cancel();
    owner.request_shutdown(client.now()?.checked_add(Duration::from_secs(5))?);
    eprintln!("Qualification execution shutdown requested");
    let mut shutdown_observations = 0;
    let stopped = wake.until(Some(Duration::from_millis(16)), || {
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        let report = owner.poll_shutdown()?;
        if shutdown_observations < 3 {
            eprintln!("Qualification shutdown: {report:?}");
        }
        shutdown_observations += 1;
        Ok(report.settled.then_some(()))
    });
    let finished = stopped.and_then(|()| owner.finish_shutdown().map_err(Box::<dyn Error>::from));
    match (result, finished) {
        (Err(primary), Err(retirement)) => {
            Err(format!("{primary}; GPU qualification retirement: {retirement}").into())
        }
        (Err(cause), _) | (_, Err(cause)) => Err(cause),
        _ => Ok(()),
    }
}
