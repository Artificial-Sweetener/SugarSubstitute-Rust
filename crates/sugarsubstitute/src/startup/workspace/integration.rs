// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose session workers, authenticated grant facts, and existing native input ownership.
use super::*;
impl<B: StartupBackend> StartupHost<B> {
    pub(in crate::startup) fn advance_workspace(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(workspace) = &mut self.workspace else {
            return Ok(());
        };
        let now = self.execution.now()?;
        workspace.advance(now)?;
        if let Some((effect, bootstrap, placement, input, source)) = workspace.take_ready() {
            if let Some(canvas) = &mut self.canvas {
                workspace.input_plan = canvas.select_input_directory(input, source)?;
            }
            self.child.configure_workspace(bootstrap, placement)?;
            self.child
                .apply(effect, self.startup.as_mut().ok_or("Actual startup custody absent")?)?;
        }
        workspace.input_changed(
            self.canvas.as_ref().and_then(canvas::EmbeddedCanvas::workspace_input_generation),
            now,
        )?;
        if let Some(plan) = workspace.input_association.take() {
            self.canvas
                .as_mut()
                .ok_or("Native input association owner absent")?
                .resolve_input_association(plan.clone())?;
            workspace.input_plan = plan;
        }
        if workspace.wants_inputs()
            && let Some(startup) = &self.startup
            && let Some(backend) = self.child.workspace_context(startup)?
            && let Some(native) = self
                .canvas
                .as_ref()
                .map(canvas::EmbeddedCanvas::workspace_inputs)
                .transpose()?
                .flatten()
        {
            workspace.offer_inputs(native, backend)?;
        }
        workspace.submit_publication(now)?;
        #[cfg(feature = "native-composition")]
        self.advance_native_output_replays()?;
        #[cfg(feature = "native-composition")]
        for _ in 0..8 {
            let Some(observed) = self.workspace.as_mut().and_then(WorkspaceWork::take_surface)
            else {
                break;
            };
            self.route(policy::StartupRoute::Native(DesktopStartupObservation::SurfaceMessage(
                observed,
            )))?;
        }
        Ok(())
    }
}
