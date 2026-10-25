// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire native submissions and mappings before releasing their original managed allocations.
use crate::{GpuDevice, GpuError, buffer::Buffer};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::execution::MemoryClaim;

/// Own both ends and any pending upload staging through actual native retirement.
pub(crate) struct Transfer {
    device: Arc<GpuDevice>,
    index: Option<wgpu::SubmissionIndex>,
    done: bool,
    _buffers: [Option<Arc<Buffer>>; 8],
    _staging: Option<MemoryClaim>,
}
impl Transfer {
    pub(crate) fn upload(buffer: Arc<Buffer>, staging: MemoryClaim, bytes: &[u8]) -> Self {
        let device = buffer.device.clone();
        let mut transfer = Self {
            device,
            index: None,
            done: false,
            _buffers: std::array::from_fn(|index| (index == 0).then(|| buffer.clone())),
            _staging: Some(staging),
        };
        transfer.device.queue.write_buffer(&buffer.raw, 0, bytes);
        transfer.index = Some(transfer.device.queue.submit([]));
        transfer
    }
    pub(crate) fn copy(source: Arc<Buffer>, receiving: Arc<Buffer>) -> Self {
        let device = source.device.clone();
        let mut transfer = Self {
            device,
            index: None,
            done: false,
            _buffers: std::array::from_fn(|index| match index {
                0 => Some(source.clone()),
                1 => Some(receiving.clone()),
                _ => None,
            }),
            _staging: None,
        };
        let mut commands =
            transfer.device.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SugarSubstitute image readback"),
            });
        commands.copy_buffer_to_buffer(&source.raw, 0, &receiving.raw, 0, source.bytes);
        transfer.index = Some(transfer.device.queue.submit([commands.finish()]));
        transfer
    }
    /// Retain command inputs, destinations and host-side native bookkeeping until the fence.
    pub(crate) fn commands(
        device: Arc<GpuDevice>,
        buffers: [Option<Arc<Buffer>>; 8],
        memory: MemoryClaim,
        commands: wgpu::CommandEncoder,
    ) -> Self {
        let mut transfer =
            Self { device, index: None, done: false, _buffers: buffers, _staging: Some(memory) };
        transfer.index = Some(transfer.device.queue.submit([commands.finish()]));
        transfer
    }
    /// Cancellation stops new image work; submitted native work keeps every guard until this ends.
    pub(crate) fn retire(&mut self) -> Result<(), GpuError> {
        if self.done {
            return self.device.faults.check();
        }
        if self.index.is_none() {
            self.index = Some(self.device.queue.submit([]));
        }
        let mut failure = None;
        loop {
            match self.device.device.poll(wgpu::PollType::Wait {
                submission_index: self.index.clone(),
                timeout: Some(Duration::from_millis(100)),
            }) {
                Ok(status) if status.wait_finished() => break,
                Ok(_) | Err(wgpu::PollError::Timeout) => {}
                Err(cause @ wgpu::PollError::WrongSubmissionIndex(..)) => {
                    if failure.is_none() {
                        failure = Some(GpuError::new("gpu_submission_identity", cause));
                    }
                    self.index = None;
                }
            }
        }
        self.done = true;
        match failure {
            Some(cause) => Err(cause),
            None => self.device.faults.check(),
        }
    }
}
impl Drop for Transfer {
    fn drop(&mut self) {
        if !self.done
            && let Err(cause) = self.retire()
        {
            tracing::error!(operation="image_gpu_transfer_retirement", cause=%cause, "GPU transfer retired with a native failure");
        }
    }
}
type MapResult = Result<(), wgpu::BufferAsyncError>;
/// Mapping callbacks also retain their receiving buffer until actual terminal observation.
pub(crate) struct Mapping {
    buffer: Arc<Buffer>,
    completion: Arc<Mutex<Option<MapResult>>>,
    done: bool,
    mapped: bool,
}
impl Mapping {
    pub(crate) fn new(buffer: Arc<Buffer>) -> Self {
        let completion = Arc::new(Mutex::new(None));
        let delivered = completion.clone();
        buffer.raw.map_async(wgpu::MapMode::Read, .., move |outcome| {
            *delivered.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(outcome);
        });
        Self { buffer, completion, done: false, mapped: false }
    }
    pub(crate) fn wait(&mut self) -> Result<(), GpuError> {
        let mut failure = None;
        while !self.done {
            let result =
                self.completion.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
            if let Some(result) = result {
                self.done = true;
                result.map_err(|cause| GpuError::new("gpu_buffer_mapping", cause))?;
                self.mapped = true;
                break;
            }
            match self.buffer.device.device.poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(Duration::from_millis(100)),
            }) {
                Ok(_) | Err(wgpu::PollError::Timeout) => {}
                Err(cause) => {
                    if failure.is_none() {
                        failure = Some(GpuError::new("gpu_mapping_observation", cause));
                    }
                }
            }
        }
        match failure {
            Some(cause) => Err(cause),
            None => self.buffer.device.faults.check(),
        }
    }
    pub(crate) fn view(&self) -> Result<wgpu::BufferView, GpuError> {
        self.buffer
            .raw
            .get_mapped_range(..)
            .map_err(|cause| GpuError::new("gpu_mapped_view", cause))
    }
    pub(crate) fn finish(&mut self) -> Result<(), GpuError> {
        let observed = self.wait();
        if self.mapped {
            self.buffer.raw.unmap();
            self.mapped = false;
        }
        observed.and_then(|()| self.buffer.device.faults.check())
    }
}
impl Drop for Mapping {
    fn drop(&mut self) {
        if let Err(cause) = self.finish() {
            tracing::error!(operation="image_gpu_mapping_retirement", cause=%cause, "GPU mapping retired with a native failure");
        }
    }
}
