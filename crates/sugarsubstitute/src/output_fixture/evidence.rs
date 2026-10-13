// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep independent stream, process, family and source-effect evidence bounded.
use super::{Result, channel::Channel, corpus, options::Role};
use std::{io, time::Instant};
use sugarsubstitute_application::transcript::{MAX_OWNER_ALLOCATION_BYTES, TranscriptStore};
use sugarsubstitute_contracts::{
    execution::{MemoryClaim, Moment, OwnedJob, TerminalKind},
    lifecycle::{ProcessKey, ResourceKey, RunKey, ShutdownBudget, ShutdownId},
    transcript::{OutputBatch, TranscriptDiagnostics},
};
use sugarsubstitute_execution::windows_output::{
    AdmittedOutputBatch, DesktopOutputBatch, OutputIdentity, WindowsOutputReport,
    WindowsOutputSession,
};
pub(super) const STORE_BYTES: u64 = MAX_OWNER_ALLOCATION_BYTES as u64 + 65536;
pub(super) struct Store {
    pub(super) transcript: TranscriptStore,
    pub(super) sequences: [u64; 2],
    pub(super) delivered: [u64; 2],
    pub(super) gaps: [u64; 2],
    pub(super) eof: [bool; 2],
    pub(super) raw: [Vec<u8>; 2],
    _claim: MemoryClaim,
}
impl Store {
    pub(super) fn new(resource: ResourceKey, claim: MemoryClaim) -> Self {
        Self {
            transcript: TranscriptStore::new(resource),
            sequences: [0; 2],
            delivered: [0; 2],
            gaps: [0; 2],
            eof: [false; 2],
            raw: [Vec::with_capacity(256), Vec::with_capacity(256)],
            _claim: claim,
        }
    }
    /// Qualify Domain parsing separately under this fixture's original Store claim.
    /// The native source remains Desktop(run); this bounded parser input uses the
    /// independently assigned test resource and is never process/backend authority.
    /// The unchanged extra 64KiB covers this one 32KiB copy and the bounded raw corpus.
    pub(super) fn apply_desktop(&mut self, role: Role, batch: &DesktopOutputBatch) -> Result<()> {
        let input = OutputBatch::new(
            self.transcript.resource(),
            batch.stream(),
            batch.sequence(),
            batch.bytes(),
            batch.gap(),
            batch.end(),
        )?;
        self.apply(role, &input)
    }
    pub(super) fn apply(&mut self, role: Role, batch: &OutputBatch) -> Result<()> {
        let i = batch.stream().index();
        if self.eof[i]
            || batch.sequence().get()
                != self.sequences[i]
                    .checked_add(1)
                    .and_then(|n| n.checked_add(batch.gap().map_or(0, |gap| gap.skipped_batches())))
                    .ok_or("Output evidence sequence overflow")?
        {
            return Err("Output sequence/gap/EOF differs".into());
        }
        self.transcript.apply(batch)?;
        self.sequences[i] = batch.sequence().get();
        self.delivered[i] = self.delivered[i]
            .checked_add(batch.bytes().len() as u64)
            .ok_or("Output byte evidence overflow")?;
        self.gaps[i] = self.gaps[i]
            .checked_add(batch.gap().map_or(0, |g| g.dropped_bytes()))
            .ok_or("Output gap evidence overflow")?;
        self.eof[i] = batch.end();
        if role == Role::Corpus || role == Role::Forward {
            if self.raw[i].len() + batch.bytes().len() > 256 {
                return Err("Output fixed corpus exceeded bound".into());
            }
            self.raw[i].extend_from_slice(batch.bytes());
        }
        if self.transcript.allocated_bytes() > MAX_OWNER_ALLOCATION_BYTES {
            return Err("Transcript actual allocations exceed admitted bound".into());
        }
        Ok(())
    }
    pub(super) fn verify(&self, role: Role, report: WindowsOutputReport) -> Result<()> {
        for i in 0..2 {
            if self.delivered[i].checked_add(self.gaps[i]) != Some(report.counters[i].read_bytes)
                || self.gaps[i] != report.counters[i].dropped_bytes
            {
                return Err("Output delivered/loss/native counts differ".into());
            }
        }
        if role == Role::Corpus {
            if self.raw[0] != corpus::OUT_EXPECTED || self.raw[1] != corpus::ERR_EXPECTED {
                return Err("Actual three-handle corpus bytes differ".into());
            }
            let snapshot = self.transcript.snapshot(None, 8)?;
            let out: Vec<_> =
                snapshot.records().filter(|r| r.stream().index() == 0).map(|r| r.text()).collect();
            let err: Vec<_> =
                snapshot.records().filter(|r| r.stream().index() == 1).map(|r| r.text()).collect();
            if out != ["start €", "red", "new"] || err != ["error 🌸", "green", "two"] {
                return Err("Actual corpus transcript UTF8/ANSI/CR semantics differ".into());
            }
            for (text, color) in [("red", 1), ("green", 2)] {
                let record = snapshot
                    .records()
                    .find(|r| r.text() == text)
                    .ok_or("Actual ANSI corpus record missing")?;
                if !record.runs().iter().all(|run| {
                    run.style.foreground
                        == sugarsubstitute_contracts::transcript::AnsiColor::Indexed(color)
                }) {
                    return Err("Actual ANSI corpus style differs".into());
                }
            }
            let diagnostics = self.transcript.diagnostics();
            if diagnostics.unicode_replacements != 0 || diagnostics.malformed_escapes != 0 {
                return Err("Exact source corpus unexpectedly lost UTF8/ANSI semantics".into());
            }
        }
        if role == Role::Forward
            && (self.raw[0] != corpus::LEAF_OUT || self.raw[1] != corpus::LEAF_ERR)
        {
            return Err("Actual descendant writer corpus differs".into());
        }
        Ok(())
    }
}
pub(super) struct Stop {
    pub(super) id: ShutdownId,
    pub(super) budget: ShutdownBudget,
    pub(super) grace: Instant,
    pub(super) total: Instant,
    pub(super) grace_seen: bool,
    pub(super) total_seen: bool,
}
impl Stop {
    pub(super) fn new(id: ShutdownId, now: Moment, wall: Instant) -> Result<Self> {
        let budget = ShutdownBudget::new(now)?;
        Ok(Self {
            id,
            budget,
            grace: wall
                .checked_add(budget.graceful_until.checked_duration_since(now)?)
                .ok_or("Output grace overflow")?,
            total: wall
                .checked_add(budget.settle_until.checked_duration_since(now)?)
                .ok_or("Output settlement overflow")?,
            grace_seen: false,
            total_seen: false,
        })
    }
    pub(super) fn next(&self) -> Option<Instant> {
        if !self.grace_seen {
            Some(self.grace)
        } else if !self.total_seen {
            Some(self.total)
        } else {
            None
        }
    }
    pub(super) fn due(&mut self, now: Moment) -> (bool, bool) {
        let grace = !self.grace_seen && now >= self.budget.graceful_until;
        let total = !self.total_seen && now >= self.budget.settle_until;
        self.grace_seen |= grace;
        self.total_seen |= total;
        (grace, total)
    }
}
pub(super) struct Case {
    pub(super) role: Role,
    pub(super) resource: ResourceKey,
    pub(super) run: RunKey,
    pub(super) output: Option<WindowsOutputSession>,
    pub(super) output_notice: Option<TerminalKind>,
    pub(super) output_terminal: Option<TerminalKind>,
    pub(super) output_report: Option<WindowsOutputReport>,
    pub(super) channel: Option<Channel>,
    pub(super) store_job: Option<OwnedJob<Store, io::Error>>,
    pub(super) store_notice: Option<TerminalKind>,
    pub(super) store: Option<Store>,
    pub(super) process: Option<ProcessKey>,
    pub(super) started: bool,
    pub(super) exited: Option<u32>,
    pub(super) stop: Option<Stop>,
    pub(super) cleanup_requested: bool,
    pub(super) cleanup_success: bool,
    pub(super) member_success: bool,
    pub(super) cleanup_job: Option<sugarsubstitute_contracts::execution::JobId>,
    pub(super) member_job: Option<sugarsubstitute_contracts::execution::JobId>,
    pub(super) settled: bool,
    pub(super) retired: bool,
    pub(super) guard: Option<AdmittedOutputBatch>,
    pub(super) guard_busy: bool,
    pub(super) primed: bool,
    pub(super) go: bool,
    pub(super) done: bool,
    pub(super) exit_sent: bool,
    pub(super) pending_before_cancel: Option<WindowsOutputReport>,
    pub(super) root_before_eof: bool,
    pub(super) complete_inventory: bool,
    pub(super) forced: bool,
    pub(super) member_verified: bool,
    pub(super) diagnostics: Option<TranscriptDiagnostics>,
    pub(super) sequences: [u64; 2],
    pub(super) delivered: [u64; 2],
    pub(super) gaps: [u64; 2],
    pub(super) drained: bool,
}
impl Case {
    pub(super) fn output_identity(&self) -> OutputIdentity {
        if self.role.authenticated() {
            OutputIdentity::Desktop(self.run)
        } else {
            OutputIdentity::Managed(self.resource)
        }
    }
    pub(super) fn new(role: Role, resource: ResourceKey, run: RunKey) -> Self {
        Self {
            role,
            resource,
            run,
            output: None,
            output_notice: None,
            output_terminal: None,
            output_report: None,
            channel: None,
            store_job: None,
            store_notice: None,
            store: None,
            process: None,
            started: false,
            exited: None,
            stop: None,
            cleanup_requested: false,
            cleanup_success: false,
            member_success: false,
            cleanup_job: None,
            member_job: None,
            settled: false,
            retired: false,
            guard: None,
            guard_busy: false,
            primed: false,
            go: false,
            done: false,
            exit_sent: false,
            pending_before_cancel: None,
            root_before_eof: false,
            complete_inventory: false,
            forced: false,
            member_verified: false,
            diagnostics: None,
            sequences: [0; 2],
            delivered: [0; 2],
            gaps: [0; 2],
            drained: false,
        }
    }
    pub(super) fn complete(&self) -> bool {
        self.started
            && self.exited == Some(0)
            && self.settled
            && self.retired
            && self.drained
            && (!self.role.authenticated() || self.exit_sent)
            && (self.role == Role::Forward || !self.forced)
            && self.channel.as_ref().is_none_or(|c| c.terminal && c.verified)
            && match self.role {
                Role::Corpus => self.output_terminal == Some(TerminalKind::Success),
                Role::Flood => {
                    self.guard_busy
                        && self.go
                        && self.done
                        && self.output_terminal == Some(TerminalKind::Success)
                }
                Role::Idle => {
                    self.pending_before_cancel.is_some()
                        && self.output_terminal == Some(TerminalKind::Cancelled)
                }
                Role::Forward => {
                    self.root_before_eof
                        && self.complete_inventory
                        && self.forced
                        && self.member_verified
                        && self.output_terminal == Some(TerminalKind::Success)
                }
                _ => false,
            }
    }
}
