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
/** Supply transport and deadlines from their owning runtime. */
export interface CanvasClientPort {
  send(message: CanvasMessage): void;
  later(milliseconds: number, callback: () => void): () => void;
  report(error: unknown): void;
}
interface Pending {
  readonly type: 'context.reply' | 'image.reply' | 'source.reply';
  accept(message: CanvasMessage): void;
  reject(error: Error): void;
  stop(): void;
}
type Request =
  | Pick<CanvasReadRequest, 'type'>
  | Pick<CanvasInsertRequest, 'type' | 'image'>
  | Pick<CanvasSourceRequest, 'type' | 'command'>;
/** Retain one correlated client for a validated host session; never replay mutations after reconnect. */
export class CanvasClient implements CanvasLayerHost {
  readonly sources: CanvasSourceConnection;
  readonly #pending = new Map<number, Pending>();
  readonly #sourceListeners = new Set<(change: CanvasSourceChange) => void>();
  #next = 0;
  #context: CanvasContext | null = null;
  #disposed = false;
  /** Bind an authenticated session and injected clock without claiming its transport lifetime. */
  constructor(
    private readonly session: string,
    private readonly port: CanvasClientPort,
  ) {
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
  /** Read the current document before preparing an insertion against that exact identity. */
  async readContext(): Promise<CanvasContext> {
    const reply = await this.#request({ type: 'context.request' }, 'context.reply');
    if (reply.type !== 'context.reply') throw new Error('Invalid canvas context reply.');
    this.#context = reply.context;
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
    for (const [id, pending] of this.#pending) {
      pending.stop();
      if (pending.type === 'image.reply') this.#cancel(id);
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
  /** Register reply ownership before synchronous transports can answer, and bound every request. */
  #request(
    request: Request,
    reply: Pending['type'],
    cancellation?: CanvasCancellationSource,
  ): Promise<CanvasMessage> {
    if (this.#disposed) return Promise.reject(new Error('Canvas connection closed.'));
    const requestId = ++this.#next;
    return new Promise((resolve, reject) => {
      let stopDeadline = () => {};
      let stopCancellation = () => {};
      const stop = (): void => {
        stopDeadline();
        stopCancellation();
      };
      this.#pending.set(requestId, { type: reply, accept: resolve, reject, stop });
      stopDeadline = this.port.later(30000, () => {
        this.#pending.delete(requestId);
        stop();
        if (request.type === 'image.insert') this.#cancel(requestId);
        reject(new Error('Canvas request timed out; it was not retried.'));
      });
      stopCancellation =
        cancellation?.subscribe(() => {
          const pending = this.#pending.get(requestId);
          if (!pending) return;
          this.#pending.delete(requestId);
          stop();
          this.#cancel(requestId);
          resolve({
            protocol: 'sugarcubes.canvas',
            version: 2,
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
          version: 2,
          session: this.session,
          requestId,
        } as const;
        this.port.send({ ...envelope, ...request });
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
        version: 2,
        session: this.session,
        requestId,
        type: 'cancel',
      });
    } catch (error: unknown) {
      this.port.report(error);
    }
  }
}
