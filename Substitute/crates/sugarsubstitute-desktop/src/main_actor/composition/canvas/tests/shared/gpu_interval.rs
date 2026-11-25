// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Time composition work on the GPU with D3D11 timestamp queries.

use super::TestResult;
use crate::canvas::CanvasGraphics;
use windows::Win32::{
    Foundation::S_OK,
    Graphics::Direct3D11::{
        D3D11_QUERY_DATA_TIMESTAMP_DISJOINT, D3D11_QUERY_DESC, D3D11_QUERY_TIMESTAMP,
        D3D11_QUERY_TIMESTAMP_DISJOINT, ID3D11Asynchronous, ID3D11DeviceContext, ID3D11Query,
    },
};
use windows::core::Interface;

/// GPU timestamp interval around UI-thread composition work on the immediate context.
pub(super) struct GpuInterval {
    context: ID3D11DeviceContext,
    disjoint: ID3D11Query,
    start: ID3D11Query,
    end: ID3D11Query,
}

impl GpuInterval {
    pub(super) fn begin(graphics: &CanvasGraphics) -> TestResult<Self> {
        let device = graphics.d3d().ok_or("Composition device absent")?;
        let query = |kind| -> TestResult<ID3D11Query> {
            let mut query = None;
            // SAFETY: The test thread owns this device query output slot.
            unsafe {
                device
                    .CreateQuery(&D3D11_QUERY_DESC { Query: kind, MiscFlags: 0 }, Some(&mut query))
            }?;
            Ok(query.ok_or("Timestamp query absent")?)
        };
        let interval = Self {
            // SAFETY: The immediate context is used only on this test thread.
            context: unsafe { device.GetImmediateContext() }?,
            disjoint: query(D3D11_QUERY_TIMESTAMP_DISJOINT)?,
            start: query(D3D11_QUERY_TIMESTAMP)?,
            end: query(D3D11_QUERY_TIMESTAMP)?,
        };
        // SAFETY: These queries belong to this context's device.
        unsafe {
            interval.context.Begin(&interval.disjoint);
            interval.context.End(&interval.start);
        }
        Ok(interval)
    }

    /// End the interval and spin on the non-blocking query results; no sleep is involved.
    ///
    /// The GPU reports each result or a device failure, which `?` preserves, so the spin has no
    /// wall-clock bound: a slow or contended GPU only makes the test slower.
    pub(super) fn finish(self) -> TestResult<f64> {
        // SAFETY: These queries belong to this context's device.
        unsafe {
            self.context.End(&self.end);
            self.context.End(&self.disjoint);
            self.context.Flush();
        }
        let mut disjoint = D3D11_QUERY_DATA_TIMESTAMP_DISJOINT::default();
        let (mut start, mut end) = (0_u64, 0_u64);
        for (query, data, size) in [
            (&self.disjoint, (&raw mut disjoint).cast(), size_of_val(&disjoint)),
            (&self.start, (&raw mut start).cast(), size_of::<u64>()),
            (&self.end, (&raw mut end).cast(), size_of::<u64>()),
        ] {
            let query: ID3D11Asynchronous = query.cast()?;
            loop {
                // SAFETY: `data` addresses a live result of exactly `size` bytes.
                let status = unsafe {
                    (self.context.vtable().GetData)(
                        self.context.as_raw(),
                        query.as_raw(),
                        data,
                        u32::try_from(size)?,
                        0,
                    )
                };
                if status == S_OK {
                    break;
                }
                status.ok()?;
                std::hint::spin_loop();
            }
        }
        if disjoint.Disjoint.as_bool() || disjoint.Frequency == 0 {
            return Err("GPU timestamps were disjoint".into());
        }
        #[allow(clippy::cast_precision_loss)]
        Ok(end.saturating_sub(start) as f64 * 1_000_000.0 / disjoint.Frequency as f64)
    }
}
