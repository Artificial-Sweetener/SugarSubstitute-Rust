// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep exact families and original stop budgets through observed native closure.
use super::*;
impl Fixture {
    pub(super) fn family(&self, managed: bool) -> Option<&Family> {
        if managed {
            self.managed.as_ref()
        } else {
            self.child.as_ref().and_then(|case| case.family.as_ref())
        }
    }
    pub(super) fn family_mut(&mut self, managed: bool) -> Option<&mut Family> {
        if managed {
            self.managed.as_mut()
        } else {
            self.child.as_mut().and_then(|case| case.family.as_mut())
        }
    }
    pub(super) fn begin_stop(&mut self, managed: bool) -> Result<bool> {
        let Some(family) = self.family(managed) else { return Ok(false) };
        if family.stop.is_some() || family.progress.retired {
            return Ok(false);
        }
        let key = family.ticket.process();
        let plan = StopPlan::new(self.clock.now(), campaign_watch::native_now())?;
        let shutdown = ShutdownId::new(key.id.get())?;
        self.registry.as_mut().ok_or("Claim campaign registry missing")?.begin_stop(
            key,
            shutdown,
            plan.budget,
        )?;
        let family = self.family_mut(managed).ok_or("Claim campaign family missing")?;
        family.progress.stop = Some((shutdown, plan.budget));
        family.stop = Some(plan);
        family.cleanup_requested = true;
        Ok(true)
    }
    pub(super) fn family_step(&mut self, managed: bool, admit: bool) -> Result<bool> {
        let Some(family) = self.family(managed) else { return Ok(false) };
        if family.progress.retired {
            return Ok(false);
        }
        let key = family.ticket.process();
        let snapshot =
            self.registry.as_ref().ok_or("Claim campaign registry missing")?.snapshot(key)?;
        let now = self.clock.now();
        let mut expired = false;
        if let Some(family) = self.family_mut(managed)
            && let Some(stop) = &mut family.stop
        {
            let due = stop.due(now, snapshot.settled);
            family.cleanup_requested |= due.cleanup;
            expired = due.expired;
        }
        if expired {
            return Err("Claim family exceeded its original15-second settlement budget".into());
        }
        if snapshot.settled {
            return Ok(false);
        }
        let family = self.family(managed).ok_or("Claim campaign family missing")?;
        if !admit
            || family.stop.is_none()
            || !family.cleanup_requested
            || family.cleanup_job.is_some()
            || snapshot.creation_pending
            || snapshot.cleanup_step_pending
        {
            return Ok(false);
        }
        let watch = if snapshot.member_watch_required {
            Some(self.spec(JobClass::NativeObservation, 4096, false)?)
        } else {
            None
        };
        let spec = self.spec(JobClass::InfrastructureLifecycle, 4096, false)?;
        self.family_mut(managed).ok_or("Claim campaign family missing")?.cleanup_requested = false;
        let id =
            self.registry.as_mut().ok_or("Claim campaign registry missing")?.submit_cleanup_step(
                self.client.as_ref().ok_or("Claim campaign client missing")?,
                key,
                self.cleanup_scope.as_ref().ok_or("Claim campaign cleanup scope missing")?,
                spec,
                watch,
            )?;
        let family = self.family_mut(managed).ok_or("Claim campaign family missing")?;
        family.cleanup_job = Some(id);
        family.member_job = watch.map(|spec| spec.id);
        Ok(true)
    }
    /// Managed claim release precedes retirement of its actual settled registry proof.
    pub(super) fn retire_family(&mut self, managed: bool) -> Result<bool> {
        let Some(family) = self.family(managed) else { return Ok(false) };
        if family.progress.retired || !family.settlement_transferred {
            return Ok(false);
        }
        let key = family.ticket.process();
        let snapshot =
            self.registry.as_ref().ok_or("Claim campaign registry missing")?.snapshot(key)?;
        if !snapshot.settled || (managed && self.resource.is_some()) {
            return Ok(false);
        }
        self.registry.as_mut().ok_or("Claim campaign registry missing")?.retire_settled(key)?;
        self.family_mut(managed).ok_or("Claim campaign family missing")?.progress.retired = true;
        Ok(true)
    }
    pub(super) fn claims_cleanup(&mut self) -> Result<bool> {
        if !self.initialization_admitted || self.claims_pending || self.claims_closed {
            return Ok(false);
        }
        self.fallback_registry()?;
        if self.registry_creation.is_some() || self.registry.is_none() {
            return Ok(false);
        }
        let spec = self.spec(JobClass::InfrastructureLifecycle, CLAIM_OPERATION_BYTES, false)?;
        let client = self.client.as_ref().ok_or("Claim campaign client missing")?;
        let scope = self.cleanup_scope.as_ref().ok_or("Claim campaign cleanup scope missing")?;
        let result = if let Some(resource) = self.resource {
            self.claims
                .submit_backend_release(
                    client,
                    scope,
                    spec,
                    self.registry.as_ref().ok_or("Claim campaign registry missing")?,
                    resource,
                )
                .map(|_| ())
        } else if let Some(ticket) = &self.resolution {
            self.claims.submit_retire_resolution(client, scope, spec, ticket).map(|_| ())
        } else {
            self.claims.close_admission();
            self.claims
                .submit_close(
                    client,
                    scope,
                    spec,
                    self.registry.as_mut().ok_or("Claim campaign registry missing")?,
                )
                .map(|_| ())
        };
        match result {
            Ok(()) => {
                self.claims_pending = true;
                Ok(true)
            }
            Err(ClaimAdmissionError::Busy) => Ok(false),
            Err(cause) => Err(Box::new(cause)),
        }
    }
    pub(super) fn closing_step(&mut self) -> Result<bool> {
        let mut changed = false;
        if let Some(case) = &mut self.child
            && !case.event_retired
            && let Some(session) = &case.session
        {
            if case.ready && !case.abortive && !case.release_sent {
                session.control.request_release()?;
                case.release_sent = true;
                changed = true;
            } else if !case.release_sent && !case.event_cancel_requested {
                session.control.cancel();
                case.event_cancel_requested = true;
                changed = true;
            }
        }
        changed |= self.begin_stop(true)?;
        changed |= self.begin_stop(false)?;
        changed |= self.family_step(true, true)?;
        changed |= self.family_step(false, true)?;
        changed |= self.claims_cleanup()?;
        changed |= self.retire_family(true)?;
        changed |= self.retire_family(false)?;
        Ok(changed)
    }
}
