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
/** Supply GIMP document and insertion capabilities to the existing shared canvas server. */
import type {
  CanvasContext,
  CanvasInsertImage,
  CanvasInsertResult,
} from '../../../contracts/CanvasProtocol.generated.js';
import type { CanvasCancellationSource } from '../../../contracts/CanvasHost.js';
import { decodeCanvasImage } from '../../../shared/CanvasImageEncoding.js';
import { canvasPngDimensions } from '../../../shared/CanvasImagePolicy.js';
import { placeCanvasImage } from '../../../shared/CanvasPlacement.js';
import { sameCanvasSourceDocument } from '../../../shared/CanvasSourceIdentity.js';
import type { GimpNativePort } from '../application/NativeGimpSession.js';
import {
  GimpNativeFailure,
  gimpCanvasDocument,
  readGimpContext,
  rejectNativeResult,
} from './GimpCanvasMetadata.js';

/** Keep image admission and placement with their shared owners; translate only the native mutation. */
export class GimpCanvasDocuments {
  /** Borrow a viewer-independent native port without introducing another execution API. */
  constructor(private readonly native: GimpNativePort) {}

  /** Report actual insertion support for the bound native document. */
  async context(): Promise<CanvasContext> {
    const current = await readGimpContext(this.native);
    const insert = current.document !== null && current.rgb;
    return {
      canvas: {
        id: 'gimp',
        label: 'GIMP',
        capabilities: {
          displayOutput: false,
          insertImageLayer: insert,
          positionImageLayer: insert,
          geometry: true,
        },
      },
      document: current.document ? gimpCanvasDocument(current.hostId, current.document) : null,
    };
  }

  /** Admit one PNG and commit exactly the placement chosen by the common canvas policy. */
  async insert(
    request: CanvasInsertImage,
    cancellation: CanvasCancellationSource,
  ): Promise<CanvasInsertResult> {
    try {
      if (cancellation.cancelled())
        return { status: 'cancelled', message: 'GIMP insertion cancelled.' };
      const current = await readGimpContext(this.native, cancellation);
      if (
        !current.document ||
        !sameCanvasSourceDocument(
          gimpCanvasDocument(current.hostId, current.document),
          request.document,
        )
      )
        return {
          status: 'stale',
          message: 'The captured GIMP destination is no longer available.',
        };
      if (!current.rgb)
        return { status: 'unsupported', message: 'GIMP insertion requires an RGB image.' };
      const dimensions = canvasPngDimensions(decodeCanvasImage(request.image));
      const position = placeCanvasImage(request.document, dimensions, request.position);
      const result = await this.native.execute(
        {
          type: 'image.insert',
          document: current.document,
          data: request.image.data,
          width: dimensions.width,
          height: dimensions.height,
          name: request.layerName,
          ...position,
        },
        cancellation,
      );
      if (result.kind !== 'inserted') rejectNativeResult(result);
      return {
        status: 'inserted',
        documentId: request.document.id,
        layerId: String(result.tattoo),
        left: result.left,
        top: result.top,
        width: result.width,
        height: result.height,
      };
    } catch (error: unknown) {
      if (error instanceof GimpNativeFailure)
        return { status: error.status, message: error.message };
      throw error;
    }
  }
}
