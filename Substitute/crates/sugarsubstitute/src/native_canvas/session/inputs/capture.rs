// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Capture synced input documents, current or parked, as owned jobs.
//!
//! A capture reads every target from the one document revision current when it was requested,
//! renders each at one exact extent and encodes its reply on the same worker; the encoded bytes
//! stay charged to the canvas's retained lease until the host delivers them. Captures run one at
//! a time behind a bounded queue, and their outcomes wait in request order for the host.
use super::*;
use crate::native_canvas::work::capture::{self as render, CapturePixels};
use sugarsubstitute_canvas::CaptureLayers;
use sugarsubstitute_contracts::canvas_native::{CanvasNativeError, CanvasNativeReplyPayload};
use sugarsubstitute_execution::OwnedInputError;
use sugarsubstitute_image_ops::ImageError;

/// Captures that may wait behind the running one.
const MAX_QUEUED: usize = 8;
/// Fixed working memory of one capture job beyond its rasters: evaluation tiles, graph and
/// transfer table.
const WORKING_BYTES: u64 = 40 * 1024 * 1024;

/// What one captured target reads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CaptureTarget {
    /// The selection, as coverage bytes.
    Selection,
    /// The document's current layer: its active mask.
    CurrentLayer,
    /// One layer; a mask layer captures in the coverage image encoding.
    Layer(LayerId),
    /// The visible image layers composed.
    Composition,
    /// The visible image layers from the bottom up to and including this layer.
    CompositionFrom(LayerId),
}

/// One captured raster, in request order.
pub(crate) struct CapturedRaster {
    /// What the raster shows, which fixes its encoding.
    pub(crate) pixels: CapturePixels,
    /// Raster size in pixels.
    pub(crate) extent: Extent,
    /// Tightly packed row-major pixels.
    pub(crate) bytes: Vec<u8>,
}

/// Every raster of one capture, read from a single document revision.
pub(crate) struct InputCapture {
    /// The captured document.
    pub(crate) document: InputDocumentId,
    /// Its extent when captured.
    pub(crate) extent: Extent,
    /// The revision every raster shows.
    pub(crate) revision: u64,
    /// One raster per requested target, in order.
    pub(crate) rasters: Vec<CapturedRaster>,
}

/// Turns a finished capture into reply bytes on its worker.
pub(crate) type CaptureEncoder =
    Box<dyn FnOnce(InputCapture) -> Result<CanvasNativeReplyPayload, CanvasNativeError> + Send>;

/// Encoded reply bytes and the retained canvas memory covering them until delivery.
pub(crate) struct EncodedCapture {
    /// Schema-validated reply.
    pub(crate) payload: CanvasNativeReplyPayload,
    /// Charge for the payload's bytes.
    pub(crate) storage: MemoryClaim,
}

/// Completion handle of one capture; its outcome arrives through
/// [`Session::take_input_captures`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CaptureTicket(u64);

/// Why a capture cannot start; nothing was queued.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CaptureRejection {
    /// No synced, loaded document has this identity, a named layer is absent, or there is no
    /// current layer.
    Unavailable,
    /// The document's geometry differs, or workflow work is about to change it.
    Stale,
    /// Too many captures are waiting, or the canvas is closing.
    Busy,
}

/// The result of one capture.
pub(crate) struct CaptureOutcome {
    /// The capture this answers.
    pub(crate) ticket: CaptureTicket,
    /// Its encoded reply, or why it failed.
    pub(crate) result: Result<EncodedCapture, InputFailure>,
}

struct Queued {
    ticket: CaptureTicket,
    snapshot: DocumentSnapshot,
    sources: Vec<work::Source>,
    targets: Vec<(CapturePixels, Extent)>,
    encode: CaptureEncoder,
}

struct Running {
    ticket: CaptureTicket,
    job: OwnedJob<EncodedCapture, WorkError>,
}

/// Queued, running and finished captures.
#[derive(Default)]
pub(super) struct Captures {
    next: u64,
    queue: VecDeque<Queued>,
    running: Option<Running>,
    finished: VecDeque<CaptureOutcome>,
}

impl Captures {
    /// Whether a capture job awaits its terminal outcome.
    pub(super) const fn in_flight(&self) -> bool {
        self.running.is_some()
    }

    /// Whether queued captures still hold document content.
    pub(super) fn holds(&self) -> bool {
        !self.queue.is_empty()
    }

    /// Cancel the running capture and drop every queued one.
    pub(super) fn close(&mut self) {
        if let Some(running) = &self.running {
            running.job.cancel();
        }
        self.queue.clear();
    }
}

/// The extent a capture of `document` returns: the document itself, or with `edge` its longer
/// side scaled to at most `edge` pixels, each side `max(1, round(side × scale))`.
pub(crate) fn capture_extent(document: Extent, edge: Option<u32>) -> Result<Extent, ImageError> {
    let longest = document.width().max(document.height());
    let Some(edge) = edge.filter(|edge| *edge < longest) else {
        return Ok(document);
    };
    let scale = f64::from(edge) / f64::from(longest);
    let side = |value: u32| ((f64::from(value) * scale).round() as u32).clamp(1, value);
    Extent::new(side(document.width()), side(document.height()))
}

impl Session {
    /// Capture `targets` of a synced input document at `extent` and encode the reply with
    /// `encode` on the capture's worker. `geometry` is the document's geometry revision as its
    /// requester last saw it.
    ///
    /// A capture reads what the canvas shows: while a gesture stages a preview on the published
    /// revision, such as a brush stroke being drawn, it reads that preview, so card previews
    /// follow the stroke as the canvas does. The preview shares its published revision, which
    /// the capture reports.
    ///
    /// # Errors
    /// Rejects an unavailable or stale document, an absent layer or current layer, and a full
    /// queue or closing canvas with [`CaptureRejection`], queueing nothing.
    pub(crate) fn capture_input(
        &mut self,
        document: InputDocumentId,
        geometry: &str,
        targets: &[CaptureTarget],
        extent: Extent,
        encode: CaptureEncoder,
    ) -> Result<CaptureTicket, CaptureRejection> {
        if self.closing || self.inputs.captures.queue.len() >= MAX_QUEUED {
            return Err(CaptureRejection::Busy);
        }
        let published = match self.input_snapshot(document, None) {
            InputSnapshot::Current(snapshot) => snapshot,
            InputSnapshot::Stale => return Err(CaptureRejection::Stale),
            InputSnapshot::Unavailable => return Err(CaptureRejection::Unavailable),
        };
        if document.geometry(published.extent()) != geometry {
            return Err(CaptureRejection::Stale);
        }
        let state = self.input_document(document).ok_or(CaptureRejection::Unavailable)?;
        let snapshot = state
            .shown()
            .map(|(_, shown)| shown)
            .filter(|shown| shown.extent() == published.extent())
            .unwrap_or(published);
        let current = state.tools.target();
        let sources =
            state.scene.as_ref().and_then(|scene| scene.source.clone()).into_iter().collect();
        let targets = targets
            .iter()
            .map(|target| Ok((resolve(&snapshot, *target, current)?, extent)))
            .collect::<Result<_, CaptureRejection>>()?;
        let captures = &mut self.inputs.captures;
        captures.next = captures.next.wrapping_add(1);
        let ticket = CaptureTicket(captures.next);
        captures.queue.push_back(Queued { ticket, snapshot, sources, targets, encode });
        Ok(ticket)
    }

    /// Withdraw a queued or running capture; its outcome is never reported.
    pub(crate) fn cancel_input_capture(&mut self, ticket: CaptureTicket) {
        let captures = &mut self.inputs.captures;
        captures.queue.retain(|queued| queued.ticket != ticket);
        captures.finished.retain(|outcome| outcome.ticket != ticket);
        if let Some(running) = captures.running.as_ref().filter(|running| running.ticket == ticket)
        {
            running.job.cancel();
        }
    }

    /// Take every capture outcome since the last call, oldest first.
    pub(crate) fn take_input_captures(&mut self) -> Vec<CaptureOutcome> {
        self.inputs.captures.finished.drain(..).collect()
    }

    /// Start the oldest queued capture when none is running.
    pub(in crate::native_canvas::session) fn submit_input_capture(
        &mut self,
    ) -> Result<(), Box<dyn Error>> {
        if self.closing || self.inputs.captures.running.is_some() {
            return Ok(());
        }
        let Some(queued) = self.inputs.captures.queue.front() else { return Ok(()) };
        let rasters = queued
            .targets
            .iter()
            .map(|(pixels, extent)| extent.pixels().saturating_mul(pixels.bytes_per_pixel()))
            .fold(0_u64, u64::saturating_add);
        let sums = queued
            .targets
            .iter()
            .map(|(_, extent)| extent.pixels().saturating_mul(16))
            .max()
            .unwrap_or(0);
        // Rasters, their base64 text and the encoded reply are alive together while encoding.
        let reservation =
            rasters.saturating_mul(4).saturating_add(sums).saturating_add(WORKING_BYTES);
        let memory = self.resources.as_ref().ok_or("Canvas storage is unavailable")?.memory.clone();
        let spec = self.event_spec(
            Operation::Infrastructure,
            JobClass::InfrastructureOrdinary,
            reservation,
            None,
        )?;
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        let mut queued = self.inputs.captures.queue.pop_front();
        let ticket = queued.as_ref().map(|queued| queued.ticket).ok_or("Capture vanished")?;
        let run = move |queued: Queued, context: JobContext| capture(queued, &memory, &context);
        match runtime.client.submit_blocking_owned_input(&runtime.scope, spec, &mut queued, run) {
            Ok(job) => self.inputs.captures.running = Some(Running { ticket, job }),
            // Saturated admission keeps the capture first in line for a later poll.
            Err(OwnedInputError::Admission(cause)) if activity::retryable(&cause) => {
                let queued = queued.ok_or("A rejected capture lost its input")?;
                self.inputs.captures.queue.push_front(queued);
            }
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }

    /// Observe the running capture and keep its outcome for the host.
    pub(in crate::native_canvas::session) fn consume_input_capture(
        &mut self,
    ) -> Result<(), Box<dyn Error>> {
        let Some(completion) = self
            .inputs
            .captures
            .running
            .as_mut()
            .and_then(|running| running.job.try_take_completion())
        else {
            return Ok(());
        };
        let running = self.inputs.captures.running.take().ok_or("Capture identity was lost")?;
        Self::observe_terminal(&running.job, &completion.outcome);
        let result = match completion.outcome {
            JobOutcome::Success(encoded) => Ok(encoded),
            JobOutcome::Failure(cause) => Err(InputFailure::new(cause)),
            JobOutcome::TimedOut => Err(InputFailure::message("Capture exceeded its deadline")),
            JobOutcome::Panicked(_) => Err(InputFailure::message("Capture worker panicked")),
            _ => Err(InputFailure::message("Capture stopped before completion")),
        };
        if !self.closing {
            self.inputs
                .captures
                .finished
                .push_back(CaptureOutcome { ticket: running.ticket, result });
        }
        Ok(())
    }
}

/// What `target` reads from `snapshot`, whose current layer is `current`.
fn resolve(
    snapshot: &DocumentSnapshot,
    target: CaptureTarget,
    current: Option<LayerId>,
) -> Result<CapturePixels, CaptureRejection> {
    let layer = |id: LayerId| {
        snapshot.layers().iter().find(|layer| layer.id() == id).ok_or(CaptureRejection::Unavailable)
    };
    Ok(match target {
        CaptureTarget::Selection => CapturePixels::Selection,
        CaptureTarget::Composition => CapturePixels::Image(CaptureLayers::Composition),
        CaptureTarget::CompositionFrom(id) => {
            CapturePixels::Image(CaptureLayers::Through(layer(id)?.id()))
        }
        CaptureTarget::CurrentLayer | CaptureTarget::Layer(_) => {
            let id = match target {
                CaptureTarget::Layer(id) => id,
                _ => current.ok_or(CaptureRejection::Unavailable)?,
            };
            match layer(id)?.content() {
                sugarsubstitute_canvas::LayerContent::Coverage { .. } => CapturePixels::Mask(id),
                _ => CapturePixels::Image(CaptureLayers::Only(id)),
            }
        }
    })
}

/// Render and encode one capture on its worker.
fn capture(
    queued: Queued,
    memory: &MemoryLease,
    context: &JobContext,
) -> Result<EncodedCapture, WorkError> {
    let Queued { snapshot, sources, targets, encode, .. } = queued;
    let mut rasters = Vec::with_capacity(targets.len());
    for (pixels, extent) in targets {
        let bytes = render::render(&snapshot, &sources, pixels, extent, context)?;
        rasters.push(CapturedRaster { pixels, extent, bytes });
    }
    let capture = InputCapture {
        document: InputDocumentId(snapshot.identity()),
        extent: snapshot.extent(),
        revision: snapshot.revision(),
        rasters,
    };
    let payload = encode(capture).map_err(WorkError::new)?;
    let storage = memory.claim(payload.as_bytes().len() as u64)?;
    Ok(EncodedCapture { payload, storage })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_scale_the_longer_side_down_and_never_enlarge() -> Result<(), ImageError> {
        let extent = |width, height| Extent::new(width, height);
        let rows = [
            (extent(8, 6)?, None, extent(8, 6)?),
            (extent(8, 6)?, Some(3), extent(3, 2)?),
            (extent(8, 6)?, Some(8), extent(8, 6)?),
            (extent(8, 6)?, Some(1024), extent(8, 6)?),
            (extent(1000, 1)?, Some(10), extent(10, 1)?),
            (extent(3, 1000)?, Some(1), extent(1, 1)?),
            (extent(999, 333)?, Some(100), extent(100, 33)?),
        ];
        for (document, edge, expected) in rows {
            assert_eq!(capture_extent(document, edge)?, expected, "{document:?} at {edge:?}");
        }
        Ok(())
    }
}
