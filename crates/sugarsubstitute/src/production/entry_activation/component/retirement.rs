// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire the real component family, exact profile and original helper before capture.
use super::*;
use sugarsubstitute_contracts::lifecycle::ShutdownId;
use sugarsubstitute_supervision::{
    RegistryAdmissionError, RegistryObservation, profiles::PROFILE_OPERATION_BYTES,
};
impl ProfileComponent {
    pub(super) fn retire_profile(&mut self, resources: &mut EntryResources<'_>) -> Result<()> {
        let budget = self.shutdown.ok_or("Original component cleanup budget absent")?;
        let Some(campaign) = &mut self.campaign else { return Ok(()) };
        let Some(profile) = &mut campaign.profile else { return Ok(()) };
        if let Some(ticket) = campaign.ticket.filter(|_| !campaign.registry_retired) {
            let registry =
                resources.authority.registry_mut().ok_or("Actual component registry absent")?;
            registry.begin_stop(ticket.process(), ShutdownId::new(1)?, budget)?;
            let actual = registry.snapshot(ticket.process())?;
            if actual.cleanup_step_due(resources.client.now()?) {
                let member = if actual.member_watch_required {
                    Some(Self::spec(resources, JobClass::NativeObservation, 4096, None)?)
                } else {
                    None
                };
                let spec = Self::spec(
                    resources,
                    JobClass::InfrastructureLifecycle,
                    4096,
                    Some(budget.settle_until),
                )?;
                match resources
                    .authority
                    .registry_mut()
                    .ok_or("Actual component registry absent")?
                    .submit_cleanup_step(
                        resources.client,
                        ticket.process(),
                        resources.cleanup,
                        spec,
                        member,
                    ) {
                    Ok(_) => {}
                    Err(RegistryAdmissionError::Busy) => {}
                    Err(cause) => return Err(Box::new(cause)),
                }
            }
            let registry =
                resources.authority.registry_mut().ok_or("Actual component registry absent")?;
            if !registry.snapshot(ticket.process())?.settled {
                return Ok(());
            }
            while let Some(observation) = registry.take_observation(ticket.process())? {
                if let RegistryObservation::Failed { error, .. } = observation {
                    return Err(Box::new(error));
                }
            }
        } else if campaign.ticket.is_none()
            && !profile.retire_prepared_family(
                resources.authority.registry_mut().ok_or("Actual component registry absent")?,
            )?
        {
            return Ok(());
        }
        let snapshot = profile.snapshot();
        if !snapshot.retirement_ready() {
            if !snapshot.pending && !profile.custody_release_pending() {
                let spec = Self::spec(
                    resources,
                    JobClass::InfrastructureLifecycle,
                    PROFILE_OPERATION_BYTES,
                    Some(budget.settle_until),
                )?;
                profile.submit_release(
                    resources.client,
                    resources.cleanup,
                    spec,
                    resources.authority.registry().ok_or("Actual component registry absent")?,
                )?;
            }
            return Ok(());
        }
        if !campaign.released {
            campaign.released = true;
            tracing::info!(operation="profile_component_released",run=?self.run,"Exact profile handles and genuine helper ProfileReleased ACK retired");
        }
        if !campaign.registry_retired {
            if let Some(ticket) = campaign.ticket {
                resources
                    .authority
                    .registry_mut()
                    .ok_or("Actual component registry absent")?
                    .retire_settled(ticket.process())?;
            }
            campaign.registry_retired = true;
        }
        self.complete =
            campaign.started && campaign.exited && campaign.replied && campaign.released;
        Ok(())
    }
    pub(super) fn retire(&mut self, resources: &mut EntryResources<'_>) -> Result<bool> {
        if self.projection.is_some() {
            return Ok(false);
        }
        let budget = self.shutdown.ok_or("Original component cleanup budget absent")?;
        if let Some(done) = &mut self.projected {
            projection::return_unused_source(&mut done.outcome);
        }
        self.retire_profile(resources)?;
        if !self.profile_retirement_ready() {
            return Ok(false);
        }
        if let Some(campaign) = &mut self.campaign {
            projection::return_unused_source(&mut campaign.projection.outcome);
            if let Some(custody) = &mut campaign.custody {
                custody
                    .close(
                        budget,
                        &mut super::super::progress::backend_resources(
                            resources,
                            self.capture.as_ref(),
                        ),
                    )
                    .map_err(super::super::outer)?;
                if !custody.settled() {
                    return Ok(false);
                }
            }
        }
        let capture_closed = self
            .capture
            .as_mut()
            .map(|capture| capture.close(budget, resources))
            .transpose()?
            .unwrap_or(true);
        if capture_closed && self.failure.is_some() {
            self.complete = true;
        }
        Ok(capture_closed)
    }
}
