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
/** Own read-only Photoshop source targeting independently of image and mask consumers. */
import { app, core } from 'photoshop';
import type { CanvasSourceHost } from '../../../contracts/CanvasSources.js';
import type {
  CanvasSourceChange,
  CanvasSourceQuery,
  CanvasSourceResult,
  CanvasSourceSnapshot,
  CanvasSourceTarget,
  CanvasPixelRaster,
} from '../../../contracts/CanvasProtocol.generated.js';
import {
  MAX_CANVAS_RASTER_BYTES,
  canvasRasterByteLength,
} from '../../../shared/CanvasRasterEncoding.js';
import {
  canvasDocumentIdentity,
  findCanvasDocument,
  type PhotoshopDocument,
} from './PhotoshopCanvasIdentity.js';
import { PhotoshopCommittedChanges } from './PhotoshopCommittedChanges.js';
import { nativeCanvasChanges } from '../../../shared/CanvasChangeRelay.js';
import { readSelectionPixels } from './PhotoshopSelectionPixels.js';
import { photoshopSourcePixels } from './PhotoshopSourcePixels.js';
import {
  photoshopSourceLayers,
  photoshopCurrentLayer,
  withPhotoshopCompositionRange,
} from './PhotoshopSourceLayers.js';

const targets: readonly CanvasSourceTarget['kind'][] = [
  'selection',
  'current-layer',
  'layer',
  'composition',
  'composition-from-layer',
];

/** Serialize coherent document reads and observe committed changes only while consumers subscribe. */
export class PhotoshopCanvasSources implements CanvasSourceHost {
  readonly canvasId = 'photoshop';
  readonly changes = nativeCanvasChanges<CanvasSourceChange>((listener) =>
    this.#subscribe(listener),
  );
  readonly #listeners = new Set<(change: CanvasSourceChange) => void>();
  readonly #changes: PhotoshopCommittedChanges;
  #tail: Promise<unknown> = Promise.resolve();
  #busy = false;
  #disposed = false;
  #state = '';
  #serial = 0;
  /** Bind native metadata observation without assuming a mask workflow or creating document state. */
  constructor(private readonly report: (error: unknown) => void) {
    this.#changes = new PhotoshopCommittedChanges(
      () => (app.activeDocument?.quickMaskMode ? null : this.#nativeState()),
      () => this.#busy,
      async () => {
        await this.#tail;
        if (this.#disposed) return;
        const change = {
          documentId: app.activeDocument ? String(app.activeDocument.id) : null,
          revision: this.#revision(),
        };
        for (const listener of this.#listeners) listener(change);
      },
      report,
    );
  }
  /** Read only the caller's requested document and return explicit failures for stale identities. */
  execute(query: CanvasSourceQuery): Promise<CanvasSourceResult> {
    const pending = this.#tail.then(async () => {
      if (this.#disposed)
        return {
          kind: 'failure',
          status: 'unavailable',
          message: 'Photoshop source capture closed.',
        } as const;
      this.#busy = true;
      try {
        return await this.#execute(query);
      } catch (error: unknown) {
        this.report(error);
        return {
          kind: 'failure',
          status: 'error',
          message:
            error &&
            typeof error === 'object' &&
            'message' in error &&
            typeof error.message === 'string'
              ? error.message.slice(0, 512)
              : 'Photoshop source capture failed.',
        } as const;
      } finally {
        this.#busy = false;
        this.#changes.acceptCurrent();
      }
    });
    this.#tail = pending;
    return pending;
  }
  /** Share one metadata observer across every consumer of this native canvas. */
  #subscribe(listener: (change: CanvasSourceChange) => void): (() => void) | null {
    if (this.#disposed) return null;
    this.#listeners.add(listener);
    this.#changes.start();
    return () => {
      this.#listeners.delete(listener);
      if (!this.#listeners.size) this.#changes.stop();
    };
  }
  /** Retire subscriptions and pending reads without changing a Photoshop document. */
  dispose(): void {
    this.#disposed = true;
    this.#listeners.clear();
    this.#changes.dispose();
  }
  /** Use one native modal transaction and roll back temporary composition visibility on every path. */
  async #execute(query: CanvasSourceQuery): Promise<CanvasSourceResult> {
    let result: CanvasSourceResult = {
      kind: 'failure',
      status: 'unavailable',
      message: 'Open a Photoshop document to capture a canvas source.',
    };
    await core.executeAsModal(
      async (execution) => {
        if (this.#disposed) return;
        const document = app.activeDocument;
        if (query.type === 'context') {
          const catalog = document
            ? photoshopSourceLayers(document)
            : { values: [], handles: new Map() };
          result = {
            kind: 'context',
            supported: true,
            observesChanges: true,
            document: document ? canvasDocumentIdentity(document) : null,
            revision: this.#revision(),
            targets,
            layers: catalog.values,
            currentLayerId: document ? await photoshopCurrentLayer(document) : null,
          };
          return;
        }
        if (
          !document ||
          findCanvasDocument(query.document.id)?.id !== document.id ||
          canvasDocumentIdentity(document).geometryRevision !== query.document.geometryRevision
        )
          throw new Error('The Photoshop document changed before source capture.');
        if (document.quickMaskMode || document.artboards.length) {
          result = {
            kind: 'failure',
            status: 'unsupported',
            message: 'Finish Quick Mask editing or use a Photoshop document without artboards.',
          };
          return;
        }
        const revision = this.#revision(),
          primary = await photoshopCurrentLayer(document),
          catalog = photoshopSourceLayers(document);
        const scale = query.maximumSize
          ? Math.min(1, query.maximumSize / Math.max(document.width, document.height))
          : 1;
        const width = Math.max(1, Math.round(document.width * scale)),
          height = Math.max(1, Math.round(document.height * scale));
        const budget = query.targets.reduce(
          (sum, target) =>
            sum +
            canvasRasterByteLength(
              width,
              height,
              target.kind === 'selection' ? 'coverage8' : 'rgba8',
            ),
          0,
        );
        if (budget > MAX_CANVAS_RASTER_BYTES)
          throw new Error('The canvas source collection exceeds its transfer budget.');
        const history = await execution.hostControl.suspendHistory({
          documentID: document.id,
          name: 'Read canvas sources',
        });
        try {
          const snapshots: CanvasSourceSnapshot[] = [];
          let transferred = 0;
          for (const target of query.targets) {
            const pixels = await this.#capture(
              document,
              target,
              query.maximumSize,
              primary,
              catalog,
            );
            transferred += canvasRasterByteLength(pixels.width, pixels.height, pixels.format);
            if (transferred > MAX_CANVAS_RASTER_BYTES)
              throw new Error('The canvas source collection exceeds its transfer budget.');
            snapshots.push({
              target,
              document: canvasDocumentIdentity(document),
              revision,
              pixels,
            });
          }
          result = { kind: 'captured', snapshots };
        } finally {
          await execution.hostControl.resumeHistory(history, false);
        }
      },
      { commandName: 'Read canvas sources', timeOut: 1 },
    );
    return result;
  }
  /** Translate portable target semantics only at Photoshop's native pixel boundary. */
  async #capture(
    document: PhotoshopDocument,
    target: CanvasSourceTarget,
    maximumSize: number,
    primary: string | null,
    catalog: ReturnType<typeof photoshopSourceLayers>,
  ): Promise<CanvasPixelRaster> {
    if (target.kind === 'selection') return await readSelectionPixels(document, maximumSize);
    if (target.kind === 'composition')
      return await photoshopSourcePixels(document, null, maximumSize);
    const layerId = target.kind === 'current-layer' ? primary : target.layerId;
    if (!layerId || !catalog.handles.has(layerId))
      throw new Error(
        `The Photoshop ${target.kind} source layer (${layerId ?? 'none'}) is unavailable.`,
      );
    if (target.kind === 'composition-from-layer')
      return await withPhotoshopCompositionRange(catalog, layerId, () =>
        photoshopSourcePixels(document, null, maximumSize),
      );
    return await photoshopSourcePixels(document, layerId, maximumSize);
  }
  /** Read only committed document identity and active-layer metadata while the user draws. */
  #nativeState(): string {
    const document = app.activeDocument;
    return document
      ? JSON.stringify([
          document.id,
          document.activeHistoryState.id,
          document.activeLayers.map((layer) => layer.id),
        ])
      : 'no-document';
  }
  /** Give context replies, committed changes and coherent capture batches one revision vocabulary. */
  #revision(): string {
    const state = this.#nativeState();
    if (state !== this.#state) {
      this.#state = state;
      this.#serial++;
    }
    return `${String(app.activeDocument?.id ?? 'none')}:${String(this.#serial)}`;
  }
}
