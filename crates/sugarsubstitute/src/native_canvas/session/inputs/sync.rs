// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Replace one workflow's surface set and schedule the work it implies.
use super::{
    jobs::{Target, Task, Work},
    *,
};

/// Longest key or label, in characters, the canvas protocol carries.
const MAX_TEXT: usize = 256;

/// What one surface's sync asks of the job queue.
#[derive(Default)]
struct Plan {
    tasks: Vec<Task>,
    unused: Vec<Arc<dyn acquisition::SourceAcquirer>>,
    withdrawn: Vec<Target>,
}

impl Session {
    /// Replace `workflow`'s surface set. Listed surfaces are created or updated by key;
    /// surfaces no longer listed are retired with their documents. Loading runs as owned jobs;
    /// the returned ticket's [`InputReply`] reports every listed surface once its work settles.
    ///
    /// # Errors
    /// Rejects more than 64 surfaces or 32 masks per surface, repeated keys, keys or labels
    /// longer than 256 characters and a closing canvas with [`InputRejection`], changing
    /// nothing; preserves provisioning and identity-exhaustion failures.
    pub(crate) fn sync_inputs(
        &mut self,
        workflow: &str,
        surfaces: Vec<SurfaceSpec>,
    ) -> Result<InputTicket, Box<dyn Error>> {
        let rejection = if self.closing {
            Some(InputRejection::Unavailable("The canvas is closing"))
        } else {
            validate(&surfaces).err()
        };
        if let Some(rejection) = rejection {
            for spec in surfaces {
                for owner in spec_acquirers(spec) {
                    self.inputs.release(owner);
                }
            }
            return Err(Box::new(rejection));
        }
        let workflow: Arc<str> = Arc::from(workflow);
        let index = match self.inputs.workflows.iter().position(|(id, _)| *id == workflow) {
            Some(index) => index,
            None => {
                self.inputs.workflows.push((workflow.clone(), Vec::new()));
                self.inputs.workflows.len() - 1
            }
        };
        let mut previous = std::mem::take(&mut self.inputs.workflows[index].1);
        let mut listed = Vec::with_capacity(surfaces.len());
        let mut plans = Vec::with_capacity(surfaces.len());
        let keys: Vec<Arc<str>> = surfaces.iter().map(|spec| spec.key.clone()).collect();
        for spec in surfaces {
            let (mut surface, fresh) =
                match previous.iter().position(|surface| surface.key == spec.key) {
                    Some(position) => (previous.remove(position), false),
                    None => (self.inputs.new_surface(&spec)?, true),
                };
            plans.push((surface.identity, surface.update(spec, fresh)?));
            listed.push(surface);
        }
        self.inputs.workflows[index].1 = listed;
        for retired in previous {
            self.retire_surface(retired);
        }
        let mut loaded = Vec::new();
        for (identity, plan) in plans {
            for target in plan.withdrawn {
                self.inputs.withdraw(identity, Some(target));
            }
            for owner in plan.unused {
                self.inputs.release(owner);
            }
            for task in plan.tasks {
                self.inputs.request(task);
            }
            loaded.push(identity);
        }
        for identity in loaded {
            self.reconcile_input(identity)?;
        }
        if self.inputs.workflows[index].1.is_empty() {
            self.inputs.workflows.remove(index);
        }
        let ticket = self.input_ticket(workflow, keys)?;
        self.settle_input_tickets();
        Ok(ticket)
    }

    /// Bring a loaded document, current or parked, in line with its surface.
    pub(super) fn reconcile_input(
        &mut self,
        identity: InputDocumentId,
    ) -> Result<(), Box<dyn Error>> {
        let Some(memory) = self.resources.as_ref().map(|resources| resources.memory.clone()) else {
            return Ok(());
        };
        let current = self.document.input == Some(identity);
        let Some(surface) = self.inputs.by_identity_mut(identity).filter(|surface| surface.loaded)
        else {
            return Ok(());
        };
        if current {
            return surface.reconcile(&mut self.document, &memory);
        }
        let Some(mut document) = surface.document.take() else {
            return Ok(());
        };
        let reconciled = surface.reconcile(&mut document, &memory);
        surface.document = Some(document);
        reconciled
    }

    /// Withdraw a surface no longer listed. Its parked document retires at once; a shown one
    /// leaves the screen through the Output route and then retires.
    fn retire_surface(&mut self, mut surface: Surface) {
        self.inputs.withdraw(surface.identity, None);
        if let Some(document) = surface.document.take() {
            self.inputs.retiring.documents.push(document);
        }
        let routed = matches!(
            self.inputs.route,
            Some(route::Pending::Input { surface: pending, .. }) if pending != surface.identity
        );
        if self.document.input == Some(surface.identity) && !routed {
            self.inputs.route = Some(route::Pending::Output);
        }
        if matches!(self.inputs.route, Some(route::Pending::Input { surface: pending, .. }) if pending == surface.identity)
        {
            self.inputs.route = None;
        }
    }
}

impl InputDocuments {
    /// A record for a newly listed surface with a fresh document identity.
    fn new_surface(&mut self, spec: &SurfaceSpec) -> Result<Surface, Box<dyn Error>> {
        self.next_identity =
            self.next_identity.checked_add(1).ok_or("Input document identity exhausted")?;
        if self.next_identity & IDENTITY_NAMESPACE != 0 {
            return Err("Input document identity exhausted".into());
        }
        let desired_base = match &spec.base {
            SurfaceBase::File(file) => Base::File(file.reference.clone()),
            SurfaceBase::Blank(extent) => Base::Blank(*extent),
        };
        Ok(Surface {
            key: spec.key.clone(),
            label: spec.label.clone(),
            identity: InputDocumentId(IDENTITY_NAMESPACE | self.next_identity),
            desired_base,
            loaded_base: None,
            masks: Vec::new(),
            next_layer: BASE_LAYER + 1,
            document: None,
            restored: None,
            loaded: false,
            failure: None,
        })
    }
}

impl Surface {
    /// Take the synced state of `spec` and plan the work it implies.
    fn update(&mut self, spec: SurfaceSpec, fresh: bool) -> Result<Plan, Box<dyn Error>> {
        let mut plan = Plan::default();
        self.label = spec.label;
        let base = match spec.base {
            SurfaceBase::File(file) => {
                self.desired_base = Base::File(file.reference.clone());
                SurfaceBase::File(file)
            }
            // A blank backing takes its size only when created; resizes change it later.
            SurfaceBase::Blank(extent) => match (&self.desired_base, fresh) {
                (Base::Blank(kept), false) => SurfaceBase::Blank(*kept),
                _ => {
                    self.desired_base = Base::Blank(extent);
                    SurfaceBase::Blank(extent)
                }
            },
        };
        let holds = match (&self.loaded_base, &self.desired_base) {
            (Some(Base::Blank(_)), Base::Blank(_)) => true,
            (loaded, desired) => loaded.as_ref() == Some(desired),
        };
        if self.loaded && holds {
            plan.unused.extend(base_acquirer(base));
            plan.withdrawn.push(Target::Base);
        } else {
            plan.tasks.push(Task {
                surface: self.identity,
                target: Target::Base,
                work: Work::Base(base),
            });
        }
        let mut masks = Vec::with_capacity(spec.masks.len());
        for spec in spec.masks {
            let mut mask = match self.masks.iter().position(|mask| mask.key == spec.key) {
                Some(position) => self.masks.remove(position),
                None => {
                    let layer = LayerId::new(self.next_layer)?;
                    self.next_layer =
                        self.next_layer.checked_add(1).ok_or("Mask identity exhausted")?;
                    Mask {
                        key: spec.key.clone(),
                        label: spec.label.clone(),
                        layer,
                        appearance: spec.appearance,
                        appearance_pending: false,
                        channel: spec.channel,
                        desired_initial: None,
                        loaded_initial: None,
                        outcome: None,
                    }
                }
            };
            mask.label = spec.label;
            mask.channel = spec.channel;
            if spec.appearance != mask.appearance {
                mask.appearance = spec.appearance;
                mask.appearance_pending = true;
            }
            if let Some(file) = spec.initial {
                if mask.desired_initial.as_ref() != Some(&file.reference) {
                    mask.desired_initial = Some(file.reference.clone());
                    mask.outcome = None;
                }
                if mask.desired_initial != mask.loaded_initial {
                    plan.tasks.push(Task {
                        surface: self.identity,
                        target: Target::Mask(mask.layer),
                        work: Work::Import { file, channel: mask.channel },
                    });
                } else {
                    plan.unused.push(file.acquirer);
                    plan.withdrawn.push(Target::Mask(mask.layer));
                }
            }
            masks.push(mask);
        }
        plan.withdrawn.extend(self.masks.iter().map(|mask| Target::Mask(mask.layer)));
        self.masks = masks;
        Ok(plan)
    }
}

/// The adapter a base request would use.
fn base_acquirer(base: SurfaceBase) -> Option<Arc<dyn acquisition::SourceAcquirer>> {
    match base {
        SurfaceBase::File(file) => Some(file.acquirer),
        SurfaceBase::Blank(_) => None,
    }
}

/// Every adapter a surface spec carries.
fn spec_acquirers(spec: SurfaceSpec) -> Vec<Arc<dyn acquisition::SourceAcquirer>> {
    base_acquirer(spec.base)
        .into_iter()
        .chain(spec.masks.into_iter().filter_map(|mask| mask.initial.map(|file| file.acquirer)))
        .collect()
}

/// Check a sync's limits and key uniqueness before anything changes.
fn validate(surfaces: &[SurfaceSpec]) -> Result<(), InputRejection> {
    let long = |text: &str| text.chars().count() > MAX_TEXT;
    if surfaces.len() > MAX_SURFACES {
        return Err(InputRejection::Invalid("A workflow syncs at most 64 input surfaces"));
    }
    for (index, surface) in surfaces.iter().enumerate() {
        if surfaces[..index].iter().any(|earlier| earlier.key == surface.key) {
            return Err(InputRejection::Invalid("Input surface keys repeat"));
        }
        if surface.masks.len() > MAX_MASKS {
            return Err(InputRejection::Invalid("An input surface carries at most 32 masks"));
        }
        if long(&surface.key) || long(&surface.label) {
            return Err(InputRejection::Invalid("An input surface key or label is too long"));
        }
        for (position, mask) in surface.masks.iter().enumerate() {
            if surface.masks[..position].iter().any(|earlier| earlier.key == mask.key) {
                return Err(InputRejection::Invalid("Input mask keys repeat"));
            }
            if long(&mask.key) || long(&mask.label) {
                return Err(InputRejection::Invalid("An input mask key or label is too long"));
            }
        }
    }
    Ok(())
}
