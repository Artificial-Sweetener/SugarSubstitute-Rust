// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Run input document work as owned jobs, one at a time, and apply what each produces.
//!
//! Work is queued per surface and target. A newer request for the same target replaces a
//! queued one or supersedes the running one, whose outcome is still observed and whose product
//! is then disposed of on the retirement worker. Every adapter a job used, or a request no
//! longer needs, is kept until its native custody has retired.
use super::{
    build::{CreateBase, Product},
    reconcile::NewBase,
    *,
};
use sugarsubstitute_canvas::LayerContent;

/// What a task changes on its surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Target {
    Base,
    Mask(LayerId),
    Geometry,
}

/// Requested work.
pub(super) enum Work {
    /// Create the document, or replace its base once it exists.
    Base(SurfaceBase),
    /// Load a mask file into an existing mask.
    Import { file: ComfyFile, channel: MaskChannel },
    /// Resize a blank-based document and every mask.
    Resize { extent: Extent, mode: ResizeMode, ticket: u64 },
}

/// One queued request.
pub(super) struct Task {
    pub(super) surface: InputDocumentId,
    pub(super) target: Target,
    pub(super) work: Work,
}

impl Task {
    /// The file this task loads, which identifies equivalent requests.
    fn reference(&self) -> Option<&Arc<str>> {
        match &self.work {
            Work::Base(SurfaceBase::File(file)) | Work::Import { file, .. } => {
                Some(&file.reference)
            }
            Work::Base(SurfaceBase::Blank(_)) | Work::Resize { .. } => None,
        }
    }

    /// Every adapter this task holds.
    pub(super) fn acquirers(self) -> Vec<Arc<dyn acquisition::SourceAcquirer>> {
        match self.work {
            Work::Base(SurfaceBase::File(file)) | Work::Import { file, .. } => vec![file.acquirer],
            Work::Base(SurfaceBase::Blank(_)) | Work::Resize { .. } => Vec::new(),
        }
    }
}

/// The worker half of one task.
type Run = Box<dyn FnOnce(&JobContext) -> Result<Product, WorkError> + Send>;

/// What the running job will change once it completes.
#[derive(Clone)]
pub(super) enum Applies {
    Create(Base),
    Rebase(Arc<str>),
    Import { layer: LayerId, reference: Arc<str>, expected: Extent },
    Resize { ticket: u64, revision: u64, extent: Extent },
}

/// The one admitted input job.
pub(super) struct Running {
    pub(super) surface: InputDocumentId,
    target: Target,
    reference: Option<Arc<str>>,
    applies: Applies,
    pub(super) job: OwnedJob<Product, WorkError>,
    pub(super) acquirers: Vec<Arc<dyn acquisition::SourceAcquirer>>,
    superseded: bool,
}

impl InputDocuments {
    /// Queue `task`, replacing a queued request for the same target or superseding a running
    /// one. An equivalent request already queued or running keeps its place.
    pub(super) fn request(&mut self, task: Task) {
        if let Some(queued) = self
            .queue
            .iter_mut()
            .find(|queued| queued.surface == task.surface && queued.target == task.target)
        {
            let replaced = std::mem::replace(queued, task);
            for owner in replaced.acquirers() {
                self.release(owner);
            }
            return;
        }
        if let Some(running) = self.running.as_mut().filter(|running| {
            running.surface == task.surface && running.target == task.target && !running.superseded
        }) {
            if task.target != Target::Geometry && running.reference.as_ref() == task.reference() {
                for owner in task.acquirers() {
                    self.release(owner);
                }
                return;
            }
            running.superseded = true;
            running.job.cancel();
            running.acquirers.iter().for_each(|owner| owner.cancel());
        }
        self.queue.push_back(task);
    }

    /// Drop queued work for one target, or every target of a surface, and supersede it when
    /// it is running.
    pub(super) fn withdraw(&mut self, surface: InputDocumentId, target: Option<Target>) {
        let matches = |candidate: InputDocumentId, aim: Target| {
            candidate == surface && target.is_none_or(|target| target == aim)
        };
        let mut kept = VecDeque::with_capacity(self.queue.len());
        while let Some(task) = self.queue.pop_front() {
            if matches(task.surface, task.target) {
                for owner in task.acquirers() {
                    self.release(owner);
                }
            } else {
                kept.push_back(task);
            }
        }
        self.queue = kept;
        if let Some(running) =
            self.running.as_mut().filter(|running| matches(running.surface, running.target))
        {
            running.superseded = true;
            running.job.cancel();
            running.acquirers.iter().for_each(|owner| owner.cancel());
        }
    }
}

impl Session {
    /// Admit the first queued task whose preconditions hold. Tasks that can never run are
    /// answered at once.
    pub(in crate::native_canvas::session) fn submit_input_job(
        &mut self,
    ) -> Result<(), Box<dyn Error>> {
        if self.closing || self.inputs.running.is_some() {
            return Ok(());
        }
        loop {
            let Some(position) = self.inputs.queue.iter().position(|task| {
                matches!(task.target, Target::Base)
                    || self.inputs.by_identity(task.surface).is_none_or(|surface| surface.loaded)
                    || !self
                        .inputs
                        .queue
                        .iter()
                        .any(|other| other.surface == task.surface && other.target == Target::Base)
            }) else {
                return Ok(());
            };
            let Some(task) = self.inputs.queue.remove(position) else {
                return Ok(());
            };
            if let Some(task) = self.start_input_task(task)? {
                self.inputs.queue.insert(position, task);
                return Ok(());
            }
            if self.inputs.running.is_some() {
                return Ok(());
            }
        }
    }

    /// Start one task's job, apply it at once when it needs none, or return it when execution
    /// admission is saturated.
    fn start_input_task(&mut self, task: Task) -> Result<Option<Task>, Box<dyn Error>> {
        let Some(surface) = self.inputs.by_identity(task.surface) else {
            for owner in task.acquirers() {
                self.inputs.release(owner);
            }
            return Ok(None);
        };
        let identity = surface.identity;
        let label = surface.label.clone();
        let loaded = surface.loaded;
        let snapshot = self
            .input_document(identity)
            .and_then(|document| document.scene.as_ref())
            .map(|scene| scene.document.snapshot());
        let resources = self.resources.clone().ok_or("Canvas storage is unavailable")?;
        let (applies, acquirers, run): (Applies, Vec<_>, Run) = match &task.work {
            Work::Base(SurfaceBase::Blank(extent)) if loaded => {
                let rebased =
                    self.rebase_input(identity, NewBase::Blank(*extent), Base::Blank(*extent));
                self.refuse_input_memory(identity, Applies::Create(Base::Blank(*extent)), rebased)?;
                // No job completion follows this change, so its request is answered here.
                self.settle_input_tickets();
                return Ok(None);
            }
            Work::Base(base) => {
                let (create, applies, acquirers) = match base {
                    SurfaceBase::File(file) if loaded => {
                        let owner = file.acquirer.clone();
                        let applies = Applies::Rebase(file.reference.clone());
                        (None, applies, vec![owner])
                    }
                    SurfaceBase::File(file) => (
                        Some(CreateBase::File(file.acquirer.clone())),
                        Applies::Create(Base::File(file.reference.clone())),
                        vec![file.acquirer.clone()],
                    ),
                    SurfaceBase::Blank(extent) => (
                        Some(CreateBase::Blank(*extent)),
                        Applies::Create(Base::Blank(*extent)),
                        Vec::new(),
                    ),
                };
                let job_resources = resources.clone();
                let run: Run = match create {
                    Some(base) => Box::new(move |context: &JobContext| {
                        build::create(identity, &label, base, &job_resources, context)
                    }),
                    None => {
                        let owner = acquirers[0].clone();
                        Box::new(move |context: &JobContext| {
                            let (source, original) =
                                build::rebase(owner.as_ref(), &job_resources, context)?;
                            Ok(Product::Rebased { source, original })
                        })
                    }
                };
                (applies, acquirers, run)
            }
            Work::Import { file, channel } => {
                let Target::Mask(layer) = task.target else {
                    return Err("Mask import lost its mask".into());
                };
                let present = snapshot.as_ref().is_some_and(|snapshot| {
                    snapshot.layers().iter().any(|candidate| candidate.id() == layer)
                });
                let Some(expected) =
                    snapshot.as_ref().filter(|_| present).map(DocumentSnapshot::extent)
                else {
                    tracing::warn!(
                        operation = "canvas_input_work",
                        document_id = identity.get(),
                        layer = layer.get(),
                        "A mask file cannot load: its document or mask is unavailable"
                    );
                    self.fail_input_import(
                        identity,
                        layer,
                        InputFailure::message("The mask's document is unavailable"),
                    );
                    for owner in task.acquirers() {
                        self.inputs.release(owner);
                    }
                    // No job completion follows to answer the request that asked for the file.
                    self.settle_input_tickets();
                    return Ok(None);
                };
                let owner = file.acquirer.clone();
                let channel = *channel;
                let job_resources = resources.clone();
                let applies =
                    Applies::Import { layer, reference: file.reference.clone(), expected };
                let run: Run = Box::new(move |context: &JobContext| {
                    build::import_mask(owner.as_ref(), channel, expected, &job_resources, context)
                        .map(Product::Imported)
                });
                (applies, vec![file.acquirer.clone()], run)
            }
            Work::Resize { extent, mode, ticket } => {
                let blank = self
                    .inputs
                    .by_identity(identity)
                    .is_some_and(|surface| matches!(surface.loaded_base, Some(Base::Blank(_))));
                let Some(snapshot) = snapshot.filter(|_| loaded && blank) else {
                    self.fail_input_ticket(
                        *ticket,
                        identity,
                        InputFailure::message("Only a loaded blank-based surface can be resized"),
                    );
                    self.settle_input_tickets();
                    return Ok(None);
                };
                let masks: Vec<_> = snapshot
                    .layers()
                    .iter()
                    .filter_map(|layer| match layer.content() {
                        LayerContent::Coverage { coverage, .. } => {
                            Some((layer.id(), coverage.clone()))
                        }
                        _ => None,
                    })
                    .collect();
                let (from, to, mode) = (snapshot.extent(), *extent, *mode);
                let job_resources = resources.clone();
                let applies =
                    Applies::Resize { ticket: *ticket, revision: snapshot.revision(), extent: to };
                let run: Run = Box::new(move |context: &JobContext| {
                    build::resize(masks, from, to, mode, &job_resources, context)
                });
                (applies, Vec::new(), run)
            }
        };
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        let extra = acquirers.iter().map(|owner| owner.reservation_bytes()).max().unwrap_or(0);
        let spec = self.storage_spec(
            Operation::Infrastructure,
            JobClass::InfrastructureOrdinary,
            work::IMPORT_BYTES.checked_add(extra).ok_or("Input reservation overflow")?,
            None,
        )?;
        match runtime.client.submit_blocking(&runtime.scope, spec, move |context| run(&context)) {
            Ok(job) => {
                let reference = task.reference().cloned();
                self.inputs.running = Some(Running {
                    surface: identity,
                    target: task.target,
                    reference,
                    applies,
                    job,
                    acquirers,
                    superseded: false,
                });
                Ok(None)
            }
            Err(cause) if activity::retryable(&cause) => Ok(Some(task)),
            Err(cause) => Err(cause.into()),
        }
    }

    /// Observe the running input job and apply or dispose of what it produced.
    pub(in crate::native_canvas::session) fn consume_input_job(
        &mut self,
    ) -> Result<(), Box<dyn Error>> {
        let Some(completion) =
            self.inputs.running.as_mut().and_then(|running| running.job.try_take_completion())
        else {
            return Ok(());
        };
        let running = self.inputs.running.take().ok_or("Input job identity was lost")?;
        Self::observe_terminal(&running.job, &completion.outcome);
        let unsettled = running.acquirers.iter().any(|owner| !owner.settled());
        for owner in running.acquirers.iter().cloned() {
            self.inputs.release(owner);
        }
        let dismissed = running.superseded || self.closing;
        match completion.outcome {
            JobOutcome::Success(product) if dismissed => {
                self.inputs.retiring.products.push(product)
            }
            JobOutcome::Success(product) if unsettled => {
                self.inputs.retiring.products.push(product);
                let failure =
                    InputFailure::message("Acquisition returned before native retirement");
                self.record_input_failure(running.surface, running.applies, failure);
            }
            JobOutcome::Success(product) => {
                let applied =
                    self.apply_input_product(running.surface, running.applies.clone(), product);
                self.refuse_input_memory(running.surface, running.applies, applied)?;
            }
            _ if dismissed => {}
            outcome => {
                let failure = match outcome {
                    JobOutcome::Failure(cause) => InputFailure::new(cause),
                    JobOutcome::Panicked(_) => {
                        InputFailure::message("Input canvas worker panicked")
                    }
                    JobOutcome::TimedOut => {
                        InputFailure::message("Input canvas work exceeded its deadline")
                    }
                    _ => InputFailure::message("Input canvas work stopped before completion"),
                };
                self.record_input_failure(running.surface, running.applies, failure);
            }
        }
        self.settle_input_tickets();
        Ok(())
    }

    /// Keep a memory refusal of applying `applies` where its request's reply reports it, so
    /// work the canvas memory budget cannot hold is answered instead of failing the canvas.
    /// Any other failure is returned unchanged.
    fn refuse_input_memory(
        &mut self,
        identity: InputDocumentId,
        applies: Applies,
        applied: Result<(), Box<dyn Error>>,
    ) -> Result<(), Box<dyn Error>> {
        let Err(cause) = applied else { return Ok(()) };
        if !memory_refused(cause.as_ref()) {
            return Err(cause);
        }
        tracing::warn!(
            operation = "canvas_input_memory",
            document_id = identity.get(),
            cause_chain = %crate::diagnostics::CauseChain(cause.as_ref()),
            "The canvas memory budget refused input document work"
        );
        self.record_input_failure(identity, applies, InputFailure::message(MEMORY_REFUSED));
        Ok(())
    }
}

/// The page's answer when the canvas memory budget cannot hold an input document change.
const MEMORY_REFUSED: &str = "The canvas memory budget cannot hold this input document";
