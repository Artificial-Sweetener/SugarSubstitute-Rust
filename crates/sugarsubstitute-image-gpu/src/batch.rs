// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound native command accumulation and retain original charges through submission and validation.
use crate::{
    GpuDevice, GpuError,
    buffer::Buffer,
    coordination::SubmissionPermit,
    transfer::{Mapping, Transfer},
};
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};

const MAX_STEPS: usize = 64;
const MAX_BUFFERS: usize = MAX_STEPS * 6;
const MAX_UPLOADS: usize = MAX_STEPS * 2;
const UPLOAD_BYTES: u64 = 8 * 1024 * 1024;
const METADATA_BYTES: u64 = MAX_STEPS as u64 * 32 * 1024;
const READY: u32 = 4;

/// Actual native work, independent of logical graph tile reuse.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GpuWork {
    /// Completed command batches; each uses one native submission and one validation mapping.
    pub batches: u64,
    /// Image operation dispatches, including both separable reconstruction passes.
    pub dispatches: u64,
    /// Host bytes copied into native upload staging.
    pub uploaded_bytes: u64,
    /// Bounded halo-copy groups submitted between immutable image tiles.
    pub dependency_copies: u64,
}
impl GpuWork {
    pub(crate) fn add(&mut self, other: Self) {
        self.batches = self.batches.saturating_add(other.batches);
        self.dispatches = self.dispatches.saturating_add(other.dispatches);
        self.uploaded_bytes = self.uploaded_bytes.saturating_add(other.uploaded_bytes);
        self.dependency_copies = self.dependency_copies.saturating_add(other.dependency_copies);
    }
}
/// Alpha observed on CPU sources or resolved from the exact GPU dispatch that produced a tile.
#[derive(Clone)]
pub(crate) enum Coverage {
    Known(bool),
    Pending(Arc<AtomicU32>),
}
impl Coverage {
    /// Exact zero and safely normal coverage need no alpha readback; tiny products stay pending.
    pub(crate) fn bounded_maximum(alpha: f32) -> Option<Self> {
        if alpha == 0.0 {
            Some(Self::Known(false))
        } else if alpha >= 4.0 * f32::MIN_POSITIVE {
            Some(Self::Known(true))
        } else {
            None
        }
    }
    pub(crate) fn get(&self) -> Result<Option<bool>, GpuError> {
        match self {
            Self::Known(value) => Ok(Some(*value)),
            Self::Pending(flags) => {
                let flags = flags.load(Ordering::Acquire);
                if flags & 1 != 0 {
                    return Err(GpuError::invalid(
                        "Image GPU operation produced invalid canonical pixels",
                    ));
                }
                Ok((flags & READY != 0).then_some(flags & 2 != 0))
            }
        }
    }
}
pub(crate) struct Batch {
    device: Arc<GpuDevice>,
    encoder: Option<wgpu::CommandEncoder>,
    pub(crate) status: Arc<Buffer>,
    receiving: Arc<Buffer>,
    buffers: Vec<Arc<Buffer>>,
    staging: Vec<MemoryClaim>,
    statuses: Vec<Arc<AtomicU32>>,
    steps: usize,
    work: GpuWork,
    _memory: MemoryClaim,
    submission: SubmissionPermit,
}
impl Batch {
    pub(crate) fn new(device: Arc<GpuDevice>, context: &JobContext) -> Result<Self, GpuError> {
        crate::adapter::check(context)?;
        let submission = device.submission(context)?;
        let memory = context
            .memory()
            .claim(METADATA_BYTES)
            .map_err(|cause| GpuError::new("gpu_batch_metadata", cause))?;
        let mut buffers = Vec::new();
        let mut staging = Vec::new();
        let mut statuses = Vec::new();
        buffers
            .try_reserve_exact(MAX_BUFFERS)
            .map_err(|cause| GpuError::new("gpu_batch_buffers", cause))?;
        staging
            .try_reserve_exact(MAX_UPLOADS)
            .map_err(|cause| GpuError::new("gpu_batch_staging", cause))?;
        statuses
            .try_reserve_exact(MAX_STEPS)
            .map_err(|cause| GpuError::new("gpu_batch_statuses", cause))?;
        let status = device.allocate(
            MAX_STEPS as u64 * 4,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            context,
        )?;
        let receiving = device.allocate(
            MAX_STEPS as u64 * 4,
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            context,
        )?;
        let encoder = device.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SugarSubstitute image DAG batch"),
        });
        Ok(Self {
            device,
            encoder: Some(encoder),
            status,
            receiving,
            buffers,
            staging,
            statuses,
            steps: 0,
            work: GpuWork::default(),
            _memory: memory,
            submission,
        })
    }
    pub(crate) fn full(&self) -> bool {
        self.steps == MAX_STEPS || self.work.uploaded_bytes >= UPLOAD_BYTES
    }
    pub(crate) fn start(&mut self) -> Result<(), GpuError> {
        if self.full() {
            return Err(GpuError::invalid("Image batch exceeds its bounded work inventory"));
        }
        self.steps += 1;
        Ok(())
    }
    pub(crate) fn commands(&mut self) -> Result<&mut wgpu::CommandEncoder, GpuError> {
        self.encoder.as_mut().ok_or_else(|| GpuError::invalid("Image batch already submitted"))
    }
    pub(crate) fn pin(&mut self, buffer: Arc<Buffer>) -> Result<(), GpuError> {
        if self.buffers.len() == MAX_BUFFERS {
            return Err(GpuError::invalid("Image batch resource inventory exhausted"));
        }
        self.buffers.push(buffer);
        Ok(())
    }
    pub(crate) fn upload(
        &mut self,
        bytes: &[u8],
        usage: wgpu::BufferUsages,
        context: &JobContext,
    ) -> Result<Arc<Buffer>, GpuError> {
        if self.staging.len() == MAX_UPLOADS {
            return Err(GpuError::invalid("Image batch upload inventory exhausted"));
        }
        let buffer = self.device.allocate(
            bytes.len() as u64,
            usage | wgpu::BufferUsages::COPY_DST,
            context,
        )?;
        let memory = context
            .memory()
            .claim(bytes.len() as u64)
            .map_err(|cause| GpuError::new("gpu_batch_upload_staging", cause))?;
        self.pin(buffer.clone())?;
        self.staging.push(memory);
        self.device.queue.write_buffer(&buffer.raw, 0, bytes);
        self.work.uploaded_bytes += bytes.len() as u64;
        Ok(buffer)
    }
    pub(crate) fn dispatch_status(&mut self) -> Result<(u32, Coverage), GpuError> {
        if self.statuses.len() == MAX_STEPS {
            return Err(GpuError::invalid("Image batch validation inventory exhausted"));
        }
        let flags = Arc::new(AtomicU32::new(0));
        let index = self.statuses.len() as u32;
        self.statuses.push(flags.clone());
        Ok((index, Coverage::Pending(flags)))
    }
    pub(crate) fn dispatched(&mut self) {
        self.work.dispatches += 1;
    }
    pub(crate) fn copied(&mut self) {
        self.work.dependency_copies += 1;
    }
    pub(crate) fn finish(mut self) -> Result<GpuWork, GpuError> {
        self.finish_inner()
    }
    fn finish_inner(&mut self) -> Result<GpuWork, GpuError> {
        let Some(mut commands) = self.encoder.take() else {
            return Ok(self.work);
        };
        commands.copy_buffer_to_buffer(
            &self.status.raw,
            0,
            &self.receiving.raw,
            0,
            MAX_STEPS as u64 * 4,
        );
        // Batch custody stays intact while the submission guard observes actual native completion.
        let mut submitted =
            Transfer::submit(self.device.clone(), commands, self.submission.clone());
        submitted.retire()?;
        self.work.batches = 1;
        let mut mapping = Mapping::new(self.receiving.clone(), self.submission.clone());
        mapping.wait()?;
        let mut invalid = false;
        let mut nonopaque = false;
        {
            let view = mapping.view()?;
            for (slot, bytes) in self.statuses.iter().zip(view.chunks_exact(4)) {
                let flags = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                slot.store(flags | READY, Ordering::Release);
                invalid |= flags & 1 != 0;
                nonopaque |= flags & 8 != 0;
            }
        }
        mapping.finish()?;
        if invalid {
            return Err(GpuError::invalid("Image GPU operation produced invalid canonical pixels"));
        }
        if nonopaque {
            return Err(GpuError::invalid("Display encoding requires an opaque composite"));
        }
        Ok(self.work)
    }
}
impl Drop for Batch {
    fn drop(&mut self) {
        if self.encoder.is_some()
            && let Err(cause) = self.finish_inner()
        {
            tracing::error!(operation="image_gpu_batch_retirement", cause=%cause, "Image batch retired after an unfinished preparation");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_coverage_requires_retirement_and_preserves_invalid_pixel_status()
    -> Result<(), GpuError> {
        assert_eq!(Coverage::Known(false).get()?, Some(false));
        let value = Arc::new(AtomicU32::new(0));
        let coverage = Coverage::Pending(value.clone());
        assert_eq!(coverage.get()?, None);
        value.store(READY, Ordering::Release);
        assert_eq!(coverage.get()?, Some(false));
        value.store(READY | 2, Ordering::Release);
        assert_eq!(coverage.get()?, Some(true));
        value.store(READY | 1, Ordering::Release);
        assert!(coverage.get().is_err());
        assert!(Coverage::bounded_maximum(f32::MIN_POSITIVE / 2.0).is_none());
        assert_eq!(
            Coverage::bounded_maximum(0.0)
                .ok_or_else(|| GpuError::invalid("Missing exact zero coverage"))?
                .get()?,
            Some(false)
        );
        assert_eq!(
            Coverage::bounded_maximum(0.5)
                .ok_or_else(|| GpuError::invalid("Missing normal coverage"))?
                .get()?,
            Some(true)
        );
        Ok(())
    }
}
