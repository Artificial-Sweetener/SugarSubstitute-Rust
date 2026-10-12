// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Consume bounded actual metadata and progress one owned campaign decision per wake.
use super::{Result, parent::Parent, parent_protocol};
use std::{num::NonZeroUsize, time::Instant};
use sugarsubstitute_contracts::execution::{Clock, ExecutionPort, TerminalKind};
use sugarsubstitute_desktop::HeadlessStep;

impl Parent {
    pub(super) fn activity(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        if let Some(client) = &self.client {
            let batch = client.drain_completions(max)?;
            for case in &mut self.cases {
                if let Some(session) = &case.session {
                    case.terminal_notice.capture(
                        session.completion.scope_id(),
                        session.completion.spec().id,
                        &batch,
                    )?;
                }
            }
            let _table_changed = crate::process_fixture::registry::poll(
                &mut self.registry_creation,
                &mut self.registry,
                self.closing,
            )?;
            if let Some(registry) = &mut self.registry {
                registry.consume(&batch)?;
            }
            if self.registry.is_none() {
                if !self.deadline_observed && Instant::now() >= self.deadline {
                    self.deadline_observed = true;
                    return Err("Registry construction exceeded original qualifier deadline".into());
                }
                return Ok(if self.closing {
                    sugarsubstitute_desktop::HeadlessStep::Shutdown
                } else {
                    sugarsubstitute_desktop::HeadlessStep::Active
                });
            }
            for case in &mut self.cases {
                for notice in &batch.notifications {
                    if case.cleanup_job == Some(notice.job_id) {
                        case.cleanup_job = None;
                        if let Some(key) = case.process {
                            let snapshot = self
                                .registry
                                .as_ref()
                                .ok_or("Actual registry is not prepared")?
                                .snapshot(key)?;
                            case.cleanup_requested |= notice.kind == TerminalKind::Success
                                && snapshot.member_watch_required
                                && !snapshot.cleanup_pending
                                && !snapshot.settled;
                        }
                    }
                    if case.member_job == Some(notice.job_id) {
                        case.member_job = None;
                        case.cleanup_requested |= notice.kind == TerminalKind::Success;
                    }
                    if case.creation_job == Some(notice.job_id)
                        && notice.kind == TerminalKind::Success
                        && case.stop.is_some()
                    {
                        case.cleanup_requested = true;
                    }
                }
                if !case.pipe_terminal {
                    for _ in 0..8 {
                        let next = case
                            .session
                            .as_ref()
                            .and_then(|session| session.control.take_observation());
                        let Some(next) = next else { break };
                        parent_protocol::observe(case, next)?;
                    }
                    parent_protocol::consume(case)?;
                }
            }
        }
        if !self.deadline_observed && Instant::now() >= self.deadline {
            self.deadline_observed = true;
            return Err("Lifecycle qualification reached its original sixty-second deadline".into());
        }
        if self.current == self.cases.len() {
            return Ok(HeadlessStep::Shutdown);
        }
        let index = self.current;
        if !self.closing && self.cases[index].prepared && self.cases[index].process.is_none() {
            self.launch()?;
        }
        if let Some(key) = self.cases[index].process.filter(|_| !self.cases[index].retired) {
            for _ in 0..5 {
                let Some(observation) = self
                    .registry
                    .as_mut()
                    .ok_or("Actual registry is not prepared")?
                    .take_observation(key)?
                else {
                    break;
                };
                let root_exit = matches!(
                    &observation,
                    sugarsubstitute_supervision::RegistryObservation::RootExited { .. }
                );
                self.cases[index].cleanup_requested |= root_exit;
                self.cases[index].observe_registry(observation)?;
            }
            if self.cases[index].exited.is_some() || self.closing {
                self.ensure_stop()?;
            }
            let snapshot =
                self.registry.as_ref().ok_or("Actual registry is not prepared")?.snapshot(key)?;
            if let Some(stop) = &mut self.cases[index].stop {
                let (cleanup, expired) = stop.due(self.clock.now(), snapshot.settled);
                self.cases[index].cleanup_requested |= cleanup;
                if expired {
                    return Err("Lifecycle family remains owned after original fifteen-second settlement budget".into());
                }
            }
            if self.cases[index].can_retire(self.closing, snapshot.settled) {
                self.registry
                    .as_mut()
                    .ok_or("Actual registry is not prepared")?
                    .retire_settled(key)?;
                self.cases[index].retired = true;
            } else if self.cases[index].stop.is_some()
                && self.cases[index].cleanup_requested
                && self.cases[index].cleanup_job.is_none()
                && !snapshot.creation_pending
            {
                self.submit_cleanup(key, snapshot.member_watch_required)?;
            }
        }
        parent_protocol::advance(&mut self.cases[index], self.closing)?;
        if self.cases[index].retired && self.cases[index].pipe_terminal {
            if !self.closing && !self.cases[index].complete() {
                return Err("Lifecycle case lacks complete actual evidence".into());
            }
            self.cases[index].session = None;
            self.current += 1;
            if !self.closing {
                self.reserve_next()?;
            }
        }
        Ok(if self.closing || self.current == self.cases.len() {
            HeadlessStep::Shutdown
        } else {
            HeadlessStep::Active
        })
    }
}
