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
/** Translate portable image insertion into a single Photoshop transaction without changing canvas geometry. */
import { app, core, constants, action } from 'photoshop';
import { findCanvasDocument, canvasDocumentIdentity } from './PhotoshopCanvasIdentity.js';
type Document = NonNullable<typeof app.activeDocument>;
type ModalExecution = Parameters<Parameters<typeof core.executeAsModal>[0]>[0];
import type { CanvasCancellationSource } from '../../../contracts/CanvasHost.js';
import type {
  CanvasContext,
  CanvasInsertImage,
  CanvasInsertResult,
} from '../../../contracts/CanvasProtocol.generated.js';
import type { PreparedCanvasImage } from '../application/InsertCanvasImage.js';
import type { PhotoshopImageAssets } from './PhotoshopImageAssets.js';
import { placeCanvasImage } from '../../../shared/CanvasPlacement.js';
/** Identify a deliberate cancellation separately from unexpected native failures. */
class CanvasInsertCancelled extends Error {}
/** Keep document lookup, native history and source-document cleanup inside their editor adapter. */
export class PhotoshopCanvasDocuments {
  /** Borrow native asset ownership rather than resolving file paths or downloading in a modal scope. */
  constructor(
    private readonly assets: PhotoshopImageAssets,
    private readonly report: (error: unknown) => void,
  ) {}
  /** Report implemented capabilities and a geometry-qualified opaque destination identity. */
  context(): CanvasContext {
    const document = app.activeDocument;
    return {
      canvas: {
        id: 'photoshop',
        label: 'Photoshop',
        capabilities: { displayOutput: false, insertImageLayer: true, positionImageLayer: true },
      },
      document: document ? canvasDocumentIdentity(document) : null,
    };
  }
  /** Place in an isolated source before duplication so an active target selection cannot constrain translation. */
  async insert(
    request: CanvasInsertImage,
    image: PreparedCanvasImage,
    cancellation: CanvasCancellationSource,
  ): Promise<CanvasInsertResult> {
    const state = { modalCancelled: false };
    try {
      let result: CanvasInsertResult = {
        status: 'error',
        message: 'Photoshop did not execute the canvas transaction.',
      };
      await core.executeAsModal(
        async (execution) => {
          try {
            result = await this.#commit(execution, request, image, cancellation);
          } catch (error: unknown) {
            state.modalCancelled = execution.isCancelled;
            throw error;
          }
        },
        { commandName: 'Insert generated image', timeOut: 2000 },
      );
      return result;
    } catch (error: unknown) {
      if (
        error instanceof CanvasInsertCancelled ||
        cancellation.cancelled() ||
        state.modalCancelled
      )
        return { status: 'cancelled', message: 'Canvas insertion cancelled.' };
      throw error;
    }
  }
  /** Commit only inside Photoshop's exclusive scope and roll back the target on any partial failure. */
  async #commit(
    execution: ModalExecution,
    request: CanvasInsertImage,
    image: PreparedCanvasImage,
    cancellation: CanvasCancellationSource,
  ): Promise<CanvasInsertResult> {
    const target = findCanvasDocument(request.document.id);
    const current = target ? canvasDocumentIdentity(target) : null;
    if (
      !target ||
      !current ||
      current.id !== request.document.id ||
      current.geometryRevision !== request.document.geometryRevision ||
      current.width !== request.document.width ||
      current.height !== request.document.height
    )
      return {
        status: 'stale',
        message: 'The destination document closed or its canvas geometry changed.',
      } as const;
    if (
      target.artboards.length ||
      target.mode !== constants.DocumentMode.RGB ||
      target.quickMaskMode
    )
      return {
        status: 'unsupported',
        message: 'Image insertion requires an RGB document without artboards or Quick Mask.',
      } as const;
    if (cancellation.cancelled()) throw new CanvasInsertCancelled();
    const historyControl = execution.hostControl;
    if (
      !('registerAutoCloseDocument' in historyControl) ||
      typeof historyControl.registerAutoCloseDocument !== 'function'
    )
      throw new Error('Photoshop cannot guarantee temporary document cleanup.');
    const previous = app.activeDocument;
    let source: Document | null = null;
    const history = await execution.hostControl.suspendHistory({
      documentID: target.id,
      name: 'Insert generated image',
    });
    let committed = false;
    try {
      const existing = new Set(app.documents.map((document) => document.id));
      const opened = await action.batchPlay(
        [
          {
            _obj: 'open',
            null: { _path: this.assets.token(image.key), _kind: 'local' },
            _options: { dialogOptions: 'dontDisplay' },
          },
        ],
        {},
      );
      if (opened[0]?._obj === 'error')
        throw new Error('Photoshop could not decode the generated image.');
      const openedDocument = app.activeDocument ?? null;
      if (!openedDocument || existing.has(openedDocument.id))
        throw new Error('Photoshop did not open an isolated image document.');
      source = openedDocument;
      // The verified host API is missing from the pinned SDK's history-control declaration.
      const registerAutoClose = historyControl.registerAutoCloseDocument as (
        id: number,
      ) => Promise<void>;
      await registerAutoClose.call(historyControl, source.id);
      if (source.width !== image.width || source.height !== image.height)
        throw new Error('Decoded image dimensions disagree with its admitted PNG header.');
      let layer = source.layers[0];
      if (!layer) throw new Error('The generated image has no pixel layer.');
      if (layer.isBackgroundLayer) {
        layer = (await layer.duplicate()) ?? undefined;
        if (!layer) throw new Error('The imported image could not become a raster layer.');
      }
      layer.allLocked = false;
      const position = placeCanvasImage(request.document, image, request.position);
      await layer.translate(position.left, position.top);
      if (this.#cancelled(execution, cancellation)) throw new CanvasInsertCancelled();
      const inserted = await layer.duplicate(
        target,
        constants.ElementPlacement.PLACEATBEGINNING,
        request.layerName,
      );
      if (!inserted) throw new Error('Photoshop did not create the generated image layer.');
      if (this.#cancelled(execution, cancellation)) throw new CanvasInsertCancelled();
      if (canvasDocumentIdentity(target).geometryRevision !== request.document.geometryRevision)
        throw new Error('Photoshop changed the target canvas during image insertion.');
      await execution.hostControl.resumeHistory(history, true);
      committed = true;
      return {
        status: 'inserted',
        documentId: request.document.id,
        layerId: String(inserted.id),
        ...position,
        width: image.width,
        height: image.height,
      } as const;
    } catch (error: unknown) {
      if (!committed && !execution.isCancelled) {
        try {
          await execution.hostControl.resumeHistory(history, false);
        } catch (rollback: unknown) {
          throw new AggregateError([error, rollback], 'Canvas insertion and rollback failed.');
        }
      }
      throw error;
    } finally {
      if (!execution.isCancelled) {
        try {
          source?.closeWithoutSaving();
        } catch (error: unknown) {
          this.report(error);
        }
        try {
          if (previous && findCanvasDocument(String(previous.id))) app.activeDocument = previous;
        } catch (error: unknown) {
          this.report(error);
        }
      }
    }
  }
  /** Re-read externally mutable host and caller cancellation after each awaited native operation. */
  #cancelled(execution: ModalExecution, cancellation: CanvasCancellationSource): boolean {
    return execution.isCancelled || cancellation.cancelled();
  }
}
