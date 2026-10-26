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
/** Dispatch authenticated canvas requests with bounded at-most-once mutation ownership. */
import type {
  CanvasContext,
  CanvasInsertImage,
  CanvasInsertResult,
  CanvasMessage,
  CanvasSourceRequest,
  CanvasSourceResult,
} from '../contracts/CanvasProtocol.generated.js';
import type { CanvasSourceHost } from '../contracts/CanvasSources.js';
import type { CanvasGeometryHost } from '../contracts/CanvasGeometry.js';
import type {
  CanvasGeometryRequest,
  CanvasGeometryResult,
} from '../contracts/CanvasProtocol.generated.js';
import { CanvasCancellation } from './CanvasCancellation.js';
import { isCanvasMessage } from './CanvasValidation.generated.js';
import { MAX_CANVAS_ENCODED_CHARACTERS } from './CanvasImageEncoding.js';
/** Supply editor capabilities through typed ports rather than importing native APIs. */
export interface CanvasServerPort {
  readonly geometry?: CanvasGeometryHost;
  readonly sources?: CanvasSourceHost;
  context(): CanvasContext;
  insert(image: CanvasInsertImage, cancellation: CanvasCancellation): Promise<CanvasInsertResult>;
  send(message: CanvasMessage): void;
  report(error: unknown): void;
}
interface Entry {
  readonly fingerprint: string;
  readonly cancellation: CanvasCancellation;
  readonly imageData: string;
  reply: CanvasMessage | null;
}
/** Keep replay protection within one live session and refuse forgotten mutation identities. */
export class CanvasServer {
  readonly #entries = new Map<number, Entry>();
  #highest = 0;
  #disposed = false;
  #retainedCharacters = 0;
  #stopSources: (() => void) | null = null;
  /** Bind one navigation's session and its capability implementation. */
  constructor(
    private readonly session: string,
    private readonly port: CanvasServerPort,
  ) {}
  /** Validate before dispatch, including duplicate payload identity and cancellation ownership. */
  receive(value: unknown): boolean {
    if (this.#disposed || !isCanvasMessage(value) || value.session !== this.session) return false;
    if (value.type === 'cancel') {
      this.#entries.get(value.requestId)?.cancellation.cancel();
      return true;
    }
    if (
      value.type !== 'context.request' &&
      value.type !== 'image.insert' &&
      value.type !== 'geometry.request' &&
      value.type !== 'source.request'
    )
      return false;
    const fingerprint =
      value.type === 'context.request'
        ? value.type
        : value.type === 'source.request' || value.type === 'geometry.request'
          ? JSON.stringify([value.type, value.command])
          : JSON.stringify([
              value.type,
              value.image.document.id,
              value.image.document.geometryRevision,
              value.image.document.width,
              value.image.document.height,
              value.image.image.format,
              value.image.image.encoding,
              value.image.layerName,
              value.image.position ?? null,
            ]);
    const existing = this.#entries.get(value.requestId);
    const imageData = value.type === 'image.insert' ? value.image.image.data : '';
    if (existing) {
      if (existing.fingerprint !== fingerprint || existing.imageData !== imageData) {
        this.#failure(value.requestId, 'stale', 'Canvas request identity was reused.');
        return true;
      }
      if (existing.reply) this.#send(existing.reply);
      return true;
    }
    const retained = imageData.length + fingerprint.length;
    this.#trim(retained);
    if (
      value.requestId <= this.#highest ||
      this.#entries.size >= 64 ||
      this.#retainedCharacters + retained > MAX_CANVAS_ENCODED_CHARACTERS
    ) {
      this.#failure(value.requestId, 'stale', 'Canvas request is stale or the host is busy.');
      return true;
    }
    this.#highest = value.requestId;
    const entry: Entry = {
      fingerprint,
      imageData,
      cancellation: new CanvasCancellation(),
      reply: null,
    };
    this.#retainedCharacters += retained;
    this.#entries.set(value.requestId, entry);
    if (value.type === 'context.request') {
      try {
        this.#reply(entry, {
          ...this.#envelope(value.requestId),
          type: 'context.reply',
          context: this.port.context(),
        });
      } catch (error: unknown) {
        this.port.report(error);
        this.#reply(entry, {
          ...this.#envelope(value.requestId),
          type: 'error.reply',
          failure: { status: 'error', message: 'The canvas context is unavailable.' },
        });
      }
    } else if (value.type === 'geometry.request') void this.#geometry(value, entry);
    else if (value.type === 'source.request') void this.#source(value, entry);
    else void this.#insert(value, entry);
    return true;
  }
  /** Cancel owned operations and refuse responses after the native panel is destroyed. */
  dispose(): void {
    this.#disposed = true;
    this.#stopSources?.();
    this.#stopSources = null;
    for (const entry of this.#entries.values()) entry.cancellation.cancel();
    this.#entries.clear();
    this.#retainedCharacters = 0;
  }
  /** Dispatch negotiated document geometry with the same replay and cancellation ownership as insertion. */
  async #geometry(request: CanvasGeometryRequest, entry: Entry): Promise<void> {
    let result: CanvasGeometryResult;
    try {
      result = this.port.geometry
        ? await this.port.geometry.execute(request.command, entry.cancellation)
        : {
            kind: 'failure',
            status: 'unsupported',
            message: 'This canvas does not support geometry operations.',
          };
    } catch (error: unknown) {
      this.port.report(error);
      result = {
        kind: 'failure',
        status: 'error',
        message: 'The canvas geometry operation failed.',
      };
    }
    this.#reply(entry, { ...this.#envelope(request.requestId), type: 'geometry.reply', result });
  }
  /** Dispatch optional read capabilities and scope native observation to this authenticated session. */
  async #source(request: CanvasSourceRequest, entry: Entry): Promise<void> {
    let result: CanvasSourceResult;
    const source = this.port.sources;
    try {
      if (!source) {
        result =
          request.command.type === 'context'
            ? {
                kind: 'context',
                supported: false,
                document: this.port.context().document,
                revision: 'unsupported',
                targets: [],
                layers: [],
                currentLayerId: null,
              }
            : {
                kind: 'failure',
                status: 'unsupported',
                message: 'This canvas does not support source capture.',
              };
      } else if (request.command.type === 'watch') {
        if (request.command.enabled && !this.#stopSources)
          this.#stopSources = source.subscribe((change) => {
            if (this.#disposed) return;
            this.#send({
              protocol: 'sugarcubes.canvas',
              version: 2,
              session: this.session,
              requestId: 0,
              type: 'source.changed',
              change,
            });
          });
        else if (!request.command.enabled) {
          this.#stopSources?.();
          this.#stopSources = null;
        }
        result = { kind: 'watching' };
      } else result = await source.execute(request.command);
    } catch (error: unknown) {
      this.port.report(error);
      result = {
        kind: 'failure',
        status: 'error',
        message: 'The canvas could not capture this source.',
      };
    }
    this.#reply(entry, { ...this.#envelope(request.requestId), type: 'source.reply', result });
    if (result.kind === 'captured' && this.#entries.get(request.requestId) === entry) {
      this.#entries.delete(request.requestId);
      this.#retainedCharacters -= entry.imageData.length + entry.fingerprint.length;
    }
  }
  /** Keep native exceptions actionable while returning a stable public error contract. */
  async #insert(
    request: Extract<CanvasMessage, { type: 'image.insert' }>,
    entry: Entry,
  ): Promise<void> {
    let result: CanvasInsertResult;
    try {
      const capabilities = this.port.context().canvas.capabilities;
      result =
        request.image.position && !capabilities.positionImageLayer
          ? {
              status: 'unsupported',
              message: 'This canvas does not support positioned image layers.',
            }
          : capabilities.insertImageLayer
            ? await this.port.insert(request.image, entry.cancellation)
            : { status: 'unsupported', message: 'This canvas does not support image layers.' };
    } catch (error: unknown) {
      if (entry.cancellation.cancelled())
        result = { status: 'cancelled', message: 'Canvas insertion cancelled.' };
      else {
        this.port.report(error);
        result = { status: 'error', message: 'The canvas could not insert this image.' };
      }
    }
    this.#reply(entry, { ...this.#envelope(request.requestId), type: 'image.reply', result });
  }
  /** Publish only inside the original lifetime and evict completed entries while retaining the replay frontier. */
  #reply(entry: Entry, reply: CanvasMessage): void {
    if (this.#disposed) return;
    entry.reply = reply;
    this.#send(reply);
    this.#trim(0);
  }
  /** Bound retained payloads as well as request count while keeping the replay frontier authoritative. */
  #trim(incoming: number): void {
    while (
      this.#entries.size > 32 ||
      this.#retainedCharacters + incoming > MAX_CANVAS_ENCODED_CHARACTERS
    ) {
      const completed = [...this.#entries].find(([, item]) => item.reply !== null);
      if (!completed) break;
      this.#entries.delete(completed[0]);
      this.#retainedCharacters -= completed[1].imageData.length + completed[1].fingerprint.length;
    }
  }
  /** Return an explicit refusal without invoking native document mutation. */
  #failure(requestId: number, status: 'stale', message: string): void {
    this.#send({
      ...this.#envelope(requestId),
      type: 'error.reply',
      failure: { status, message },
    });
  }
  /** Keep transport failures observable without losing cached mutation results or leaking rejected tasks. */
  #send(message: CanvasMessage): void {
    try {
      this.port.send(message);
    } catch (error: unknown) {
      this.port.report(error);
    }
  }
  /** Attach the authoritative protocol and session to every response. */
  #envelope(requestId: number): {
    readonly protocol: 'sugarcubes.canvas';
    readonly version: 2;
    readonly session: string;
    readonly requestId: number;
  } {
    return { protocol: 'sugarcubes.canvas', version: 2, session: this.session, requestId };
  }
}
