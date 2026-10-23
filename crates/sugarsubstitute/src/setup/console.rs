// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare installer output for the splash's existing native console owner.
use crate::job_sequence::JobSequence;
use std::{collections::VecDeque, error::Error, io, num::NonZeroUsize};
use sugarsubstitute_contracts::{
    execution::*,
    lifecycle::*,
    presentation::{ConsolePresentation, FrameIdentity, FrameShape, FrameValidator, PreparedFrame},
    transcript::{
        MAX_SNAPSHOT_RECORDS, OutputStream, RecordId, StyleRun, TextStyle, TranscriptRecord,
    },
};
use sugarsubstitute_desktop::{
    ConsoleText, ConsoleView,
    installer::{InstallerConsole, InstallerConsoleLayout},
};
use sugarsubstitute_execution::ExecutionClient;
type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub(super) struct Output {
    records: VecDeque<TranscriptRecord>,
    revision: u64,
    sequence: u64,
    key: ResourceKey,
    layout: Option<InstallerConsoleLayout>,
    prepared: Option<(InstallerConsoleLayout, u64, PreparedFrame)>,
    job: Option<(InstallerConsoleLayout, u64, OwnedJob<PreparedFrame, io::Error>)>,
}
impl Output {
    pub(super) fn new() -> Result<Self> {
        Ok(Self {
            records: VecDeque::new(),
            revision: 1,
            sequence: 0,
            layout: None,
            prepared: None,
            job: None,
            key: ResourceKey {
                owner: CoordinatorKey {
                    namespace: NamespaceId::new([0x49; 32]),
                    epoch: CoordinatorEpoch::new([0x50; 16])?,
                },
                id: ResourceId::new(1)?,
                generation: ResourceGeneration::new(1)?,
            },
        })
    }
    pub(super) fn append(&mut self, text: &str) -> Result<()> {
        for line in text.lines().take(128) {
            self.revision =
                self.revision.checked_add(1).ok_or("Installer output identity exhausted")?;
            let mut end = line.len().min(1024);
            while !line.is_char_boundary(end) {
                end -= 1;
            }
            let text = line[..end].to_owned();
            let runs = if text.is_empty() {
                vec![]
            } else {
                vec![StyleRun { start: 0, end: text.len() as u32, style: TextStyle::default() }]
            };
            self.records.push_back(TranscriptRecord::new(
                RecordId::new(self.revision)?,
                OutputStream::Stdout,
                text,
                runs,
                true,
            )?);
            if self.records.len() > MAX_SNAPSHOT_RECORDS {
                self.records.pop_front();
            }
        }
        Ok(())
    }
    pub(super) fn layout(&mut self, value: InstallerConsoleLayout) -> Result<()> {
        self.layout = Some(layout::clip(value)?);
        Ok(())
    }
    pub(super) fn advance(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        closing: bool,
    ) -> Result<()> {
        if closing && let Some((_, _, job)) = &self.job {
            job.cancel();
        }
        if let Some((layout, revision, job)) = &mut self.job
            && let Some(completion) = job.try_take_completion()
        {
            let identity = (*layout, *revision);
            self.job = None;
            match completion.outcome {
                JobOutcome::Success(frame) if !closing && self.layout == Some(identity.0) => {
                    self.prepared = Some((identity.0, identity.1, frame))
                }
                JobOutcome::Success(_) | JobOutcome::Cancelled => {}
                JobOutcome::Failure(cause) => return Err(cause.into()),
                _ => return Err("Installer console preparation did not complete".into()),
            }
        }
        let Some(layout) = self.layout else {
            return Ok(());
        };
        if closing
            || !layout.visible
            || self.job.is_some()
            || self
                .prepared
                .as_ref()
                .is_some_and(|(old, revision, _)| *old == layout && *revision == self.revision)
        {
            return Ok(());
        }
        let shape = FrameShape::new(layout.width, layout.height, layout.scale)?;
        self.sequence = self.sequence.checked_add(1).ok_or("Console frame identity exhausted")?;
        let identity = FrameIdentity {
            surface: SurfaceId::new(1)?,
            state_generation: Generation::new(1)?,
            sequence: Generation::new(self.sequence)?,
        };
        let job = client.submit_cpu(
            scope,
            JobSpec {
                id: jobs.allocate()?,
                operation: Operation::FramePrepare,
                class: JobClass::CpuInteractive,
                deadline: None,
                reservation_bytes: shape.byte_len() as u64
                    + ConsolePresentation::Native.storage_bytes(shape),
                replacement: Some(ReplacementKey {
                    surface: identity.surface,
                    revision: identity.sequence,
                }),
            },
            move |context| Preparation {
                shape,
                identity,
                lease: context.memory().clone(),
                validator: None,
            },
        );
        match job {
            Ok(job) => self.job = Some((layout, self.revision, job)),
            Err(
                AdmissionError::QueueSaturated { .. }
                | AdmissionError::CompletionSaturated
                | AdmissionError::MemorySaturated { .. },
            ) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }
    pub(super) fn view(&self, label: &'static str) -> Option<InstallerConsole<'_>> {
        let (layout, _, frame) = self.prepared.as_ref()?;
        let visible = self.layout.is_some_and(|current| current.visible && current == *layout);
        Some(InstallerConsole {
            frame,
            view: ConsoleView {
                label,
                bounds: layout.bounds,
                visible,
                dark: true,
                text: Some(self),
            },
        })
    }
    pub(super) fn settled(&self) -> bool {
        self.job.is_none()
    }
}
mod layout;
impl ConsoleText for Output {
    fn resource(&self) -> ResourceKey {
        self.key
    }
    fn revision(&self) -> u64 {
        self.revision
    }
    fn len(&self) -> usize {
        self.records.len()
    }
    fn record(&self, index: usize) -> Option<&TranscriptRecord> {
        self.records.get(index)
    }
}
struct Preparation {
    shape: FrameShape,
    identity: FrameIdentity,
    lease: MemoryLease,
    validator: Option<FrameValidator>,
}
impl CpuTask<PreparedFrame, io::Error> for Preparation {
    fn step(&mut self, _: &JobContext) -> io::Result<CpuStep<PreparedFrame>> {
        if self.validator.is_none() {
            let mut pixels = vec![16; self.shape.byte_len()];
            for pixel in pixels.chunks_exact_mut(4) {
                pixel[3] = 255;
            }
            self.validator = Some(
                FrameValidator::new(self.shape, self.identity, pixels, self.lease.clone())
                    .map_err(io::Error::other)?,
            );
        }
        if self
            .validator
            .as_mut()
            .ok_or_else(|| io::Error::other("Console frame absent"))?
            .validate_next(NonZeroUsize::MIN.saturating_add(65535))
            .map_err(io::Error::other)?
        {
            Ok(CpuStep::Complete(
                self.validator
                    .take()
                    .ok_or_else(|| io::Error::other("Console frame absent"))?
                    .finish()
                    .map_err(io::Error::other)?,
            ))
        } else {
            Ok(CpuStep::Yield)
        }
    }
}
