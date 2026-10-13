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
/** Qualify local overlay commands and revisions against one current browser attachment. */

import type { CanvasAttachmentAddress } from '../domain/CanvasIdentity.js';
import type {
  CanvasCapabilities,
  CanvasCommand,
  CanvasCommandOutcome,
  CanvasCommandRequest,
  CanvasSurfaceGeometry,
  CanvasViewPort,
  CanvasViewSnapshot,
} from '../domain/CanvasViewPort.js';
import { decodeCanvasCommandRequest } from './CanvasCommandDecoder.js';
import { decodeCanvasSurfaceGeometry } from './CanvasGeometryDecoder.js';
import { authorizeCanvasMessage } from './CanvasMessageAuthority.js';

export interface BrowserCanvasPortOptions {
  readonly origin: string;
  readonly capabilities: CanvasCapabilities;
  /** Check both the attachment lease and the current workflow generation. */
  accepts(): boolean;
  /** Apply a validated intent through the owning navigation/camera/resource collaborator. */
  apply(command: CanvasCommand): boolean | Promise<boolean>;
  /** Reconcile qualified client geometry through the attachment surface owner. */
  geometry(geometry: CanvasSurfaceGeometry): void;
  /** Release this exact surface and its pending work. */
  release(): void;
  /** Preserve unexpected command errors with attachment context. */
  report(error: unknown, request: CanvasCommandRequest): void;
}

/** Keep surface revisions independent of lightweight selection-state revisions. */
export class BrowserCanvasPort implements CanvasViewPort {
  readonly capabilities: CanvasCapabilities;
  readonly #listeners = new Set<(snapshot: CanvasViewSnapshot) => void>();
  readonly #address: CanvasAttachmentAddress;
  #snapshot: CanvasViewSnapshot;
  #request = 0;
  #disposed = false;

  /** Seal the first attachment address and explicit feature capabilities. */
  constructor(
    initial: CanvasViewSnapshot,
    private readonly options: BrowserCanvasPortOptions,
  ) {
    this.#address = initial.address;
    this.#snapshot = Object.freeze(initial);
    this.capabilities = Object.freeze({
      schema: 1,
      commands: Object.freeze([...options.capabilities.commands]),
    });
  }

  /** Read the latest published surface snapshot without consulting ambient host state. */
  read(): CanvasViewSnapshot {
    return this.#snapshot;
  }

  /** Subscribe only to this attachment, with consumer-owned release. */
  subscribe(listener: (snapshot: CanvasViewSnapshot) => void): () => void {
    if (this.#disposed) return () => {};
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  }

  /** Publish an authoritative presentation change with a monotonically increasing local revision. */
  publish(snapshot: CanvasViewSnapshot): void {
    if (this.#disposed || !this.options.accepts()) return;
    this.#snapshot = Object.freeze({
      ...snapshot,
      address: this.#address,
      revision: this.#snapshot.revision + 1,
    });
    for (const listener of this.#listeners) listener(this.#snapshot);
  }

  /** Decode every local intent as an external boundary and reject duplicate/stale deliveries. */
  async dispatch(value: CanvasCommandRequest): Promise<CanvasCommandOutcome> {
    const revision = this.#snapshot.revision;
    if (this.#disposed || !this.options.accepts()) return { status: 'cancelled', revision };
    const request = decodeCanvasCommandRequest(value, this.options.origin, {
      origin: this.options.origin,
      address: this.#address,
      capabilities: this.capabilities,
    });
    if (!request) return { status: 'unsupported', revision };
    if (request.requestId <= this.#request || request.expectedRevision !== revision)
      return { status: 'stale', revision };
    this.#request = request.requestId;
    try {
      const applied = await this.options.apply(request.command);
      if (this.#disposed || !this.options.accepts())
        return { status: 'cancelled', revision: this.#snapshot.revision };
      if (request.requestId !== this.#request)
        return { status: 'stale', revision: this.#snapshot.revision };
      return { status: applied ? 'applied' : 'unsupported', revision: this.#snapshot.revision };
    } catch (error: unknown) {
      this.options.report(error, request);
      return {
        status: 'error',
        revision: this.#snapshot.revision,
        message: 'The canvas action could not be completed.',
      };
    }
  }

  /** Validate density, visibility, and coordinates before updating this browser surface. */
  updateSurface(address: CanvasAttachmentAddress, geometry: CanvasSurfaceGeometry): void {
    if (this.#disposed || !this.options.accepts()) return;
    const decoded = decodeCanvasSurfaceGeometry(
      { schema: 1, address, geometry },
      this.options.origin,
      { origin: this.options.origin, address: this.#address },
    );
    if (decoded) this.options.geometry(decoded);
  }

  /** Release only a matching current attachment, leaving replacement surfaces untouched. */
  releaseSurface(address: CanvasAttachmentAddress): void {
    if (this.#disposed || !this.options.accepts()) return;
    if (
      authorizeCanvasMessage({ schema: 1, address }, this.options.origin, {
        origin: this.options.origin,
        address: this.#address,
      })
    )
      this.options.release();
  }

  /** End delivery without implicitly retiring a view or another attachment. */
  dispose(): void {
    this.#disposed = true;
    this.#listeners.clear();
  }
}
