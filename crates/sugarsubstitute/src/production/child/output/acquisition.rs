// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt the original provisional source through Grant without replacing custody.
use super::*;
impl ChildOutput {
    /// Accept only this live constructor/source; existing pulls retain their original cutoff.
    pub(super) fn grant(&mut self, binding: OutputBinding) -> Result<()> {
        if self.closed
            || self.binding != Some(binding)
            || self.acquisition_until.is_none()
            || (self.job.is_none()
                && !self
                    .terminal
                    .as_ref()
                    .is_some_and(|terminal| matches!(terminal.outcome, JobOutcome::Success(_))))
        {
            return Err("Granted output source differs or original constructor is absent".into());
        }
        self.acquisition_until = None;
        Ok(())
    }
}
impl ProductionChild {
    /// Adopt one data-only source from the exact acquisition; Grant releases only the
    /// provisional pull cutoff and never replaces its transcript, constructor or ACK.
    pub(in super::super) fn adopt_output(
        &mut self,
        lease: sugarsubstitute_supervision::lifecycle_protocol::LeaseReference,
        original: Moment,
        granted: bool,
    ) -> Result<()> {
        if !self.connection_budget.is_some_and(|budget| {
            budget.kind() == sugarsubstitute_application::startup::ConnectionBudgetKind::Managed
        }) {
            if !granted {
                return Err("Attached acquisition cannot reserve captured output".into());
            }
            return Ok(());
        }
        let binding = OutputBinding {
            resource: ResourceKey {
                owner: self.run.owner,
                id: lease.resource,
                generation: lease.generation,
            },
            run: self.run,
            connection: self
                .acquire
                .ok_or("Original acquisition missing")?
                .header
                .connection
                .ok_or("Acquisition connection missing")?,
            lease,
        };
        if let Some(current) = self.output.binding {
            if !granted || current != binding {
                return Err("Output acquisition progress duplicated or changed".into());
            }
        } else {
            let id = self.next_job()?;
            self.output.begin(
                self.client.as_ref().ok_or("Actual child client absent")?,
                self.bootstrap.scope().ok_or("Actual child scope absent")?,
                id,
                binding,
                original,
            )?;
        }
        if granted {
            self.output.grant(binding)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::production::child::configuration::tests::Rig;
    type TestResult = std::result::Result<(), Box<dyn Error + Send + Sync>>;
    fn binding() -> TestResultBinding {
        let run = RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([4; 32]),
                epoch: CoordinatorEpoch::new([5; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        };
        let connection = ConnectionId::new(1)?;
        let mut coordinator =
            sugarsubstitute_application::supervision::ResourceCoordinator::new(run.owner);
        coordinator.register_run(ClientRegistration {
            run,
            connection,
            launch: LaunchId::new(1)?,
            profile: sugarsubstitute_contracts::configuration::ProfileId::new("Output_1".into())?,
        })?;
        let root = std::env::current_dir()?;
        let transition = coordinator.request(
            ClientBinding { run, connection },
            ResourceRequest {
                run,
                connection,
                id: RequestId::new(1)?,
                command: ResourceCommand::Acquire(BackendTarget::Managed(
                    sugarsubstitute_contracts::configuration::LaunchConfiguration::new(
                        root.clone(),
                        root.join("inert-python.exe"),
                        vec![],
                        8188,
                    )?,
                )),
            },
        )?;
        let RequestOutcome::Pending(lease) =
            transition.replies.first().ok_or("Actual Pending absent")?.outcome
        else {
            return Err("Not Pending".into());
        };
        Ok(OutputBinding {
            run,
            connection,
            resource: lease.resource,
            lease: LeaseReference {
                resource: lease.resource.id,
                generation: lease.resource.generation,
                lease: lease.id,
            },
        })
    }
    type TestResultBinding = std::result::Result<OutputBinding, Box<dyn Error + Send + Sync>>;
    #[test]
    fn grant_adopts_original_real_output_constructor_without_second_admission_or_budget()
    -> TestResult {
        let rig = Rig::new()?;
        let binding = binding()?;
        let original = Moment::from_nanos(100);
        let mut output = ChildOutput::new();
        output
            .begin(&rig.client, &rig.scope, JobId::new(1)?, binding, original)
            .map_err(|cause| cause.to_string())?;
        let spec = output.job.as_ref().ok_or("Actual constructor absent")?.spec();
        assert_eq!(spec.deadline, Some(original));
        assert_eq!(spec.reservation_bytes, crate::production_output::SOURCE_BYTES);
        let reserved = rig.scope.snapshot().reserved_bytes;
        output.grant(binding).map_err(|cause| cause.to_string())?;
        assert_eq!(output.job.as_ref().ok_or("Original job replaced")?.spec(), spec);
        assert_eq!(output.binding, Some(binding));
        assert_eq!(output.acquisition_until, None);
        assert_eq!(rig.scope.snapshot().reserved_bytes, reserved);
        assert!(output.grant(binding).is_err());
        assert!(output.begin(&rig.client, &rig.scope, JobId::new(2)?, binding, original).is_err());
        let completed = rig.completion(output.job.as_mut().ok_or("Original job absent")?)?;
        assert!(matches!(completed.outcome, JobOutcome::Success(_)));
        drop((completed, output, rig.owner));
        assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
        Ok(())
    }
    #[test]
    fn closed_or_foreign_grant_keeps_provisional_constructor_and_original_cutoff() -> TestResult {
        let rig = Rig::new()?;
        let binding = binding()?;
        let original = Moment::from_nanos(100);
        let mut output = ChildOutput::new();
        output
            .begin(&rig.client, &rig.scope, JobId::new(1)?, binding, original)
            .map_err(|cause| cause.to_string())?;
        let mut foreign = binding;
        foreign.connection = ConnectionId::new(2)?;
        assert!(output.grant(foreign).is_err());
        assert_eq!(output.acquisition_until, Some(original));
        output.close();
        assert!(output.grant(binding).is_err());
        assert!(output.pending());
        assert_eq!(output.job.as_ref().ok_or("Real job lost")?.spec().deadline, Some(original));
        let completed = rig.completion(output.job.as_mut().ok_or("Real job lost")?)?;
        assert!(matches!(
            completed.outcome,
            JobOutcome::Success(_)
                | JobOutcome::Cancelled
                | JobOutcome::Failure(OutputError::Cancelled)
        ));
        drop((completed, output, rig.owner));
        assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
        Ok(())
    }
}
