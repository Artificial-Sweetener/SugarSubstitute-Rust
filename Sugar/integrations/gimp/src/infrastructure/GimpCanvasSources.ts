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
/** Supply native frozen compositions and selections through the existing capture-only source contract. */
import type { CanvasSourceHost } from '../../../contracts/CanvasSources.js';
import type {
  CanvasSourceQuery,
  CanvasSourceResult,
  CanvasSourceSnapshot,
  CanvasPixelRaster,
} from '../../../contracts/CanvasProtocol.generated.js';
import { CanvasCancellation } from '../../../shared/CanvasCancellation.js';
import {
  MAX_CANVAS_RASTER_BYTES,
  canvasRasterByteLength,
  decodeCanvasRaster,
} from '../../../shared/CanvasRasterEncoding.js';
import { sameCanvasSourceDocument } from '../../../shared/CanvasSourceIdentity.js';
import type { GimpNativePort } from '../application/NativeGimpSession.js';
import { gimpSourceVisibility } from './GimpSourceLayers.js';
import { GimpNativeCapture } from './GimpNativeCapture.js';
import {
  GimpNativeFailure,
  gimpCanvasDocument,
  gimpCanvasLayers,
  readGimpContext,
  rejectNativeResult,
} from './GimpCanvasMetadata.js';

/** Own coherent source batches while native change observation is explicitly unavailable. */
export class GimpCanvasSources implements CanvasSourceHost {
  readonly canvasId = 'gimp';
  readonly changes = null;
  readonly #capture: GimpNativeCapture;
  #tail: Promise<unknown> = Promise.resolve();
  #active: CanvasCancellation | null = null;
  #disposed = false;

  /** Borrow native translation and keep source failures observable at the adapter boundary. */
  constructor(
    private readonly native: GimpNativePort,
    private readonly report: (error: unknown) => void,
  ) {
    this.#capture = new GimpNativeCapture(native);
  }

  /** Serialize batches without polling pixels or guessing a committed-content revision. */
  execute(query: CanvasSourceQuery): Promise<CanvasSourceResult> {
    const pending = this.#tail.then(async (): Promise<CanvasSourceResult> => {
      if (this.#disposed)
        return { kind: 'failure', status: 'unavailable', message: 'GIMP source capture closed.' };
      const cancellation = new CanvasCancellation();
      this.#active = cancellation;
      try {
        return await this.#execute(query, cancellation);
      } catch (error: unknown) {
        if (error instanceof GimpNativeFailure)
          return { kind: 'failure', status: error.status, message: error.message };
        this.report(error);
        return { kind: 'failure', status: 'error', message: 'GIMP source capture failed.' };
      } finally {
        this.#active = null;
      }
    });
    this.#tail = pending;
    return pending;
  }

  /** Cancel a retired public session and retry every known native release after its last batch. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#active?.cancel();
    void this.#tail.then(() => this.#capture.close()).catch(this.report);
  }

  /** Keep placement, scale admission and total collection bounds in the portable canvas vocabulary. */
  async #execute(
    query: CanvasSourceQuery,
    cancellation: CanvasCancellation,
  ): Promise<CanvasSourceResult> {
    const current = await readGimpContext(this.native, cancellation);
    const document = current.document ? gimpCanvasDocument(current.hostId, current.document) : null;
    if (query.type === 'context')
      return {
        kind: 'context',
        supported: true,
        observesChanges: false,
        document,
        revision: current.document?.revision ?? 'no-document',
        targets: ['selection', 'current-layer', 'layer', 'composition', 'composition-from-layer'],
        layers: gimpCanvasLayers(current.layers),
        currentLayerId: current.primaryLayer === null ? null : String(current.primaryLayer),
      };
    if (!current.document || !document || !sameCanvasSourceDocument(document, query.document))
      return {
        kind: 'failure',
        status: 'stale',
        message: 'The captured GIMP source is no longer available.',
      };
    const scale = query.maximumSize
      ? Math.min(1, query.maximumSize / Math.max(document.width, document.height))
      : 1;
    const width = Math.max(1, Math.round(document.width * scale));
    const height = Math.max(1, Math.round(document.height * scale));
    const bytes = query.targets.reduce(
      (sum, target) =>
        sum +
        canvasRasterByteLength(width, height, target.kind === 'selection' ? 'coverage8' : 'rgba8'),
      0,
    );
    if (bytes > MAX_CANVAS_RASTER_BYTES)
      throw new Error('The canvas source collection exceeds its transfer budget.');
    return this.#capture.run(current.document, cancellation, async (snapshot) => {
      const snapshots: CanvasSourceSnapshot[] = [];
      for (const target of query.targets) {
        const result = await this.native.execute(
          target.kind === 'selection'
            ? { type: 'snapshot.selection', token: snapshot.token, width, height, scale }
            : {
                type: 'snapshot.projection',
                token: snapshot.token,
                width,
                height,
                scale,
                visibility: gimpSourceVisibility(target, snapshot.layers, snapshot.primaryLayer),
              },
          cancellation,
        );
        if (result.kind !== 'pixels') rejectNativeResult(result);
        const format = target.kind === 'selection' ? 'coverage8' : 'rgba8';
        if (result.width !== width || result.height !== height || result.format !== format)
          throw new Error('The GIMP capture differs from its admitted pixel geometry.');
        const pixels: CanvasPixelRaster = { width, height, format, data: result.data };
        decodeCanvasRaster(pixels);
        snapshots.push({ target, document, revision: snapshot.token, pixels });
      }
      return { kind: 'captured', snapshots };
    });
  }
}
