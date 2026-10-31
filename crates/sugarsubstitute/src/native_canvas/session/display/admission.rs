// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit software display steps and native GPU preparation through their execution-owned lanes.
use super::*;
impl Session {
    /// Native queue/acquisition waits use blocking admission; CPU projection stays cooperative.
    pub(in crate::native_canvas::session) fn display_spec(
        &self,
        target: FrameStamp,
        native: bool,
    ) -> Result<JobSpec, Box<dyn Error>> {
        self.spec(
            if native { Operation::Infrastructure } else { Operation::FramePrepare },
            if native { JobClass::InfrastructureOrdinary } else { JobClass::CpuInteractive },
            2 * 1024 * 1024,
            Duration::from_secs(30),
            if native {
                None
            } else {
                Some(ReplacementKey { surface: SurfaceId::new(2)?, revision: target.generation })
            },
        )
    }
    /// Consume only the demand admitted with this frame. Later exposure or pointer activity
    /// remains pending while preparation and native queue acceptance retain their original slot.
    pub(super) fn admit_display(
        &mut self,
        input: work::DrawInput,
        mode: PresentationMode,
        input_at: Duration,
        now: Duration,
    ) -> Result<(), Box<dyn Error>> {
        let target = input.target;
        let document = input.document;
        let source_revision = input.source_revision;
        let native = matches!(input.source, work::DrawSource::Gpu { .. });
        let spec = self.display_spec(target, native)?;
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        let mut input = Some(input);
        let submitted = if native {
            runtime.client.submit_blocking_owned_input(
                &runtime.scope,
                spec,
                &mut input,
                work::DrawInput::prepare_gpu,
            )
        } else {
            runtime
                .client
                .submit_cpu_owned_input(&runtime.scope, spec, &mut input, |input, _| input)
        };
        if let Some(input) = input
            && let work::DrawSource::Cpu { storage: Some(storage), .. } = input.source
        {
            self.recycle_buffer(storage)?;
        }
        match submitted {
            Ok(job) => {
                self.pacing.submitted(target, mode, input_at, now)?;
                self.displaying = Some(Displaying { target, document, source_revision, job });
                self.frame_revision = target.generation.get();
                self.expose_pending = false;
            }
            Err(sugarsubstitute_execution::OwnedInputError::Admission(cause))
                if activity::retryable(&cause) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }
}
