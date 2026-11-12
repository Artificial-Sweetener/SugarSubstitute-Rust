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
/** Correlate canvas requests without coupling callers to a browser or native editor. */
import type { CanvasCancellationSource, CanvasLayerHost } from '../contracts/CanvasHost.js';
import type {
  CanvasContext,
  CanvasDescriptor,
  CanvasInsertImage,
  CanvasInsertResult,
  CanvasMessage,
  CanvasReadRequest,
  CanvasInsertRequest,
  CanvasSourceRequest,
  CanvasSourceCommand,
  CanvasSourceResult,
  CanvasSourceChange,
} from '../contracts/CanvasProtocol.generated.js';
import { isCanvasMessage } from './CanvasValidation.generated.js';
import { CanvasSourceConnection } from './CanvasSourceConnection.js';
import type { CanvasGeometryHost } from '../contracts/CanvasGeometry.js';
import type { CanvasSurfaceCommand, CanvasSurfaceHost } from '../contracts/CanvasSurface.js';
import { requestCanvasSurface } from './CanvasSurfaceClient.js';
import {
  snapshotCanvasSurfaceCameraChange,
  snapshotCanvasSurfaceViewport,
} from './CanvasSurfaceViewport.js';
import { CanvasSurfaceConnection } from './CanvasSurfaceConnection.js';
import {
  canvasPreviewMessageFits,
  isCanvasPreviewCommand,
  snapshotCanvasSurfacePresentation,
  validCanvasSurfacePresentation,
} from './CanvasSurfacePreview.js';
import type {
  CanvasSurfaceCameraChange,
  CanvasSurfacePresentationChange,
} from '../contracts/CanvasSurface.js';
import { canvasSurfaceOverviewMessageFits } from './CanvasSurfaceOverview.js';
import {
  CanvasSurfaceDiscoveryFailure,
  snapshotCanvasSurfaceCapabilities,
} from './CanvasSurfaceDiscovery.js';
import { isRecord } from './ContractRecord.js';
import { NEWEST_CANVAS_SURFACE_SCHEMA } from './CanvasSurfaceSchemas.js';
import type {
  CanvasEditRequest,
  CanvasGeometryRequest,
  CanvasWorkflowRequest,
} from '../contracts/CanvasProtocol.generated.js';
import type { CanvasWorkflowHost } from '../contracts/CanvasWorkflowSurfaces.js';
import { CanvasEditConnection } from './CanvasEditConnection.js';
import { CANCELLED_INSERT, cancelledCanvasReply } from './CanvasCancelledReplies.js';
import {
  canvasEditCommands,
  canvasGeometryCommands,
  canvasWorkflowCommands,
  type CanvasCommandPort,
} from './CanvasCommandHosts.js';
/** Supply transport and deadlines from their owning runtime. */
export interface CanvasClientPort {
  send(message: CanvasMessage): void;
  later(milliseconds: number, callback: () => void): () => void;
  report(error: unknown): void;
  /**
   * Let every request wait for the host's own reply without a deadline.
   *
   * Set it only for a transport that settles every admitted request itself and observes its
   * peer's loss directly, such as the embedded native bridge: the host owns this page, answers
   * each request once, and the bridge fails whatever is still queued when the connection ends.
   * That bridge admits one request at a time, so a deadline there would measure the wait behind
   * a slow surface preparation rather than the host. A transport that can only infer a silent
   * peer's death from elapsed time leaves it unset.
   */
  readonly settlesRequests?: boolean;
}
interface Pending {
  readonly type:
    | 'context.reply'
    | 'image.reply'
    | 'source.reply'
    | 'geometry.reply'
    | 'edit.reply'
    | 'workflow.reply'
    | 'surface.capabilities.reply'
    | 'surface.viewport.reply'
    | 'surface.presentation.reply'
    | 'surface.reply';
  accept(message: CanvasMessage): void;
  reject(error: Error): void;
  stop(): void;
  cancel(): void;
}
type Request =
  | Pick<CanvasReadRequest, 'type'>
  | Pick<CanvasInsertRequest, 'type' | 'image'>
  | Pick<CanvasSourceRequest, 'type' | 'command'>
  | Pick<CanvasGeometryRequest, 'type' | 'command'>
  | Pick<CanvasEditRequest, 'type' | 'command'>
  | Pick<CanvasWorkflowRequest, 'type' | 'command'>
  | { readonly type: 'surface.capabilities.request'; readonly schema: number }
  | { readonly type: 'surface.presentation.request'; readonly schema: 1; readonly enabled: boolean }
  | CanvasSurfaceCommand;
/** Retain one correlated client for a validated host session; never replay mutations after reconnect. */
export class CanvasClient implements CanvasLayerHost {
  readonly geometry: CanvasGeometryHost;
  readonly sources: CanvasSourceConnection;
  readonly editing: CanvasEditConnection;
  readonly workflow: CanvasWorkflowHost;
  readonly #surfaces: CanvasSurfaceConnection;
  readonly #pending = new Map<number, Pending>();
  readonly #sourceListeners = new Set<(change: CanvasSourceChange) => void>();
  readonly #presentationListeners = new Set<(change: CanvasSurfacePresentationChange) => void>();
  readonly #cameraListeners = new Set<(change: CanvasSurfaceCameraChange) => void>();
  #presentationSequence = 0;
  #next = 0;
  #context: CanvasContext | null = null;
  #contextRevision: object = {};
  #disposed = false;
  /** Bind an authenticated session and injected clock without claiming its transport lifetime. */
  constructor(
    private readonly session: string,
    private readonly port: CanvasClientPort,
  ) {
    this.#surfaces = new CanvasSurfaceConnection({
      read: () =>
        this.#request(
          { type: 'surface.capabilities.request', schema: NEWEST_CANVAS_SURFACE_SCHEMA },
          'surface.capabilities.reply',
        ),
      execute: (command, cancellation) =>
        requestCanvasSurface(command, cancellation, (admitted, scope) =>
          this.#request(
            admitted,
            admitted.type === 'surface.viewport.request'
              ? 'surface.viewport.reply'
              : 'surface.reply',
            scope,
          ),
        ),
      watch: async (enabled) => {
        const reply = await this.#request(
          { type: 'surface.presentation.request', schema: 1, enabled },
          'surface.presentation.reply',
        );
        if (reply.type !== 'surface.presentation.reply')
          throw new Error('Invalid canvas presentation observation reply.');
        return reply;
      },
      presentation: (listener) => {
        this.#presentationListeners.add(listener);
        return () => {
          this.#presentationListeners.delete(listener);
        };
      },
      camera: (listener) => {
        this.#cameraListeners.add(listener);
        return () => {
          this.#cameraListeners.delete(listener);
        };
      },
      report: (error) => {
        this.port.report(error);
      },
    });
    const readIdentity = (): string => this.#context?.canvas.id ?? '';
    const commands: CanvasCommandPort = {
      identity: readIdentity,
      request: (request, reply, cancellation) => this.#request(request, reply, cancellation),
    };
    this.geometry = canvasGeometryCommands(commands);
    this.workflow = canvasWorkflowCommands(commands);
    this.editing = new CanvasEditConnection({
      identity: readIdentity,
      advertised: () => {
        const capabilities = this.#context?.canvas.capabilities;
        return !!capabilities && 'editing' in capabilities && capabilities.editing;
      },
      execute: canvasEditCommands(commands),
    });
    this.sources = new CanvasSourceConnection({
      identity: readIdentity,
      execute: (command) => this.#sourceRequest(command),
      listen: (listener) => {
        this.#sourceListeners.add(listener);
        return () => {
          this.#sourceListeners.delete(listener);
        };
      },
      report: (error) => {
        this.port.report(error);
      },
    });
  }
  /** Discover only capabilities actually reported by this canvas implementation. */
  readCanvas(): CanvasDescriptor | null {
    return this.#context?.canvas ?? null;
  }
  /** Expose the active native grant, including during a query-only refresh of the same canvas. */
  get surface(): CanvasSurfaceHost | null {
    return this.#surfaces.surface;
  }
  /** Capture a new mutation destination after immediately withdrawing the previous surface. */
  readContext(): Promise<CanvasContext> {
    this.withdrawSurface();
    return this.#readContext();
  }
  /** Query current host metadata without replacing an identical active surface or its frame. */
  refreshContext(): Promise<CanvasContext> {
    return this.#readContext();
  }
  /** Revoke active and pending surface grants at an explicit connection or navigation boundary. */
  withdrawSurface(): void {
    this.#contextRevision = {};
    this.#surfaces.withdraw();
  }
  /** Admit only the latest validated context and atomically negotiate its surface grant. */
  async #readContext(): Promise<CanvasContext> {
    const revision = {};
    this.#contextRevision = revision;
    const current = (): boolean => !this.#disposed && revision === this.#contextRevision;
    try {
      const reply = await this.#request({ type: 'context.request' }, 'context.reply');
      if (reply.type !== 'context.reply') throw new Error('Invalid canvas context reply.');
      this.#requireConnected();
      if (current()) {
        this.#context = reply.context;
        await this.#surfaces.refresh(reply.context.canvas, current);
      }
      this.#requireConnected();
      return reply.context;
    } catch (error: unknown) {
      if (current()) this.withdrawSurface();
      throw error;
    }
  }
  /** Recheck page teardown after each awaited discovery step. */
  #requireConnected(): void {
    if (this.#disposed) throw new Error('Canvas connection closed.');
  }
  /** Insert once using the caller's captured destination rather than a later active document. */
  async insertImage(
    image: CanvasInsertImage,
    cancellation?: CanvasCancellationSource,
  ): Promise<CanvasInsertResult> {
    if (cancellation?.cancelled()) return CANCELLED_INSERT;
    const reply = await this.#request({ type: 'image.insert', image }, 'image.reply', cancellation);
    if (reply.type !== 'image.reply') throw new Error('Invalid canvas image reply.');
    return reply.result;
  }
  /** Accept only the expected reply kind from this session and retire its deadline before callbacks run. */
  receive(value: unknown): boolean {
    if (this.#disposed) return false;
    if (!isCanvasMessage(value)) {
      this.#refuseDiscoveryReply(value);
      return false;
    }
    if (value.session !== this.session) return false;
    if (value.type === 'source.changed') {
      for (const listener of this.#sourceListeners) listener(value.change);
      return true;
    }
    if (value.type === 'edit.changed') return this.editing.publish(value.state);
    if (value.type === 'surface.presentation.changed') {
      if (
        !validCanvasSurfacePresentation(value.change) ||
        value.change.sequence <= this.#presentationSequence
      )
        return false;
      this.#presentationSequence = value.change.sequence;
      const change = snapshotCanvasSurfacePresentation(value.change);
      for (const listener of [...this.#presentationListeners])
        if (this.#presentationListeners.has(listener)) listener(change);
      return this.#presentationListeners.size > 0;
    }
    if (value.type === 'surface.camera.changed') {
      // Camera notifications share the presentation sequence, so either kind fences the other.
      if (value.change.sequence <= this.#presentationSequence) return false;
      this.#presentationSequence = value.change.sequence;
      const change = snapshotCanvasSurfaceCameraChange(value.change);
      for (const listener of [...this.#cameraListeners])
        if (this.#cameraListeners.has(listener)) listener(change);
      return this.#cameraListeners.size > 0;
    }
    const pending = this.#pending.get(value.requestId);
    const viewportFailure =
      pending?.type === 'surface.viewport.reply' &&
      value.type === 'surface.reply' &&
      value.status !== 'applied' &&
      value.status !== 'ready';
    if (
      !pending ||
      (pending.type !== value.type && value.type !== 'error.reply' && !viewportFailure)
    )
      return false;
    this.#pending.delete(value.requestId);
    pending.stop();
    if (value.type === 'error.reply') pending.reject(new Error(value.failure.message));
    else if (value.type === 'surface.capabilities.reply')
      pending.accept({
        ...value,
        capabilities: snapshotCanvasSurfaceCapabilities(value.capabilities),
      });
    else if (value.type === 'context.reply')
      pending.accept({ ...value, context: snapshotCanvasContext(value.context) });
    else if (value.type === 'surface.viewport.reply')
      pending.accept({
        ...value,
        address: Object.freeze({ ...value.address }),
        viewport: snapshotCanvasSurfaceViewport(value.viewport),
      });
    else pending.accept(value);
    return true;
  }
  /**
   * End an outstanding discovery at once when its correlated reply fails validation.
   *
   * An unreadable answer, such as one in a schema newer than this page requested, will not be
   * repaired by waiting, so the discovery fails now instead of at its deadline. Only a reply that
   * names this session and an outstanding discovery request is considered; anything else stays
   * an ignored message.
   */
  #refuseDiscoveryReply(value: unknown): void {
    if (
      !isRecord(value) ||
      value.protocol !== 'sugarcubes.canvas' ||
      value.session !== this.session ||
      value.type !== 'surface.capabilities.reply' ||
      typeof value.requestId !== 'number'
    )
      return;
    const pending = this.#pending.get(value.requestId);
    if (pending?.type !== 'surface.capabilities.reply') return;
    this.#pending.delete(value.requestId);
    pending.stop();
    pending.reject(
      new CanvasSurfaceDiscoveryFailure(
        'invalid-reply',
        'The canvas host answered surface discovery with a reply this page cannot read.',
      ),
    );
  }
  /** Refuse all late responses and cancel outstanding host work when the session leaves. */
  dispose(): void {
    if (this.#disposed) return;
    this.sources.dispose();
    this.editing.dispose();
    this.withdrawSurface();
    this.#disposed = true;
    for (const pending of this.#pending.values()) {
      pending.stop();
      // Source reads and observation acknowledgements end with the session; every other
      // request may own native work that the host must release.
      if (pending.type !== 'source.reply') pending.cancel();
      pending.reject(new Error('Canvas connection closed.'));
    }
    this.#pending.clear();
    this.#context = null;
    this.#sourceListeners.clear();
    this.#presentationListeners.clear();
  }
  /** Translate source query failures without introducing another transport or request sequence. */
  async #sourceRequest(command: CanvasSourceCommand): Promise<CanvasSourceResult> {
    try {
      const reply = await this.#request({ type: 'source.request', command }, 'source.reply');
      if (reply.type !== 'source.reply') throw new Error('Invalid canvas source reply.');
      return reply.result;
    } catch (error: unknown) {
      return {
        kind: 'failure',
        status: 'unavailable',
        message:
          error instanceof Error ? error.message.slice(0, 512) : 'Canvas source request failed.',
      };
    }
  }
  /** Register reply ownership before a synchronous transport answers; bound only unsettled transports. */
  #request(
    request: Request,
    reply: Pending['type'],
    cancellation?: CanvasCancellationSource,
  ): Promise<CanvasMessage> {
    if (this.#disposed) return Promise.reject(new Error('Canvas connection closed.'));
    if (this.#next === 2147483647)
      return Promise.reject(new Error('Canvas request identity exhausted.'));
    if (request.type.startsWith('surface.') && this.#pending.size >= 64)
      return Promise.reject(new Error('Canvas request capacity is exhausted.'));
    const requestId = ++this.#next;
    return new Promise((resolve, reject) => {
      let stopDeadline = () => {};
      let stopCancellation = () => {};
      let transportOwned = false;
      let cancellationSent = false;
      const stop = (): void => {
        stopDeadline();
        stopCancellation();
      };
      const cancel = (): void => {
        if (cancellationSent) return;
        cancellationSent = true;
        this.#cancel(requestId);
      };
      this.#pending.set(requestId, { type: reply, accept: resolve, reject, stop, cancel });
      if (this.port.settlesRequests !== true)
        stopDeadline = this.port.later(30000, () => {
          this.#pending.delete(requestId);
          stop();
          if (request.type !== 'source.request') cancel();
          reject(
            request.type === 'surface.capabilities.request'
              ? new CanvasSurfaceDiscoveryFailure(
                  'timed-out',
                  'Native surface schema 2 is unsupported or unavailable. Update SugarCubes and its native host together.',
                )
              : new Error('Canvas request timed out; it was not retried.'),
          );
        });
      stopCancellation =
        cancellation?.subscribe(() => {
          const pending = this.#pending.get(requestId);
          if (!pending) return;
          if (isSurfaceRequest(request) && transportOwned) {
            stopCancellation();
            cancel();
            return;
          }
          this.#pending.delete(requestId);
          stop();
          if (isSurfaceRequest(request)) {
            resolve({
              protocol: 'sugarcubes.canvas',
              version: 3,
              session: this.session,
              requestId,
              type: 'surface.reply',
              address: request.address,
              revision: request.revision,
              status: 'cancelled',
              message: 'Canvas surface request cancelled.',
            });
            return;
          }
          cancel();
          if (
            request.type === 'image.insert' ||
            request.type === 'geometry.request' ||
            request.type === 'edit.request' ||
            request.type === 'workflow.request'
          )
            resolve(cancelledCanvasReply(request.type, this.session, requestId));
          else reject(new Error('Canvas request cancelled.'));
        }) ?? (() => {});
      if (!this.#pending.has(requestId)) return;
      try {
        const envelope = {
          protocol: 'sugarcubes.canvas',
          version: 3,
          session: this.session,
          requestId,
        } as const;
        const message = { ...envelope, ...request };
        // Commands assembled by consumers are refused locally rather than by a host timeout.
        if (
          (request.type.startsWith('surface.') ||
            request.type === 'edit.request' ||
            request.type === 'workflow.request') &&
          !isCanvasMessage(message)
        )
          throw new Error(`Canvas ${request.type.split('.')[0] ?? ''} request is invalid.`);
        if (
          isSurfaceRequest(request) &&
          isCanvasPreviewCommand(request) &&
          !canvasPreviewMessageFits(message)
        )
          throw new Error('Canvas preview request exceeds the native transport bounds.');
        if (request.type === 'surface.overview' && !canvasSurfaceOverviewMessageFits(message))
          throw new Error('Canvas overview request exceeds the native transport bounds.');
        transportOwned = true;
        this.port.send(message);
      } catch (error: unknown) {
        this.#pending.delete(requestId);
        stop();
        reject(
          error instanceof Error ? error : new Error('Canvas transport failed.', { cause: error }),
        );
      }
    });
  }
  /** Signal cancellation without inventing a new mutation identity. */
  #cancel(requestId: number): void {
    try {
      this.port.send({
        protocol: 'sugarcubes.canvas',
        version: 3,
        session: this.session,
        requestId,
        type: 'cancel',
      });
    } catch (error: unknown) {
      this.port.report(error);
    }
  }
}

/** Distinguish requests whose cancellation completion is acknowledged by the native surface. */
function isSurfaceRequest(request: Request): request is CanvasSurfaceCommand {
  return (
    request.type.startsWith('surface.') &&
    request.type !== 'surface.capabilities.request' &&
    request.type !== 'surface.presentation.request'
  );
}

/** Retain admitted context metadata before an asynchronous consumer can observe caller mutation. */
function snapshotCanvasContext(context: CanvasContext): CanvasContext {
  return Object.freeze({
    canvas: Object.freeze({
      ...context.canvas,
      capabilities: Object.freeze({ ...context.canvas.capabilities }),
    }),
    document: context.document ? Object.freeze({ ...context.document }) : null,
  });
}
