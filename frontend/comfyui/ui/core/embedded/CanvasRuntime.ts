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
/** Bundle portable canvas mechanics for browser consumers without native editor dependencies. */
export { CanvasClient } from '../../../../../integrations/shared/CanvasClient.js';
export { MAX_CANVAS_IMAGE_BYTES } from '../../../../../integrations/shared/CanvasImagePolicy.js';
export { encodeCanvasImage } from '../../../../../integrations/shared/CanvasImageEncoding.js';
export { CanvasCancellation } from '../../../../../integrations/shared/CanvasCancellation.js';
export { WEB_CANVAS } from '../../../../../integrations/contracts/CanvasHost.js';
export { CanvasMaskClient } from '../../../../../integrations/shared/CanvasMaskClient.js';
export {
  encodeMaskPixels,
  decodeMaskPixels,
} from '../../../../../integrations/shared/CanvasMaskPixels.js';
export { isCanvasMaskReference } from '../../../../../integrations/shared/CanvasMasksValidation.generated.js';
export type { CanvasSourceHost } from '../../../../../integrations/contracts/CanvasSources.js';
export type {
  CanvasDocument,
  CanvasPixelRaster,
  CanvasInputTarget,
  CanvasInputSource,
  CanvasSourceTarget,
  CanvasSourceLayer,
  CanvasSourceContext,
  CanvasSourceSnapshot,
  CanvasSourceQuery,
  CanvasSourceResult,
  CanvasSourceChange,
} from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
export {
  isCanvasInputTarget,
  isCanvasInputSource,
} from '../../../../../integrations/shared/CanvasValidation.generated.js';
export {
  encodeCanvasRaster,
  decodeCanvasRaster,
  canvasRasterByteLength,
  MAX_CANVAS_RASTER_BYTES,
} from '../../../../../integrations/shared/CanvasRasterEncoding.js';
export {
  canvasInputSourceKey,
  canvasSourceTargetKey,
  sameCanvasSourceDocument,
} from '../../../../../integrations/shared/CanvasSourceIdentity.js';
export type { CanvasMaskHost } from '../../../../../integrations/contracts/CanvasMasks.js';
export type {
  CanvasMaskMessage,
  CanvasMaskCommand,
  CanvasMaskResult,
  CanvasMaskReference,
  CanvasMaskPixels,
  CanvasMaskSnapshot,
  CanvasMaskChanged,
} from '../../../../../integrations/contracts/CanvasMasksProtocol.generated.js';
export type {
  CanvasHost,
  CanvasLayerHost,
  CanvasCancellationSource,
} from '../../../../../integrations/contracts/CanvasHost.js';
export type {
  CanvasMessage,
  CanvasContext,
  CanvasDescriptor,
  CanvasInsertImage,
  CanvasInsertResult,
  CanvasEncodedImage,
} from '../../../../../integrations/contracts/CanvasProtocol.generated.js';

export type {
  CanvasAdapter,
  CanvasOutputBinding,
} from '../../../../../integrations/contracts/CanvasAdapter.js';
