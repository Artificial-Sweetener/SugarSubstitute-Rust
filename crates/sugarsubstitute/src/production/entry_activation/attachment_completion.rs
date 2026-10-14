// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Move actual attachment outcomes directly into their already-funded desktop record.
use super::*;
impl ProductionActivation {
    // Keep the full launch outcome temporaries off the persistent activity frame.
    #[inline(never)]
    pub(super) fn adopt_attachment(
        record: &mut Record,
        resources: &mut EntryResources<'_>,
        closing: bool,
    ) -> Result<()> {
        if let Some(job) = &mut record.attach
            && let Some(completion) = job.try_take_completion()
        {
            record.attach = None;
            match completion.outcome {
                JobOutcome::Success(pin) if !closing => {
                    record.attached_memory = Some(completion.memory);
                    let projection =
                        record.projected.take().ok_or("Original projected completion missing")?;
                    record.start = Some(DesktopStart::new(
                        sugarsubstitute_contracts::startup::MainIdentity {
                            run: record.run,
                            launch: record.launch,
                            navigation: record.navigation,
                        },
                        std::mem::take(&mut record.qualification),
                        record.connection,
                        projection,
                        pin,
                        resources,
                    )?);
                }
                JobOutcome::Success(pin) => {
                    record.released_attach = Some(JobCompletion {
                        outcome: JobOutcome::Success(pin.release()),
                        scope_id: completion.scope_id,
                        spec: completion.spec,
                        queue_time: completion.queue_time,
                        execution_time: completion.execution_time,
                        memory: completion.memory,
                        native_handles: completion.native_handles,
                    });
                }
                outcome => {
                    record.attach_failure = Some(JobCompletion { outcome, ..completion });
                    if !closing {
                        return Err(
                            "Actual capture attachment failed with retained original outcome"
                                .into(),
                        );
                    }
                }
            }
        }
        Ok(())
    }
}
