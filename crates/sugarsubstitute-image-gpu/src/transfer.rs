// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire native submissions and mappings before releasing their original managed allocations.
use crate::{GpuDevice, GpuError, buffer::Buffer, coordination::SubmissionPermit};
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
    submission: Option<SubmissionPermit>,
}
impl Transfer {
    pub(crate) fn upload(
        buffer: Arc<Buffer>,
        staging: MemoryClaim,
        bytes: &[u8],
        commands: Option<wgpu::CommandEncoder>,
        submission: SubmissionPermit,
    ) -> Self {
        let device = buffer.device.clone();
        let mut transfer = Self {
            device,
            index: None,
            done: false,
            _buffers: std::array::from_fn(|index| (index == 0).then(|| buffer.clone())),
            _staging: Some(staging),
            submission: Some(submission),
        };
        transfer.device.write_buffer(&buffer, bytes);
        transfer.index = Some(transfer.device.submit(commands));
        transfer
    }
    pub(crate) fn copy(
        source: Arc<Buffer>,
        receiving: Arc<Buffer>,
        submission: SubmissionPermit,
    ) -> Self {
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
            submission: Some(submission),
        };
        let mut commands =
            transfer.device.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SugarSubstitute image readback"),
            });
        commands.copy_buffer_to_buffer(&source.raw, 0, &receiving.raw, 0, source.bytes);
        transfer.index = Some(transfer.device.submit(Some(commands)));
        transfer
    }
    /// Submit a command batch whose calling owner keeps every allocation and staging claim alive.
    /// The guard's blocking destructor retires work before that calling custody can be released.
    pub(crate) fn submit(
        device: Arc<GpuDevice>,
        commands: wgpu::CommandEncoder,
        submission: SubmissionPermit,
    ) -> Self {
        let mut transfer = Self {
            device,
            index: None,
            done: false,
            _buffers: std::array::from_fn(|_| None),
            _staging: None,
            submission: Some(submission),
        };
        transfer.index = Some(transfer.device.submit(Some(commands)));
        transfer
    }
    /// Cancellation stops new image work; submitted native work keeps every guard until this ends.
    pub(crate) fn retire(&mut self) -> Result<(), GpuError> {
        if self.done {
            return self.device.faults.check();
        }
        if self.index.is_none() {
            self.index = Some(self.device.submit(None));
        }
        let mut timing = self.device.timings.begin(crate::timing::Phase::Wait, 0, 1);
        let mut failure = None;
        loop {
            match self.device.device.poll(wgpu::PollType::Wait {
                submission_index: self.index.clone(),
                timeout: Some(Duration::from_millis(100)),
            }) {
                Ok(status) if status.wait_finished() => break,
                Ok(_) => {}
                Err(wgpu::PollError::Timeout) => timing.timeout(),
                Err(cause @ wgpu::PollError::WrongSubmissionIndex(..)) => {
                    if failure.is_none() {
                        failure = Some(GpuError::new("gpu_submission_identity", cause));
                    }
                    self.index = None;
                }
            }
        }
        self.done = true;
        self.submission.take();
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
    submission: Option<SubmissionPermit>,
}
impl Mapping {
    pub(crate) fn new(buffer: Arc<Buffer>, submission: SubmissionPermit) -> Self {
        let completion = Arc::new(Mutex::new(None));
        let delivered = completion.clone();
        let timing = buffer.device.timings.begin(crate::timing::Phase::MapRequest, 0, 1);
        buffer.raw.map_async(wgpu::MapMode::Read, .., move |outcome| {
            *delivered.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(outcome);
        });
        drop(timing);
        Self { buffer, completion, done: false, mapped: false, submission: Some(submission) }
    }
    pub(crate) fn wait(&mut self) -> Result<(), GpuError> {
        if self.done {
            return self.buffer.device.faults.check();
        }
        let mut timing =
            self.buffer.device.timings.begin(crate::timing::Phase::MapWait, self.buffer.bytes, 1);
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
                Ok(_) => {}
                Err(wgpu::PollError::Timeout) => timing.timeout(),
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
        self.submission.take();
        observed.and_then(|()| self.buffer.device.faults.check())
    }
}
impl Drop for Mapping {
    fn drop(&mut self) {
        if (!self.done || self.mapped)
            && let Err(cause) = self.finish()
        {
            tracing::error!(operation="image_gpu_mapping_retirement", cause=%cause, "GPU mapping retired with a native failure");
        }
    }
}
