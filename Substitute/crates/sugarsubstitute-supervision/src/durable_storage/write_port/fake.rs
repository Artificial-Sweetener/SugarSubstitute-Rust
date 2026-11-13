// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Test fault plans that fail one update write below a registered folder.
//!
//! A plan observes every write below its folder in order. Armed, it fails the Nth write or the
//! Nth write of a named kind once. A power cut leaves the stage complete but unpublished and turns
//! the plan's disk off: no later write below the folder lands until the test restores power,
//! which models the next start of the machine.
use super::UpdateFile;
use crate::native_claims::lock;
use std::{
    fs::File,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

/// One fault class injected at a write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fault {
    /// The disk is full before any byte lands.
    StorageFull,
    /// The complete stage cannot replace its destination.
    AccessDenied,
    /// Half of the bytes land, then the device fails.
    TornWrite,
    /// Every byte lands, then the power fails before the stage is published.
    PowerCut,
}

impl Fault {
    /// Every fault class, in the order tests walk them.
    pub(crate) const ALL: [Self; 4] =
        [Self::StorageFull, Self::AccessDenied, Self::TornWrite, Self::PowerCut];
}

/// Which write an armed plan fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    /// The write at this zero-based position among all writes below the folder.
    Nth(usize),
    /// The write at this zero-based position among writes of one kind.
    Named(UpdateFile, usize),
}

/// One write a plan observed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Observed {
    pub(crate) kind: UpdateFile,
    pub(crate) destination: PathBuf,
}

struct Plan {
    roots: Vec<String>,
    armed: Option<(Target, Fault)>,
    writes: Vec<Observed>,
    fired: Option<(Observed, Fault)>,
    powered: bool,
}

static PLANS: Mutex<Vec<Arc<Mutex<Plan>>>> = Mutex::new(Vec::new());

/// Comparable spelling of a local path: no verbatim prefix, backslashes, lowercase.
fn normalized(path: &Path) -> String {
    let text = path.to_string_lossy().replace('/', "\\");
    let text = text.strip_prefix("\\\\?\\").unwrap_or(&text);
    let mut text = text.to_lowercase();
    while text.ends_with('\\') {
        text.pop();
    }
    text
}

/// A registered fault plan; dropping it unregisters the folder.
pub(crate) struct FaultPlan(Arc<Mutex<Plan>>);

/// Observe, and optionally fail, every update write below `root` until the plan drops.
pub(crate) fn install(root: &Path) -> io::Result<FaultPlan> {
    let mut roots = vec![normalized(root)];
    let canonical = normalized(&std::fs::canonicalize(root)?);
    if !roots.contains(&canonical) {
        roots.push(canonical);
    }
    let plan = Arc::new(Mutex::new(Plan {
        roots,
        armed: None,
        writes: Vec::new(),
        fired: None,
        powered: true,
    }));
    lock(&PLANS).push(plan.clone());
    Ok(FaultPlan(plan))
}

impl FaultPlan {
    /// Fail `target` with `fault` once; earlier observations are kept.
    pub(crate) fn arm(&self, target: Target, fault: Fault) {
        let mut plan = lock(&self.0);
        plan.armed = Some((target, fault));
        plan.fired = None;
    }

    /// Every write observed so far, in order.
    pub(crate) fn writes(&self) -> Vec<Observed> {
        lock(&self.0).writes.clone()
    }

    /// The write the armed fault failed, if it was reached.
    pub(crate) fn fired(&self) -> Option<(Observed, Fault)> {
        lock(&self.0).fired.clone()
    }

    /// Start the machine again: the disk accepts writes and nothing stays armed.
    pub(crate) fn restore_power(&self) {
        let mut plan = lock(&self.0);
        plan.powered = true;
        plan.armed = None;
    }
}

impl Drop for FaultPlan {
    fn drop(&mut self) {
        lock(&PLANS).retain(|plan| !Arc::ptr_eq(plan, &self.0));
    }
}

#[derive(Clone, Copy, Debug)]
enum Effect {
    Fault(Fault),
    /// The device failed this write; the process carries on.
    Failed,
    PoweredOff,
}

/// The effect a plan applies to one write that began below its folder.
pub(crate) struct Armed {
    plan: Arc<Mutex<Plan>>,
    effect: Effect,
}

fn os(code: i32) -> io::Error {
    io::Error::from_raw_os_error(code)
}

const ERROR_ACCESS_DENIED: i32 = 5;
const ERROR_NOT_READY: i32 = 21;
const ERROR_DISK_FULL: i32 = 112;
const ERROR_IO_DEVICE: i32 = 1117;

/// Find the plan for `destination`, record the write and decide its effect.
pub(super) fn begin(kind: UpdateFile, destination: &Path) -> Option<Armed> {
    let name = normalized(destination);
    let plan = lock(&PLANS)
        .iter()
        .find(|plan| {
            lock(plan).roots.iter().any(|root| {
                name.strip_prefix(root.as_str()).is_some_and(|rest| rest.starts_with('\\'))
            })
        })?
        .clone();
    let effect = {
        let mut state = lock(&plan);
        if !state.powered {
            Some(Effect::PoweredOff)
        } else {
            let position = state.writes.len();
            let occurrence = state.writes.iter().filter(|write| write.kind == kind).count();
            let observed = Observed { kind, destination: destination.to_owned() };
            state.writes.push(observed.clone());
            match state.armed {
                Some((target, fault))
                    if state.fired.is_none()
                        && match target {
                            Target::Nth(index) => index == position,
                            Target::Named(named, index) => named == kind && index == occurrence,
                        } =>
                {
                    state.fired = Some((observed, fault));
                    Some(Effect::Fault(fault))
                }
                _ => None,
            }
        }
    }?;
    Some(Armed { plan, effect })
}

impl Armed {
    pub(super) fn bytes(&mut self, file: &mut File, bytes: &[u8]) -> io::Result<()> {
        match self.effect {
            Effect::PoweredOff => Err(os(ERROR_NOT_READY)),
            Effect::Failed => Err(os(ERROR_IO_DEVICE)),
            Effect::Fault(Fault::StorageFull) => Err(os(ERROR_DISK_FULL)),
            Effect::Fault(Fault::TornWrite) => {
                file.write_all(&bytes[..bytes.len() / 2])?;
                self.effect = Effect::Failed;
                Err(os(ERROR_IO_DEVICE))
            }
            Effect::Fault(Fault::AccessDenied | Fault::PowerCut) => file.write_all(bytes),
        }
    }

    pub(super) fn abandon(&self) -> io::Result<()> {
        match self.effect {
            Effect::PoweredOff => Err(os(ERROR_NOT_READY)),
            Effect::Fault(_) | Effect::Failed => Ok(()),
        }
    }

    pub(super) fn publish(&mut self) -> io::Result<()> {
        match self.effect {
            Effect::PoweredOff => Err(os(ERROR_NOT_READY)),
            Effect::Failed => Err(os(ERROR_IO_DEVICE)),
            Effect::Fault(Fault::AccessDenied) => Err(os(ERROR_ACCESS_DENIED)),
            Effect::Fault(Fault::PowerCut) => {
                lock(&self.plan).powered = false;
                self.effect = Effect::PoweredOff;
                Err(os(ERROR_NOT_READY))
            }
            Effect::Fault(Fault::StorageFull | Fault::TornWrite) => Ok(()),
        }
    }
}
