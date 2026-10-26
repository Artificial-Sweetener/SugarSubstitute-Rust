// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Consume a prepared swapchain image on an admitted worker while native input keeps pumping.

use crate::{
    DisplayReadback, DisplayReadbackWorkspace, GpuError,
    display_readback::read_texture_region_into_with_submission,
    surface_state::ReconfigureReason,
    window::{WindowFrame, WindowPresentation, accept_presentation},
};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_ops::{Extent, Region};

impl WindowFrame {
    /// Read a bounded region from a retained reference image under this frame's existing queue
    /// custody. This prevents qualification from re-entering a gate behind its own frame permit.
    /// # Errors
    /// Rejects images from another selected device, foreign workspaces and invalid regions;
    /// preserves original control and native causes.
    pub fn read_reference_region_into<'a>(
        &self,
        image: &crate::DisplayImage,
        region: Region,
        workspace: &'a mut DisplayReadbackWorkspace,
        context: &JobContext,
    ) -> Result<&'a [u32], GpuError> {
        if !std::sync::Arc::ptr_eq(image.device(), &self.owner.device) {
            return Err(GpuError::invalid(
                "Native window frame and reference image belong to different GPU devices",
            ));
        }
        image.read_region_into_with_submission(region, workspace, context, self.submission.clone())
    }

    /// Read a bounded region of the actual drawn swapchain image for native qualification.
    /// Call on an admitted worker before presenting; normal presentation performs no readback.
    /// # Errors
    /// Rejects unsupported swapchain capture and invalid regions; preserves original native causes.
    pub fn read_region(
        &self,
        region: Region,
        context: &JobContext,
    ) -> Result<DisplayReadback, GpuError> {
        if !self.owner.capture {
            return Err(GpuError::invalid(
                "Native swapchain does not support qualification readback",
            ));
        }
        let texture = self
            .texture
            .as_ref()
            .ok_or_else(|| GpuError::invalid("Native window frame was already consumed"))?;
        let extent = Extent::new(texture.texture.width(), texture.texture.height())
            .map_err(crate::evaluate::image_error)?;
        let mut workspace =
            DisplayReadbackWorkspace::for_region(&self.owner.device, extent, region, context)?;
        self.read_region_into(region, &mut workspace, context)?;
        Ok(workspace.into_readback(region))
    }

    /// Read a bounded region into previously admitted reusable selected-device storage.
    ///
    /// This performs no memory claim or vector growth. Allocate the workspace from the source
    /// display image before acquiring this frame so admission pressure cannot abandon the image.
    /// # Errors
    /// Rejects unsupported capture, foreign workspaces, consumed frames and invalid regions.
    pub fn read_region_into<'a>(
        &self,
        region: Region,
        workspace: &'a mut DisplayReadbackWorkspace,
        context: &JobContext,
    ) -> Result<&'a [u32], GpuError> {
        if !self.owner.capture {
            return Err(GpuError::invalid(
                "Native swapchain does not support qualification readback",
            ));
        }
        let texture = self
            .texture
            .as_ref()
            .ok_or_else(|| GpuError::invalid("Native window frame was already consumed"))?;
        let extent = Extent::new(texture.texture.width(), texture.texture.height())
            .map_err(crate::evaluate::image_error)?;
        read_texture_region_into_with_submission(
            &texture.texture,
            &self.owner.device,
            extent,
            region,
            workspace,
            context,
            self.submission.clone(),
        )?;
        let pixels = region.pixels() as usize;
        if self.owner.format == wgpu::TextureFormat::Rgba8Unorm {
            workspace.swap_red_blue(pixels);
        }
        Ok(workspace.pixels(pixels))
    }

    /// Present already-completed native GPU work on an admitted worker.
    ///
    /// wgpu acquires device locks and may call a blocking driver entry point here, even after
    /// rendering retired. Never call this from the native window/input thread. The original
    /// desktop peer remains retained by the frame's surface owner through completion.
    /// The returned receipt requests one follow-up acquisition after each configuration because
    /// wgpu does not expose recoverable present status through its public queue API.
    /// # Errors
    /// Preserves original control/device failures. Once native presentation starts, cancellation
    /// cannot establish retirement; the owning execution job must observe the call returning.
    pub fn present(mut self, context: &JobContext) -> Result<WindowPresentation, GpuError> {
        crate::adapter::check(context)?;
        self.owner.device.faults.check()?;
        let texture = self
            .texture
            .take()
            .ok_or_else(|| GpuError::invalid("Native window frame was already consumed"))?;
        self.owner.device.queue.present(texture);
        self.owner.device.faults.check()?;
        self.owner.reconfiguration.presented();
        Ok(accept_presentation(&self.owner.follow_up_after_configuration))
    }
}

impl Drop for WindowFrame {
    fn drop(&mut self) {
        if self.texture.is_some() {
            self.owner.reconfiguration.mark(ReconfigureReason::Discarded);
        }
    }
}
