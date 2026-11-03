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
  CanvasSourceChange,
} from '../contracts/CanvasProtocol.generated.js';
import type { CanvasSourceHost } from '../contracts/CanvasSources.js';
import type { CanvasGeometryHost } from '../contracts/CanvasGeometry.js';
import type { CanvasSurfaceHost } from '../contracts/CanvasSurface.js';
import type { CanvasSurfacePresentationChange } from '../contracts/CanvasSurface.js';
import type { CanvasSurfacePresentationRequest } from '../contracts/CanvasSurfaceProtocol.generated.js';
import { isCanvasSurfacePresentationChange } from './CanvasSurfaceValidation.generated.js';
import {
  canvasPreviewMessageFits,
  isCanvasPreviewCommand,
  snapshotCanvasSurfacePresentation,
  validCanvasSurfacePresentation,
} from './CanvasSurfacePreview.js';
import { dispatchCanvasSurfaceDiscovery } from './CanvasSurfaceDiscovery.js';
import type {
  CanvasGeometryRequest,
  CanvasGeometryResult,
} from '../contracts/CanvasProtocol.generated.js';
import { CanvasCancellation } from './CanvasCancellation.js';
import { isCanvasMessage } from './CanvasValidation.generated.js';
import { MAX_CANVAS_ENCODED_CHARACTERS } from './CanvasImageEncoding.js';
import { CanvasChangeRelay } from './CanvasChangeRelay.js';
import {
  dispatchCanvasSurface,
  isCanvasSurfaceRequest,
  snapshotCanvasSurface,
  type CanvasSurfaceRequest,
} from './CanvasSurfaceDispatch.js';
/** Supply editor capabilities through typed ports rather than importing native APIs. */
export interface CanvasServerPort {
  readonly geometry?: CanvasGeometryHost;
  readonly sources?: CanvasSourceHost;
  readonly surface?: CanvasSurfaceHost;
  /** Read fresh native document metadata through the host's process boundary. */
  context(): Promise<CanvasContext>;
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
  readonly #sourceChanges: CanvasChangeRelay<CanvasSourceChange>;
  readonly #presentationChanges: CanvasChangeRelay<CanvasSurfacePresentationChange>;
  /** Bind one navigation's session and its capability implementation. */
  constructor(
    private readonly session: string,
    private readonly port: CanvasServerPort,
  ) {
    this.#sourceChanges = new CanvasChangeRelay(this.port.sources?.changes ?? null, (change) => {
      this.#send({
        protocol: 'sugarcubes.canvas',
        version: 3,
        session: this.session,
        requestId: 0,
        type: 'source.changed',
        change,
      });
    });
    this.#presentationChanges = new CanvasChangeRelay(
      this.port.surface?.presentation ?? null,
      (change) => {
        if (!isCanvasSurfacePresentationChange(change) || !validCanvasSurfacePresentation(change)) {
          this.port.report(
            new Error('Canvas surface supplied an invalid presentation observation.'),
          );
          return;
        }
        this.#send({
          ...this.#envelope(0),
          requestId: 0,
          type: 'surface.presentation.changed',
          schema: 1,
          change: snapshotCanvasSurfacePresentation(change),
        });
      },
    );
  }
  /** Validate before dispatch, including duplicate payload identity and cancellation ownership. */
  receive(value: unknown): boolean {
    if (this.#disposed || !isCanvasMessage(value) || value.session !== this.session) return false;
    if (value.type === 'cancel') {
      this.#entries.get(value.requestId)?.cancellation.cancel();
      return true;
    }
    // Editor ports advertise neither editing nor workflow surfaces; refuse them explicitly.
    if (value.type === 'edit.request' || value.type === 'workflow.request') {
      const result = {
        kind: 'failure',
        status: 'unsupported',
        message: 'This canvas does not support native editing.',
      } as const;
      this.#send(
        value.type === 'edit.request'
          ? { ...this.#envelope(value.requestId), type: 'edit.reply', result }
          : { ...this.#envelope(value.requestId), type: 'workflow.reply', result },
      );
      return true;
    }
    if (
      value.type !== 'context.request' &&
      value.type !== 'image.insert' &&
      value.type !== 'geometry.request' &&
      value.type !== 'source.request' &&
      value.type !== 'surface.capabilities.request' &&
      value.type !== 'surface.presentation.request' &&
      value.type !== 'surface.prepare' &&
      value.type !== 'surface.geometry' &&
      value.type !== 'surface.select' &&
      value.type !== 'surface.overview' &&
      value.type !== 'surface.commit' &&
      value.type !== 'surface.viewport.request' &&
      value.type !== 'surface.preview.begin' &&
      value.type !== 'surface.preview.chunk' &&
      value.type !== 'surface.preview.end' &&
      value.type !== 'surface.preview.withdraw' &&
      value.type !== 'surface.release'
    )
      return false;
    const surface = isCanvasSurfaceRequest(value) ? snapshotCanvasSurface(value) : null;
    if (surface && isCanvasPreviewCommand(surface) && !canvasPreviewMessageFits(value)) {
      this.#failure(
        value.requestId,
        'stale',
        'Canvas preview request exceeds the native transport bounds.',
      );
      return true;
    }
    const fingerprint =
      value.type === 'context.request' || value.type === 'surface.capabilities.request'
        ? value.type
        : value.type === 'surface.presentation.request'
          ? JSON.stringify([value.type, value.schema, value.enabled])
          : value.type === 'source.request' || value.type === 'geometry.request'
            ? JSON.stringify([value.type, value.command])
            : value.type === 'image.insert'
              ? JSON.stringify([
                  value.type,
                  value.image.document.id,
                  value.image.document.geometryRevision,
                  value.image.document.width,
                  value.image.document.height,
                  value.image.image.format,
                  value.image.image.encoding,
                  value.image.layerName,
                  value.image.position ?? null,
                ])
              : JSON.stringify(surface);
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
    if (value.type === 'context.request') void this.#context(value.requestId, entry);
    else if (value.type === 'surface.capabilities.request')
      void dispatchCanvasSurfaceDiscovery(value, entry.cancellation, this.port).then((reply) => {
        this.#reply(entry, reply);
      });
    else if (value.type === 'surface.presentation.request') void this.#presentation(value, entry);
    else if (value.type === 'geometry.request') void this.#geometry(value, entry);
    else if (value.type === 'source.request') void this.#source(value, entry);
    else if (value.type === 'image.insert') void this.#insert(value, entry);
    else if (surface) void this.#surface(surface, entry);
    return true;
  }
  /** Reuse the session's replay, admission and cancellation owner for surface operations. */
  async #surface(request: CanvasSurfaceRequest, entry: Entry): Promise<void> {
    if (
      isCanvasPreviewCommand(request) &&
      request.type !== 'surface.preview.withdraw' &&
      !this.#presentationChanges.observing
    ) {
      this.#reply(entry, {
        ...this.#envelope(request.requestId),
        type: 'surface.reply',
        address: request.address,
        revision: request.revision,
        status: 'unavailable',
        message: 'Canvas preview presentation is not observed.',
      });
      return;
    }
    const reply = await dispatchCanvasSurface(request, entry.cancellation, this.port);
    this.#reply(entry, reply);
  }
  /** Cancel owned operations and refuse responses after the native panel is destroyed. */
  dispose(): void {
    this.#disposed = true;
    this.#sourceChanges.dispose();
    this.#presentationChanges.dispose();
    for (const entry of this.#entries.values()) entry.cancellation.cancel();
    this.#entries.clear();
    this.#retainedCharacters = 0;
  }
  /** Negotiate observation independently of attachment mutation and stop through the existing relay. */
  async #presentation(request: CanvasSurfacePresentationRequest, entry: Entry): Promise<void> {
    let status: 'available' | 'unsupported' | 'unavailable' | 'error' = 'unsupported';
    try {
      const surface = this.port.surface;
      const supported = surface && surface.capabilities.previewFormats.length > 0;
      if (!request.enabled || supported) {
        const result = await this.#presentationChanges.watch(request.enabled);
        status = result === 'observing' ? 'available' : result;
      }
      if (entry.cancellation.cancelled() && status === 'available') {
        await this.#presentationChanges.watch(false);
        status = 'unavailable';
      }
    } catch (error: unknown) {
      this.port.report(error);
      status = 'error';
    }
    this.#reply(entry, {
      ...this.#envelope(request.requestId),
      type: 'surface.presentation.reply',
      schema: 1,
      enabled: request.enabled,
      status,
      message: '',
    });
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
  /** Await native context without publishing into a disposed session or duplicating a pending read. */
  async #context(requestId: number, entry: Entry): Promise<void> {
    try {
      this.#reply(entry, {
        ...this.#envelope(requestId),
        type: 'context.reply',
        context: await this.port.context(),
      });
    } catch (error: unknown) {
      this.port.report(error);
      this.#reply(entry, {
        ...this.#envelope(requestId),
        type: 'error.reply',
        failure: { status: 'error', message: 'The canvas context is unavailable.' },
      });
    }
  }
  /** Dispatch optional read capabilities and scope native observation to this authenticated session. */
  async #source(request: CanvasSourceRequest, entry: Entry): Promise<void> {
    let result: CanvasSourceResult;
    const source = this.port.sources;
    try {
      if (request.command.type === 'watch') {
        const status = await this.#sourceChanges.watch(request.command.enabled);
        result =
          status === 'unavailable'
            ? { kind: 'failure', status, message: 'Canvas source observation is unavailable.' }
            : { kind: 'watching', supported: status === 'observing' };
      } else if (!source) {
        result =
          request.command.type === 'context'
            ? {
                kind: 'context',
                supported: false,
                observesChanges: false,
                document: (await this.port.context()).document,
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
      const capabilities = (await this.port.context()).canvas.capabilities;
      result = entry.cancellation.cancelled()
        ? { status: 'cancelled', message: 'Canvas insertion cancelled.' }
        : request.image.position && !capabilities.positionImageLayer
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
    readonly version: 3;
    readonly session: string;
    readonly requestId: number;
  } {
    return { protocol: 'sugarcubes.canvas', version: 3, session: this.session, requestId };
  }
}
