// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe command finalization, upload and queue submission independently of native retirement.
use crate::{GpuDevice, buffer::Buffer, timing::Phase};
impl GpuDevice {
    /// Finalize at most one command buffer before measuring the driver queue entry separately.
    pub(crate) fn submit(&self, commands: Option<wgpu::CommandEncoder>) -> wgpu::SubmissionIndex {
        let commands = commands.map(|commands| {
            let _timing = self.timings.begin(Phase::Encode, 0, 1);
            commands.finish()
        });
        let _timing = self.timings.begin(Phase::Submit, 0, u64::from(commands.is_some()));
        self.queue.submit(commands)
    }
    /// Observe CPU-to-driver upload wall time and bytes without forcing native completion.
    pub(crate) fn write_buffer(&self, buffer: &Buffer, bytes: &[u8]) {
        let _timing = self.timings.begin(Phase::Upload, bytes.len() as u64, 1);
        self.queue.write_buffer(&buffer.raw, 0, bytes);
    }
}
