// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Return a vetoed allocation only after the real prepared callback quiesces.
use super::*;
impl Active {
    pub(super) fn write_revoked(&mut self, core: &Core) -> Result<(), LifecyclePipeFailure> {
        self.lease.as_mut().ok_or_else(|| failure("pipe_missing_lease"))?.quiesce(Role::Write)?;
        let write = self.write.take().ok_or_else(|| failure("pipe_write_owner"))?;
        {
            let mut state = lock(&core.state);
            let mut pipe = lock(&self.work.shared.state);
            if let Some(slots) = state
                .jobs
                .get_mut(&self.context.job_id())
                .and_then(|record| record.native_deadlines.as_mut())
            {
                slots.clear(Role::Write, write.command.operation.get());
            }
            pipe.write_busy = false;
            pipe.write_slots = 0;
            pipe.loans += 1;
            pipe.observations.push_back(LifecyclePipeObservation::WriteRevoked {
                operation: write.command.operation,
                buffer: LifecyclePipeWriteBuffer {
                    buffer: Some(write.buffer),
                    shared: self.work.shared.clone(),
                },
            });
        }
        core.changed();
        self.publish(core);
        Ok(())
    }
}
