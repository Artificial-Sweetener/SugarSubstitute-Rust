// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Cancel real admitted preparation after a source upload was queued and observe retirement.
use super::{Result, Wake};
use std::{
    error::Error,
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::ExecutionClient;
use sugarsubstitute_image_gpu::{GpuDevice, GpuEvaluator};
use sugarsubstitute_image_ops::{
    AssetId, Extent, Graph, ImageError, Operation as ImageOperation, Pixel, RasterSource, Region,
};

struct CancellingSource {
    scope: Arc<Scope>,
    reads: usize,
}
impl RasterSource for CancellingSource {
    fn read(
        &mut self,
        _: AssetId,
        _: Region,
        output: &mut [Pixel],
        _: &JobContext,
    ) -> std::result::Result<(), ImageError> {
        if self.reads == 1 {
            self.scope.cancel();
        }
        self.reads += 1;
        output.fill(Pixel::linear(0.3, 0.2, 0.1, 0.5)?);
        Ok(())
    }
}
pub(super) fn qualify(
    client: &ExecutionClient,
    parent: &Scope,
    device: Arc<GpuDevice>,
    wake: &Wake,
) -> Result {
    let scope = Arc::new(
        client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, Some(parent))?,
    );
    let checked = Arc::new(AtomicBool::new(false));
    let mut input = Some((device, scope.clone(), checked.clone()));
    let mut job = client.submit_blocking_owned_input(
        &scope,
        JobSpec {
            id: JobId::new(3)?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(client.now()?.checked_add(Duration::from_secs(10))?),
            reservation_bytes: 8 * 1024 * 1024,
            replacement: None,
        },
        &mut input,
        |(device, scope, checked), context| {
            let extent = Extent::new(256, 128)?;
            let mut graph = Graph::new(1, context.memory())?;
            let output = graph.add(ImageOperation::Raster { asset: AssetId([23; 32]), extent })?;
            let mut gpu = GpuEvaluator::new(
                graph,
                CancellingSource { scope, reads: 0 },
                4,
                device,
                context.memory(),
                &context,
            )?;
            let regions = [Region::new(0, 0, 128, 128)?, Region::new(128, 0, 128, 128)?];
            let cause = gpu
                .regions(output, &regions, &context)
                .err()
                .ok_or("Cancelled GPU work was published")?;
            let mut current: Option<&(dyn Error + 'static)> = Some(&cause);
            let mut cancelled = false;
            while let Some(cause) = current {
                cancelled |=
                    matches!(cause.downcast_ref::<ImageError>(), Some(ImageError::Cancelled));
                current = cause.source();
            }
            let retired = context.cancellation_requested()
                && cancelled
                && gpu.work().batches == 1
                && gpu.stats().resident_tiles == 0;
            drop(gpu);
            checked.store(retired, Ordering::Release);
            Ok::<(), Box<dyn Error + Send + Sync>>(())
        },
    )?;
    let JobCompletion { outcome, .. } = wake.until(None, || {
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        Ok(job.try_take_completion())
    })?;
    if !matches!(outcome, JobOutcome::Cancelled) || !checked.load(Ordering::Acquire) {
        return Err("GPU cancellation did not observe unpublished, retired native work".into());
    }
    drop(job);
    if scope.snapshot().reserved_bytes != 0 {
        return Err("Cancelled GPU work retained its original job admission".into());
    }
    drop(scope);
    println!(
        "CANCELLATION: queued_upload_retired=true unpublished=true original_job_admission_released=true"
    );
    Ok(())
}
