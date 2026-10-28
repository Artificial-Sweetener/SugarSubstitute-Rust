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
import type {
  CanvasSurfaceCommand,
  CanvasSurfaceHost,
  CanvasSurfaceResult,
} from '../contracts/CanvasSurface.js';
import type { CanvasSurfaceAddress } from '../contracts/CanvasSurfaceProtocol.generated.js';
import type {
  CanvasGeometryCommand,
  CanvasGeometryRequest,
  CanvasGeometryResult,
} from '../contracts/CanvasProtocol.generated.js';
/** Supply transport and deadlines from their owning runtime. */
export interface CanvasClientPort {
  send(message: CanvasMessage): void;
  later(milliseconds: number, callback: () => void): () => void;
  report(error: unknown): void;
}
interface Pending {
  readonly type:
    | 'context.reply'
    | 'image.reply'
    | 'source.reply'
    | 'geometry.reply'
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
  | CanvasSurfaceCommand;
/** Retain one correlated client for a validated host session; never replay mutations after reconnect. */
export class CanvasClient implements CanvasLayerHost {
  readonly geometry: CanvasGeometryHost;
  readonly sources: CanvasSourceConnection;
  readonly surface: CanvasSurfaceHost;
  readonly #pending = new Map<number, Pending>();
  readonly #sourceListeners = new Set<(change: CanvasSourceChange) => void>();
  #next = 0;
  #context: CanvasContext | null = null;
  #contextReply = 0;
  #disposed = false;
  /** Bind an authenticated session and injected clock without claiming its transport lifetime. */
  constructor(
    private readonly session: string,
    private readonly port: CanvasClientPort,
  ) {
    this.surface = {
      execute: (command, cancellation) => this.#surfaceRequest(command, cancellation),
    };
    this.geometry = {
      get canvasId() {
        return readIdentity();
      },
      execute: (command, cancellation) => this.#geometryRequest(command, cancellation),
    };
    const readIdentity = (): string => this.#context?.canvas.id ?? '';
    this.sources = new CanvasSourceConnection({
      identity: () => this.#context?.canvas.id ?? '',
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
  /** Read native context while keeping cached capabilities on the latest reply in this live session. */
  async readContext(): Promise<CanvasContext> {
    const reply = await this.#request({ type: 'context.request' }, 'context.reply');
    if (reply.type !== 'context.reply') throw new Error('Invalid canvas context reply.');
    if (this.#disposed) throw new Error('Canvas connection closed.');
    if (reply.requestId > this.#contextReply) {
      this.#contextReply = reply.requestId;
      this.#context = reply.context;
    }
    return reply.context;
  }
  /** Insert once using the caller's captured destination rather than a later active document. */
  async insertImage(
    image: CanvasInsertImage,
    cancellation?: CanvasCancellationSource,
  ): Promise<CanvasInsertResult> {
    if (cancellation?.cancelled())
      return { status: 'cancelled', message: 'Canvas insertion cancelled.' };
    const reply = await this.#request({ type: 'image.insert', image }, 'image.reply', cancellation);
    if (reply.type !== 'image.reply') throw new Error('Invalid canvas image reply.');
    return reply.result;
  }
  /** Accept only the expected reply kind from this session and retire its deadline before callbacks run. */
  receive(value: unknown): boolean {
    if (this.#disposed || !isCanvasMessage(value) || value.session !== this.session) return false;
    if (value.type === 'source.changed') {
      for (const listener of this.#sourceListeners) listener(value.change);
      return true;
    }
    const pending = this.#pending.get(value.requestId);
    if (!pending || (pending.type !== value.type && value.type !== 'error.reply')) return false;
    this.#pending.delete(value.requestId);
    pending.stop();
    if (value.type === 'error.reply') pending.reject(new Error(value.failure.message));
    else pending.accept(value);
    return true;
  }
  /** Refuse all late responses and cancel outstanding host work when the session leaves. */
  dispose(): void {
    if (this.#disposed) return;
    this.sources.dispose();
    this.#disposed = true;
    for (const pending of this.#pending.values()) {
      pending.stop();
      if (
        pending.type === 'image.reply' ||
        pending.type === 'geometry.reply' ||
        pending.type === 'surface.reply'
      )
        pending.cancel();
      pending.reject(new Error('Canvas connection closed.'));
    }
    this.#pending.clear();
    this.#context = null;
    this.#sourceListeners.clear();
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
  /** Delegate geometry and native sizing through the existing correlated host transport. */
  async #geometryRequest(
    command: CanvasGeometryCommand,
    cancellation?: CanvasCancellationSource,
  ): Promise<CanvasGeometryResult> {
    if (cancellation?.cancelled())
      return { kind: 'failure', status: 'cancelled', message: 'Canvas sizing cancelled.' };
    const reply = await this.#request(
      { type: 'geometry.request', command },
      'geometry.reply',
      cancellation,
    );
    if (reply.type !== 'geometry.reply') throw new Error('Invalid canvas geometry reply.');
    return reply.result;
  }
  /** Require the exact attachment and revision in the host's correlated surface reply. */
  async #surfaceRequest(
    command: CanvasSurfaceCommand,
    cancellation?: CanvasCancellationSource,
  ): Promise<CanvasSurfaceResult> {
    if (cancellation?.cancelled())
      return { status: 'cancelled', message: 'Canvas surface request cancelled.' };
    const address = Object.freeze({ ...command.address });
    const revision = command.revision;
    const type = command.type;
    const admitted = { ...command, address };
    const reply = await this.#request(admitted, 'surface.reply', cancellation);
    if (
      reply.type !== 'surface.reply' ||
      reply.revision !== revision ||
      !sameSurface(reply.address, address) ||
      (reply.status === 'ready' && type !== 'surface.prepare') ||
      (reply.status === 'applied' && type === 'surface.prepare')
    )
      throw new Error('Canvas surface reply does not match its attachment.');
    return { status: reply.status, message: reply.message };
  }
  /** Register reply ownership before synchronous transports can answer, and bound every request. */
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
      stopDeadline = this.port.later(30000, () => {
        this.#pending.delete(requestId);
        stop();
        if (
          request.type === 'image.insert' ||
          request.type === 'geometry.request' ||
          request.type.startsWith('surface.')
        )
          cancel();
        reject(new Error('Canvas request timed out; it was not retried.'));
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
          if (request.type === 'geometry.request') {
            resolve({
              protocol: 'sugarcubes.canvas',
              version: 3,
              session: this.session,
              requestId,
              type: 'geometry.reply',
              result: { kind: 'failure', status: 'cancelled', message: 'Canvas sizing cancelled.' },
            });
            return;
          }
          resolve({
            protocol: 'sugarcubes.canvas',
            version: 3,
            session: this.session,
            requestId,
            type: 'image.reply',
            result: { status: 'cancelled', message: 'Canvas insertion cancelled.' },
          });
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
        if (request.type.startsWith('surface.') && !isCanvasMessage(message))
          throw new Error('Canvas surface request is invalid.');
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
  return request.type.startsWith('surface.');
}

/** Compare every component rather than treating a public attachment label as authority. */
function sameSurface(left: CanvasSurfaceAddress, right: CanvasSurfaceAddress): boolean {
  return (
    left.sessionId === right.sessionId &&
    left.viewId === right.viewId &&
    left.windowId === right.windowId &&
    left.attachmentId === right.attachmentId &&
    left.generation === right.generation
  );
}
