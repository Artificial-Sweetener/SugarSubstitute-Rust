//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Retain encoded preview custody and observed pixels without another native mutation scheduler. */
import type {
  CanvasSurfaceHost,
  CanvasSurfacePresentationChange,
  CanvasSurfacePresentationDisplay,
  CanvasSurfaceResult,
} from '../../../../../integrations/contracts/CanvasSurface.js';
import type { CanvasChangeSubscription } from '../../../../../integrations/contracts/CanvasChangeObservation.js';
import {
  CANVAS_PREVIEW_CHUNK_BYTES,
  CANVAS_PREVIEW_LEASE_MILLISECONDS,
  MAX_CANVAS_IMAGE_BYTES,
  encodeCanvasBytes,
} from '../../core/embedded/CanvasRuntime.js';
import type { WorkflowPreview } from '../../execution/domain/WorkflowPreview.js';
import type { ExecutionClock } from '../../execution/domain/WorkflowExecutionEvent.js';
import { canvasPreviewLane } from '../application/CanvasDisplayPlan.js';
import type { CanvasAttachmentAddress } from '../domain/CanvasIdentity.js';
import type { NativeCanvasOperation } from './NativeCanvasBinding.js';

type PreviewOperation = Extract<NativeCanvasOperation, { type: `surface.preview.${string}` }>;
type Phase = 'begin' | 'chunk' | 'end' | 'awaiting' | 'withdraw';
interface Frame {
  readonly preview: WorkflowPreview;
  blob: Blob | null;
  phase: Phase;
  leaseRevision: number | null;
  offset: number;
  data: string | null;
  reading: boolean;
  sending: boolean;
  withdrawing: boolean;
  submitted: boolean;
  stop: (() => void) | null;
}

/** Describe the absence of local preview custody, matching the shape of active diagnostics. */
export const IDLE_NATIVE_CANVAS_PREVIEW_DIAGNOSTICS = Object.freeze({
  queuedInputs: 0,
  previewDrops: 0,
  estimatedBytes: 0,
  activeDeadlines: 0,
});

/** One custody operation offered to the existing presentation loop. */
export interface NativeCanvasPreviewStep {
  readonly operation: PreviewOperation;
  started(revision: number): void;
  settled(result: CanvasSurfaceResult): void;
  failed(error: unknown): void;
}

interface Options {
  readonly address: CanvasAttachmentAddress;
  readonly host: CanvasSurfaceHost;
  readonly clock: ExecutionClock;
  active(): boolean;
  finalRevision(): number | null;
  changed(): void;
  cancelOperation(): void;
  displayed(value: CanvasSurfacePresentationDisplay | null, presentation: object | null): void;
  report(error: unknown): void;
}

/** Own one active transfer, its latest pending Blob, and one host observation lifetime. */
export class NativeCanvasPreview {
  #subscription: CanvasChangeSubscription | null = null;
  #observing = false;
  #disposed = false;
  #selecting = false;
  #interest: WorkflowPreview | null = null;
  #active: Frame | null = null;
  #pending: { preview: WorkflowPreview; blob: Blob } | null = null;
  #shown: { preview: WorkflowPreview; leaseRevision: number } | null = null;
  #displayedPreview = false;
  #sequence = -1;
  #epoch = 0;
  #drops = 0;

  /** Negotiate observation before admitting any preview bytes or custody mutations. */
  constructor(private readonly options: Options) {
    if (!options.host.capabilities.previewFormats.includes('jpeg') || !options.host.presentation)
      return;
    try {
      const subscription = options.host.presentation.subscribe((change) => this.#observe(change));
      this.#subscription = subscription;
      void subscription.ready.then((status) => {
        if (this.#disposed || this.#subscription !== subscription) return;
        this.#observing = status === 'observing';
        this.options.changed();
      }, options.report);
    } catch (error: unknown) {
      options.report(error);
    }
  }

  /** Read accepted observation support without granting sibling preparation byte authority. */
  get observing(): boolean {
    return this.#observing && !this.#disposed;
  }
  /** Only actual accepted native pixels lock the shared controls. */
  get displayed(): boolean {
    return this.#displayedPreview;
  }
  /** Describe pending native preparation without treating it as presented pixels. */
  get preparing(): boolean {
    return !!this.#active || !!this.#pending;
  }
  /** Correlate only complete transfers acknowledged by native custody and still awaiting pixels. */
  get pendingPresentation(): object | null {
    return this.#active?.submitted ? this.#active : null;
  }
  /** Fence viewport replies that predate a native display transition. */
  get epoch(): number {
    return this.#epoch;
  }

  /** Reconcile lightweight logical interest while preserving an active frame in the same run. */
  update(preview: WorkflowPreview | null): boolean {
    if (this.#disposed) return false;
    const changed =
      preview?.token !== this.#interest?.token ||
      canvasLane(preview) !== canvasLane(this.#interest);
    if (canvasLane(preview) !== canvasLane(this.#interest)) {
      this.#pending = null;
      this.#withdraw();
    }
    this.#interest = preview;
    return changed;
  }

  /** Refuse bytes until this exact committed attachment has a qualified workflow preview. */
  wants(): boolean {
    return this.observing && this.options.active() && !this.#selecting && !!this.#interest;
  }

  /** Retain encoded bytes only; no image, URL, image loader or browser decoder is created. */
  receive(preview: WorkflowPreview, blob: Blob): void {
    if (
      !this.wants() ||
      preview.token !== this.#interest?.token ||
      canvasLane(preview) !== canvasLane(this.#interest) ||
      blob.type !== 'image/jpeg' ||
      blob.size < 1 ||
      blob.size > MAX_CANVAS_IMAGE_BYTES
    )
      return;
    if (
      this.#active?.preview.token === preview.token ||
      this.#pending?.preview.token === preview.token ||
      this.#shown?.preview.token === preview.token
    )
      return;
    if (this.#pending) this.#drops++;
    this.#pending = { preview, blob };
    this.options.changed();
  }

  /** Select already fences native preview custody; avoid withdrawing back to the previous final. */
  selecting(): void {
    this.#selecting = true;
    if (this.#active) {
      this.#active.stop?.();
      this.#active.stop = null;
      this.#active.blob = null;
      this.#active.data = null;
      this.#active.phase = 'awaiting';
    }
    this.#pending = null;
    this.#epoch++;
  }

  /** Reopen admission after any selection outcome while preserving observed lease correlation. */
  selectionFinished(): void {
    this.#selecting = false;
  }

  /** A successful final selection acknowledges the newly accepted native final frame. */
  selected(): void {
    this.#clearActive();
    this.#shown = null;
    const wasPreview = this.#displayedPreview;
    this.#displayedPreview = false;
    if (wasPreview) this.options.displayed(null, null);
  }

  /** Offer at most one ordinary operation; asynchronous Blob reads consume no mutation credit. */
  next(): NativeCanvasPreviewStep | null {
    if (!this.observing || !this.options.active() || this.#selecting) return null;
    if (!this.#active && this.#pending) {
      const pending = this.#pending;
      this.#pending = null;
      this.#active = {
        ...pending,
        phase: 'begin',
        leaseRevision: null,
        offset: 0,
        data: null,
        reading: false,
        sending: false,
        withdrawing: false,
        submitted: false,
        stop: null,
      };
    }
    const frame = this.#active;
    if (
      !frame ||
      frame.sending ||
      (frame.reading && !frame.withdrawing) ||
      frame.phase === 'awaiting'
    )
      return null;
    if (frame.phase === 'chunk' && !frame.data) {
      this.#read(frame);
      return null;
    }
    const operation = this.#operation(frame);
    if (!operation) return null;
    frame.sending = true;
    return {
      operation,
      started: (revision) => {
        if (operation.type !== 'surface.preview.begin') return;
        frame.leaseRevision = revision;
        frame.stop = this.options.clock.later(CANVAS_PREVIEW_LEASE_MILLISECONDS, () => {
          if (this.#active === frame) {
            this.#withdraw();
            this.options.changed();
          }
        });
      },
      settled: (result) => this.#settled(frame, operation, result),
      failed: (error) => {
        this.options.report(error);
        this.#settled(frame, operation, { status: 'error', message: 'Preview transfer failed.' });
      },
    };
  }

  /** Describe retained encoded custody while reporting no browser image allocations. */
  diagnostics() {
    return {
      queuedInputs: Number(!!this.#pending),
      previewDrops: this.#drops,
      estimatedBytes: (this.#active?.blob?.size ?? 0) + (this.#pending?.blob.size ?? 0),
      activeDeadlines: Number(!!this.#active?.stop),
    };
  }

  /** Fence callbacks and release local byte references before the binding releases native custody. */
  dispose(): void {
    this.#disposed = true;
    this.#observing = false;
    this.#subscription?.stop();
    this.#subscription = null;
    this.#clearActive();
    this.#pending = null;
    this.#shown = null;
    this.#interest = null;
    this.#displayedPreview = false;
    this.#epoch++;
  }

  /** Read one bounded immutable slice and discard late completion after replacement or retirement. */
  #read(frame: Frame): void {
    if (!frame.blob) return;
    frame.reading = true;
    const end = Math.min(frame.offset + CANVAS_PREVIEW_CHUNK_BYTES, frame.blob.size);
    void frame.blob
      .slice(frame.offset, end)
      .arrayBuffer()
      .then((buffer) => {
        frame.reading = false;
        if (this.#disposed || this.#active !== frame || frame.withdrawing) return;
        if (buffer.byteLength !== end - frame.offset)
          throw new Error('Preview slice length changed.');
        frame.data = encodeCanvasBytes(new Uint8Array(buffer));
        this.options.changed();
      })
      .catch((error: unknown) => {
        frame.reading = false;
        this.options.report(error);
        if (this.#active === frame) {
          this.#withdraw();
          this.options.changed();
        }
      });
  }

  /** Construct only the next cursor step; the presentation owner assigns its ordinary revision. */
  #operation(frame: Frame): PreviewOperation | null {
    const token = frame.preview.token;
    if (frame.phase === 'begin' && frame.blob) {
      const { generation, promptId, sourceId, runOrder, sequence } = frame.preview;
      return {
        type: 'surface.preview.begin',
        preview: { generation, promptId, sourceId, runOrder, sequence, token },
        byteLength: frame.blob.size,
        format: 'jpeg',
      };
    }
    if (frame.leaseRevision === null) return null;
    const leaseRevision = frame.leaseRevision;
    if (frame.phase === 'chunk' && frame.data)
      return {
        type: 'surface.preview.chunk',
        token,
        leaseRevision,
        offset: frame.offset,
        data: frame.data,
      };
    if (frame.phase === 'end' || frame.phase === 'withdraw')
      return {
        type: frame.phase === 'end' ? 'surface.preview.end' : 'surface.preview.withdraw',
        token,
        leaseRevision,
      };
    return null;
  }

  /** Acknowledgements advance byte custody, never the displayed-preview flag. */
  #settled(frame: Frame, operation: PreviewOperation, result: CanvasSurfaceResult): void {
    if (this.#active !== frame || this.#disposed) return;
    frame.sending = false;
    if (operation.type === 'surface.preview.end' && result.status === 'applied')
      frame.submitted = true;
    // A refused withdraw means native custody no longer holds this lease, so no observation follows it.
    if (operation.type === 'surface.preview.withdraw') {
      if (result.status === 'applied') frame.phase = 'awaiting';
      else this.#clearActive();
    } else if (operation.type === 'surface.preview.begin' && result.status !== 'applied')
      this.#clearActive();
    else if (result.status !== 'applied' || frame.withdrawing) this.#withdraw();
    else if (operation.type === 'surface.preview.begin') frame.phase = 'chunk';
    else if (operation.type === 'surface.preview.chunk') {
      frame.offset += Math.min(CANVAS_PREVIEW_CHUNK_BYTES, (frame.blob?.size ?? 0) - frame.offset);
      frame.data = null;
      frame.phase = frame.offset === frame.blob?.size ? 'end' : 'chunk';
    } else {
      frame.blob = null;
      frame.phase = 'awaiting';
    }
    this.options.changed();
  }

  /** Withdraw the exact current lease while retaining truthful displayed state until observation. */
  #withdraw(): void {
    if (!this.#active && this.#shown) {
      this.#active = {
        preview: this.#shown.preview,
        leaseRevision: this.#shown.leaseRevision,
        blob: null,
        phase: 'withdraw',
        offset: 0,
        data: null,
        reading: false,
        sending: false,
        withdrawing: true,
        submitted: false,
        stop: null,
      };
    }
    const frame = this.#active;
    if (!frame) return;
    if (frame.withdrawing && frame.phase === 'awaiting') return;
    frame.stop?.();
    frame.stop = null;
    frame.blob = null;
    frame.data = null;
    frame.withdrawing = true;
    if (frame.leaseRevision === null) this.#clearActive();
    else {
      frame.phase = 'withdraw';
      if (frame.sending) this.options.cancelOperation();
    }
  }

  /** Release encoded custody and its initial deadline exactly once. */
  #clearActive(): void {
    if (this.#active) {
      this.#active.stop?.();
      this.#active.blob = null;
      this.#active.data = null;
    }
    this.#active = null;
  }

  /** Accept only monotonic observations from this binding and its current candidate/display leases. */
  #observe(change: CanvasSurfacePresentationChange): void {
    if (
      !this.observing ||
      !this.options.active() ||
      (['sessionId', 'viewId', 'windowId', 'attachmentId', 'generation'] as const).some(
        (key) => change.address[key] !== this.options.address[key],
      ) ||
      change.sequence <= this.#sequence
    )
      return;
    const matches = (
      value: { preview: WorkflowPreview; leaseRevision: number | null } | null,
      token: string,
      revision: number,
    ) => value?.preview.token === token && value.leaseRevision === revision;
    if (
      !matches(this.#active, change.token, change.leaseRevision) &&
      !matches(this.#shown, change.token, change.leaseRevision)
    )
      return;
    const display = change.displayed;
    if (
      display.type === 'preview' &&
      !matches(this.#active, display.token, display.leaseRevision) &&
      !matches(this.#shown, display.token, display.leaseRevision)
    )
      return;
    if (
      (display.type === 'final' || display.type === 'overview') &&
      display.selectionRevision !== this.options.finalRevision()
    )
      return;
    if (display.type === 'empty' && this.options.finalRevision() !== null) return;
    this.#sequence = change.sequence;
    this.#epoch++;
    const active = this.#active;
    const presentation =
      active?.submitted && matches(active, change.token, change.leaseRevision) ? active : null;
    if (
      change.status === 'presented' &&
      active &&
      active.leaseRevision !== null &&
      matches(active, change.token, change.leaseRevision)
    )
      this.#shown = { preview: active.preview, leaseRevision: active.leaseRevision };
    if (
      active &&
      matches(active, change.token, change.leaseRevision) &&
      !(change.status === 'presented' && active.withdrawing)
    )
      this.#clearActive();
    if (display.type !== 'preview') this.#shown = null;
    this.#displayedPreview = display.type === 'preview';
    this.options.displayed(display, presentation);
    if (change.status === 'failed' && change.message)
      this.options.report(new Error(change.message));
    this.options.changed();
  }
}

/** Compare run/source identity independently of transport sequence and measured dimensions. */
function canvasLane(preview: WorkflowPreview | null): string | null {
  return preview ? canvasPreviewLane(preview) : null;
}
