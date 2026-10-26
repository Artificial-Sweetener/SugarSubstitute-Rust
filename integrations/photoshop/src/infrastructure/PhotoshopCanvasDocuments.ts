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
import { app, core, constants, imaging } from 'photoshop';
import { findCanvasDocument, canvasDocumentIdentity } from './PhotoshopCanvasIdentity.js';
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
/** Own direct pixel insertion as one native history transaction. */
export class PhotoshopCanvasDocuments {
  /** Borrow native asset ownership rather than resolving file paths or downloading in a modal scope. */
  constructor(private readonly assets: PhotoshopImageAssets) {}
  /** Report implemented capabilities and a geometry-qualified opaque destination identity. */
  context(): CanvasContext {
    const document = app.activeDocument;
    return {
      canvas: {
        id: 'photoshop',
        label: 'Photoshop',
        capabilities: {
          displayOutput: false,
          insertImageLayer: true,
          positionImageLayer: true,
          geometry: true,
        },
      },
      document: document ? canvasDocumentIdentity(document) : null,
    };
  }
  /** Write prepared pixels at their final origin without opening or switching documents. */
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
    const position = placeCanvasImage(request.document, image, request.position);
    const pixels = this.assets.imageData(image.key);
    if (pixels.width !== image.width || pixels.height !== image.height)
      throw new Error('The prepared native pixels do not match their admitted frame.');
    const history = await execution.hostControl.suspendHistory({
      documentID: target.id,
      name: 'Insert generated image',
    });
    let committed = false;
    try {
      const first = target.layers[0];
      const inserted = await target.createLayer(constants.LayerKind.NORMAL, {
        name: request.layerName,
      });
      if (!inserted) throw new Error('Photoshop did not create the generated image layer.');
      if (first && target.layers[0]?.id !== inserted.id) {
        // The pinned SDK declares void; normalize native completion before writing pixels.
        const moveLayer = inserted.move.bind(inserted);
        const move: (...parameters: Parameters<typeof moveLayer>) => unknown = moveLayer;
        await Promise.resolve(move(first, constants.ElementPlacement.PLACEBEFORE));
      }
      if (this.#cancelled(execution, cancellation)) throw new CanvasInsertCancelled();
      await imaging.putPixels({
        documentID: target.id,
        layerID: inserted.id,
        imageData: pixels,
        replace: true,
        targetBounds: { ...position, width: image.width, height: image.height },
        commandName: 'Insert generated image',
      });
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
      };
    } catch (error: unknown) {
      if (!committed && !execution.isCancelled) {
        try {
          await execution.hostControl.resumeHistory(history, false);
        } catch (rollback: unknown) {
          throw new AggregateError([error, rollback], 'Canvas insertion and rollback failed.');
        }
      }
      throw error;
    }
  }
  /** Re-read externally mutable host and caller cancellation after each awaited native operation. */
  #cancelled(execution: ModalExecution, cancellation: CanvasCancellationSource): boolean {
    return execution.isCancelled || cancellation.cancelled();
  }
}
