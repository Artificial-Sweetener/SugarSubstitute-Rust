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
/** Coordinate peer canvas capabilities through one execution delivery and lifecycle authority. */
import { CanvasCancellation } from '../../core/embedded/CanvasRuntime.js';
import type { CanvasGeometryHost } from '../../../../../integrations/contracts/CanvasGeometry.js';
import type { CanvasOutputBinding } from '../../../../../integrations/contracts/CanvasAdapter.js';
import type {
  WorkflowCanvasAdapter,
  WorkflowCanvasOutput,
  WorkflowCanvasPreview,
  WorkflowCanvasState,
} from '../domain/WorkflowCanvas.js';
import type { WorkflowGenerationEvent } from '../domain/WorkflowGenerationEvent.js';
import type { WorkflowOrderedSubmission } from '../domain/WorkflowSubmission.js';
import type { PromptId, WorkflowSessionId } from '../domain/ExecutionIdentity.js';
import type { WorkflowGenerationEvents } from './WorkflowGenerationEvents.js';
import type { WorkflowResultStore } from './WorkflowResultStore.js';
import { WorkflowCanvasImages } from './WorkflowCanvasImages.js';
import type { CanvasMaskHost } from '../../../../../integrations/contracts/CanvasMasks.js';
import type { CanvasSourceHost } from '../../../../../integrations/contracts/CanvasSources.js';
import type { CanvasLayerHost } from '../../../../../integrations/contracts/CanvasHost.js';
import type { CanvasSurfaceHost } from '../../../../../integrations/contracts/CanvasSurface.js';
import type { CanvasEditHost } from '../../../../../integrations/contracts/CanvasEditing.js';
import type { CanvasWorkflowHost } from '../../../../../integrations/contracts/CanvasWorkflowSurfaces.js';

interface CanvasEntry<Bytes> {
  readonly adapter: WorkflowCanvasAdapter<Bytes>;
  tail: Promise<void>;
  active: boolean;
}
interface DestinationScope<Bytes> {
  readonly entry: CanvasEntry<Bytes>;
  readonly cancellation: CanvasCancellation;
}
type Destination<Bytes> = DestinationScope<Bytes> &
  (
    | {
        readonly ordering: 'immediate';
        readonly binding: {
          receive(output: WorkflowCanvasOutput, cancellation: CanvasCancellation): void;
        };
      }
    | {
        readonly ordering: 'serial';
        readonly binding: Promise<CanvasOutputBinding<WorkflowCanvasOutput> | null>;
      }
  );
interface CanvasRun<Bytes> {
  submission: WorkflowOrderedSubmission;
  readonly destinations: readonly Destination<Bytes>[];
  readonly images: WorkflowCanvasImages;
  pending: number;
  settled: boolean;
}

/** Route outputs, view restoration and previews by capability without identifying particular hosts. */
export class WorkflowCanvasCoordinator<Bytes> {
  readonly #entries = new Map<string, CanvasEntry<Bytes>>();
  readonly #submissions = new Map<string, CanvasRun<Bytes>>();
  readonly #runs = new Map<PromptId, CanvasRun<Bytes>>();
  readonly #stops: readonly (() => void)[];
  #disposed = false;

  /** Observe the sole execution authority and its current-view projection exactly once. */
  constructor(
    events: WorkflowGenerationEvents,
    private readonly results: WorkflowResultStore,
    private readonly report: (error: unknown) => void,
    private readonly targets: (submission: WorkflowOrderedSubmission) => readonly string[],
  ) {
    this.#stops = [
      events.subscribe((event) => this.#receive(event)),
      results.subscribe((sessionId, reason) => {
        if (reason !== 'output')
          this.#state({ sessionId, result: results.read(sessionId) }, reason === 'restore');
      }),
    ];
  }

  /** Register a peer; only view-capable adapters receive presentation restoration on attachment. */
  register(adapter: WorkflowCanvasAdapter<Bytes>): () => void {
    if (this.#disposed || this.#entries.has(adapter.id))
      throw new Error('Canvas is already registered or disposed.');
    const entry: CanvasEntry<Bytes> = { adapter, tail: Promise.resolve(), active: true };
    this.#entries.set(adapter.id, entry);
    for (const result of this.results.snapshots())
      this.#invoke(() =>
        adapter.capabilities.outputView?.restore({ sessionId: result.context.sessionId, result }),
      );
    return () => {
      entry.active = false;
      if (this.#entries.get(adapter.id) === entry) this.#entries.delete(adapter.id);
      for (const run of [...this.#submissions.values(), ...this.#runs.values()])
        for (const destination of run.destinations)
          if (destination.entry === entry) destination.cancellation.cancel();
    };
  }

  /** Resolve dimension editing independently of output delivery or source-raster capture. */
  geometryHost(canvasId?: string): CanvasGeometryHost | null {
    return this.#identifiedHost(({ geometry }) => geometry, canvasId);
  }

  /** Resolve source reads independently of output routing or mask interpretation. */
  sourceHost(canvasId?: string): CanvasSourceHost | null {
    return this.#identifiedHost(({ sources }) => sources, canvasId);
  }

  /** Resolve editing commands on a canvas that advertises them, independently of output routing. */
  editHost(canvasId?: string): CanvasEditHost | null {
    return this.#identifiedHost(({ editing }) => editing, canvasId);
  }

  /** Resolve workflow surface provisioning on a canvas that advertises it. */
  workflowHost(canvasId?: string): CanvasWorkflowHost | null {
    return this.#identifiedHost(({ workflow }) => workflow, canvasId);
  }

  /** Resolve the existing layer primitive by peer identity without exposing a particular editor. */
  layerHost(canvasId?: string): CanvasLayerHost | null {
    const hosts = [...this.#entries.values()]
      .flatMap(({ adapter }) => (adapter.capabilities.layers ? [adapter.capabilities.layers] : []))
      .filter((host) => {
        const descriptor = host.readCanvas();
        return descriptor && (!canvasId || descriptor.id === canvasId);
      });
    return hosts.length === 1 ? (hosts[0] ?? null) : null;
  }

  /** Resolve an unambiguous output surface by its registered adapter identity. */
  surfaceHost(adapterId?: string): CanvasSurfaceHost | null {
    const hosts = [...this.#entries.values()]
      .filter(({ adapter }) => !adapterId || adapter.id === adapterId)
      .flatMap(({ adapter }) => {
        const surface = adapter.capabilities.surface;
        return surface ? [surface] : [];
      });
    return hosts.length === 1 ? (hosts[0] ?? null) : null;
  }

  /** Resolve one unambiguous mask capability by canvas identity independently of output routing. */
  maskHost(canvasId?: string): CanvasMaskHost | null {
    return this.#identifiedHost(({ masks }) => masks, canvasId);
  }

  /** Select the sole identified host of one capability; ambiguity and anonymity resolve to null. */
  #identifiedHost<Host extends { readonly canvasId: string }>(
    select: (capabilities: WorkflowCanvasAdapter<Bytes>['capabilities']) => Host | undefined,
    canvasId: string | undefined,
  ): Host | null {
    const hosts = [...this.#entries.values()]
      .flatMap(({ adapter }) => {
        const host = select(adapter.capabilities);
        return host ? [host] : [];
      })
      .filter((host) => host.canvasId && (!canvasId || host.canvasId === canvasId));
    return hosts.length === 1 ? (hosts[0] ?? null) : null;
  }

  /** Deliver transient bytes synchronously only to interested preview-capable canvases. */
  preview(frame: WorkflowCanvasPreview<Bytes>): void {
    if (this.#disposed) return;
    const run = this.#runs.get(frame.source.run.promptId);
    if (!run) return;
    for (const { entry, cancellation } of run.destinations)
      this.#invoke(() => {
        if (!entry.active || cancellation.cancelled()) return;
        const capability = entry.adapter.capabilities.preview;
        if (capability?.wants(frame)) capability.receive(frame);
      });
  }

  /** Await admitted deliveries without a terminal-job barrier or a cross-canvas queue. */
  async idle(): Promise<void> {
    await Promise.all([...this.#entries.values()].map((entry) => entry.tail));
  }

  /** Cancel destination scopes before releasing subscriptions and adapter registrations. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    for (const stop of this.#stops) stop();
    for (const run of [...this.#submissions.values(), ...this.#runs.values()]) this.#cancel(run);
    for (const entry of this.#entries.values()) entry.active = false;
    this.#entries.clear();
    this.#submissions.clear();
    this.#runs.clear();
  }

  /** Apply generation lifecycle once for every participating canvas. */
  #receive(event: WorkflowGenerationEvent): void {
    if (this.#disposed) return;
    if (event.type === 'disposed') return this.dispose();
    if (event.type === 'submitted') return this.#capture(event.submission);
    if (event.type === 'rejected') {
      const key = this.#key(event.submission);
      const run = this.#submissions.get(key);
      if (run) this.#cancel(run);
      this.#submissions.delete(key);
      return;
    }
    if (event.type === 'accepted') {
      const key = this.#key(event.submission);
      const run = this.#submissions.get(key);
      this.#submissions.delete(key);
      if (run) {
        run.submission = event.run;
        this.#runs.set(event.run.promptId, run);
      }
      return;
    }
    if (event.type === 'closed') return this.#close(event.sessionId);
    const id = event.type === 'settled' ? event.promptId : event.run.promptId;
    const run = this.#runs.get(id);
    if (!run) return;
    if (event.type === 'output') {
      const output: WorkflowCanvasOutput = Object.freeze({
        ...(run.submission.canvasReturns ? { canvasReturns: run.submission.canvasReturns } : {}),
        run: event.run,
        source: event.source,
        images: run.images.read(event.run, event.source.id, event.payload.images),
        state: event.retained
          ? {
              sessionId: event.run.session.sessionId,
              result: this.results.read(event.run.session.sessionId),
            }
          : null,
      });
      for (const destination of run.destinations) this.#schedule(id, run, destination, output);
    } else if (event.type === 'terminal') run.settled = event.run.status !== 'complete';
    else run.settled = true;
    this.#release(id, run);
  }

  /** Capture all participating destinations at submission, before queue receipts or tab changes. */
  #capture(submission: WorkflowOrderedSubmission): void {
    const destinations: Destination<Bytes>[] = [];
    const targets = new Set(this.targets(submission));
    for (const entry of this.#entries.values()) {
      if (!targets.has(entry.adapter.id)) continue;
      const cancellation = new CanvasCancellation();
      try {
        const capability = entry.adapter.capabilities.output;
        if (capability.ordering === 'immediate') {
          const binding = capability.bind(submission, cancellation);
          if (binding) destinations.push({ entry, cancellation, ordering: 'immediate', binding });
        } else {
          const binding = capability.bind(submission, cancellation);
          if (binding === null) continue;
          destinations.push({
            entry,
            cancellation,
            ordering: 'serial',
            binding: Promise.resolve(binding).catch((error: unknown) => {
              if (!cancellation.cancelled()) this.report(error);
              return null;
            }),
          });
        }
      } catch (error: unknown) {
        this.report(error);
      }
    }
    this.#submissions.set(this.#key(submission), {
      submission,
      destinations,
      images: new WorkflowCanvasImages(),
      pending: 0,
      settled: false,
    });
  }

  /** Serialize each adapter independently so a slow or failed editor never stalls another canvas. */
  #schedule(
    id: PromptId,
    run: CanvasRun<Bytes>,
    destination: Destination<Bytes>,
    output: WorkflowCanvasOutput,
  ): void {
    const { entry, cancellation } = destination;
    if (destination.ordering === 'immediate') {
      if (entry.active && !cancellation.cancelled())
        this.#invoke(() => destination.binding.receive(output, cancellation));
      return;
    }
    run.pending++;
    entry.tail = entry.tail.then(async () => {
      try {
        if (!entry.active || cancellation.cancelled()) return;
        const binding = await destination.binding;
        if (binding && entry.active && !cancellation.cancelled())
          await binding.receive(output, cancellation);
      } catch (error: unknown) {
        if (!cancellation.cancelled()) this.report(error);
      } finally {
        run.pending--;
        this.#release(id, run);
      }
    });
  }

  /** Update lightweight view state without replaying finished outputs into document canvases. */
  #state(state: WorkflowCanvasState, restore: boolean): void {
    for (const { adapter } of this.#entries.values())
      this.#invoke(() => {
        const view = adapter.capabilities.outputView;
        if (restore) view?.restore(state);
        else view?.update(state);
      });
  }

  /** Cancel only scopes belonging to a closed or replaced workflow generation. */
  #close(sessionId: WorkflowSessionId): void {
    for (const [key, run] of this.#submissions)
      if (run.submission.session.sessionId === sessionId) {
        this.#cancel(run);
        this.#submissions.delete(key);
      }
    for (const [id, run] of this.#runs)
      if (run.submission.session.sessionId === sessionId) {
        this.#cancel(run);
        this.#runs.delete(id);
      }
  }
  /** End destination scopes without cancelling another canvas or workflow. */
  #cancel(run: CanvasRun<Bytes>): void {
    for (const destination of run.destinations) destination.cancellation.cancel();
  }
  /** Retire delivery identities after exact recovery and every admitted operation have settled. */
  #release(id: PromptId, run: CanvasRun<Bytes>): void {
    if (run.settled && run.pending === 0 && this.#runs.get(id) === run) this.#runs.delete(id);
  }
  /** Isolate adapter callback failures at the shared boundary. */
  #invoke(action: () => void): void {
    try {
      action();
    } catch (error: unknown) {
      this.report(error);
    }
  }
  /** Preserve independent overlapping submission identities before backend acceptance. */
  #key(submission: WorkflowOrderedSubmission): string {
    return JSON.stringify([
      submission.session.sessionId,
      submission.session.generation,
      submission.order,
    ]);
  }
}
