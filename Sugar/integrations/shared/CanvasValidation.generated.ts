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
/** Generated protocol validation; edit the authoritative canvas.schema.json. */
import type * as Contract from '../contracts/CanvasProtocol.generated.js';
import { isCanvasDocument } from './CanvasDocumentValidation.generated.js';
import { isCanvasContext } from './CanvasContextValidation.generated.js';
import {
  isCanvasSourceRequest,
  isCanvasSourceReply,
  isCanvasSourceChanged,
} from './CanvasSourceValidation.generated.js';
import {
  isCanvasGeometryRequest,
  isCanvasGeometryReply,
} from './CanvasGeometryValidation.generated.js';
import {
  isCanvasEditRequest,
  isCanvasEditReply,
  isCanvasEditChanged,
} from './CanvasEditValidation.generated.js';
import {
  isCanvasWorkflowRequest,
  isCanvasWorkflowReply,
} from './CanvasWorkflowValidation.generated.js';
import { isCanvasSurfaceMessage } from './CanvasSurfaceValidation.generated.js';
import { isRecord } from './ContractRecord.js';
/** Validate a bounded CanvasFailure payload before dispatch. */
export function isCanvasFailure(value: unknown): value is Contract.CanvasFailure {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
    Object.hasOwn(value, 'status') &&
    (value['status'] === 'unavailable' ||
      value['status'] === 'unsupported' ||
      value['status'] === 'stale' ||
      value['status'] === 'cancelled' ||
      value['status'] === 'error') &&
    Object.hasOwn(value, 'message') &&
    typeof value['message'] === 'string' &&
    value['message'].length <= 512
  );
}
/** Validate a bounded CanvasInsertedLayer payload before dispatch. */
export function isCanvasInsertedLayer(value: unknown): value is Contract.CanvasInsertedLayer {
  return (
    isRecord(value) &&
    Object.keys(value).length === 7 &&
    Object.hasOwn(value, 'status') &&
    value['status'] === 'inserted' &&
    Object.hasOwn(value, 'documentId') &&
    typeof value['documentId'] === 'string' &&
    value['documentId'].length <= 128 &&
    Object.hasOwn(value, 'layerId') &&
    typeof value['layerId'] === 'string' &&
    value['layerId'].length <= 128 &&
    Object.hasOwn(value, 'left') &&
    typeof value['left'] === 'number' &&
    Number.isSafeInteger(value['left']) &&
    value['left'] >= -300000 &&
    value['left'] <= 300000 &&
    Object.hasOwn(value, 'top') &&
    typeof value['top'] === 'number' &&
    Number.isSafeInteger(value['top']) &&
    value['top'] >= -300000 &&
    value['top'] <= 300000 &&
    Object.hasOwn(value, 'width') &&
    typeof value['width'] === 'number' &&
    Number.isSafeInteger(value['width']) &&
    value['width'] >= 1 &&
    value['width'] <= 300000 &&
    Object.hasOwn(value, 'height') &&
    typeof value['height'] === 'number' &&
    Number.isSafeInteger(value['height']) &&
    value['height'] >= 1 &&
    value['height'] <= 300000
  );
}
/** Validate a bounded CanvasInsertResult payload before dispatch. */
export function isCanvasInsertResult(value: unknown): value is Contract.CanvasInsertResult {
  return isCanvasInsertedLayer(value) || isCanvasFailure(value);
}
/** Validate a bounded CanvasInsertImage payload before dispatch. */
export function isCanvasInsertImage(value: unknown): value is Contract.CanvasInsertImage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'document') &&
    isCanvasDocument(value['document']) &&
    Object.hasOwn(value, 'image') &&
    isCanvasEncodedImage(value['image']) &&
    Object.hasOwn(value, 'layerName') &&
    typeof value['layerName'] === 'string' &&
    value['layerName'].length <= 128 &&
    Object.hasOwn(value, 'position') &&
    (value['position'] === null || isCanvasImagePosition(value['position']))
  );
}
/** Validate a bounded CanvasReadRequest payload before dispatch. */
export function isCanvasReadRequest(value: unknown): value is Contract.CanvasReadRequest {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.canvas' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 3 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'requestId') &&
    typeof value['requestId'] === 'number' &&
    Number.isSafeInteger(value['requestId']) &&
    value['requestId'] >= 1 &&
    value['requestId'] <= 2147483647 &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'context.request'
  );
}
/** Validate a bounded CanvasReadReply payload before dispatch. */
export function isCanvasReadReply(value: unknown): value is Contract.CanvasReadReply {
  return (
    isRecord(value) &&
    Object.keys(value).length === 6 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.canvas' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 3 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'requestId') &&
    typeof value['requestId'] === 'number' &&
    Number.isSafeInteger(value['requestId']) &&
    value['requestId'] >= 1 &&
    value['requestId'] <= 2147483647 &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'context.reply' &&
    Object.hasOwn(value, 'context') &&
    isCanvasContext(value['context'])
  );
}
/** Validate a bounded CanvasInsertRequest payload before dispatch. */
export function isCanvasInsertRequest(value: unknown): value is Contract.CanvasInsertRequest {
  return (
    isRecord(value) &&
    Object.keys(value).length === 6 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.canvas' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 3 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'requestId') &&
    typeof value['requestId'] === 'number' &&
    Number.isSafeInteger(value['requestId']) &&
    value['requestId'] >= 1 &&
    value['requestId'] <= 2147483647 &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'image.insert' &&
    Object.hasOwn(value, 'image') &&
    isCanvasInsertImage(value['image'])
  );
}
/** Validate a bounded CanvasInsertReply payload before dispatch. */
export function isCanvasInsertReply(value: unknown): value is Contract.CanvasInsertReply {
  return (
    isRecord(value) &&
    Object.keys(value).length === 6 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.canvas' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 3 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'requestId') &&
    typeof value['requestId'] === 'number' &&
    Number.isSafeInteger(value['requestId']) &&
    value['requestId'] >= 1 &&
    value['requestId'] <= 2147483647 &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'image.reply' &&
    Object.hasOwn(value, 'result') &&
    isCanvasInsertResult(value['result'])
  );
}
/** Validate a bounded CanvasCancelRequest payload before dispatch. */
export function isCanvasCancelRequest(value: unknown): value is Contract.CanvasCancelRequest {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.canvas' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 3 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'requestId') &&
    typeof value['requestId'] === 'number' &&
    Number.isSafeInteger(value['requestId']) &&
    value['requestId'] >= 1 &&
    value['requestId'] <= 2147483647 &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'cancel'
  );
}
/** Validate a bounded CanvasErrorReply payload before dispatch. */
export function isCanvasErrorReply(value: unknown): value is Contract.CanvasErrorReply {
  return (
    isRecord(value) &&
    Object.keys(value).length === 6 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.canvas' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 3 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'requestId') &&
    typeof value['requestId'] === 'number' &&
    Number.isSafeInteger(value['requestId']) &&
    value['requestId'] >= 1 &&
    value['requestId'] <= 2147483647 &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'error.reply' &&
    Object.hasOwn(value, 'failure') &&
    isCanvasFailure(value['failure'])
  );
}
/** Validate a bounded CanvasEncodedImage payload before dispatch. */
export function isCanvasEncodedImage(value: unknown): value is Contract.CanvasEncodedImage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 3 &&
    Object.hasOwn(value, 'format') &&
    value['format'] === 'png' &&
    Object.hasOwn(value, 'encoding') &&
    value['encoding'] === 'base64' &&
    Object.hasOwn(value, 'data') &&
    typeof value['data'] === 'string' &&
    value['data'].length <= 89478488
  );
}
/** Validate a bounded CanvasImagePosition payload before dispatch. */
export function isCanvasImagePosition(value: unknown): value is Contract.CanvasImagePosition {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
    Object.hasOwn(value, 'left') &&
    typeof value['left'] === 'number' &&
    Number.isSafeInteger(value['left']) &&
    value['left'] >= -300000 &&
    value['left'] <= 300000 &&
    Object.hasOwn(value, 'top') &&
    typeof value['top'] === 'number' &&
    Number.isSafeInteger(value['top']) &&
    value['top'] >= -300000 &&
    value['top'] <= 300000
  );
}
/** Validate a bounded CanvasMessage payload before dispatch. */
export function isCanvasMessage(value: unknown): value is Contract.CanvasMessage {
  return (
    isCanvasReadRequest(value) ||
    isCanvasReadReply(value) ||
    isCanvasInsertRequest(value) ||
    isCanvasInsertReply(value) ||
    isCanvasCancelRequest(value) ||
    isCanvasErrorReply(value) ||
    isCanvasSourceRequest(value) ||
    isCanvasSourceReply(value) ||
    isCanvasSourceChanged(value) ||
    isCanvasGeometryRequest(value) ||
    isCanvasGeometryReply(value) ||
    isCanvasEditRequest(value) ||
    isCanvasEditReply(value) ||
    isCanvasEditChanged(value) ||
    isCanvasWorkflowRequest(value) ||
    isCanvasWorkflowReply(value) ||
    isCanvasSurfaceMessage(value)
  );
}
export { isCanvasDocument } from './CanvasDocumentValidation.generated.js';
export {
  isCanvasLayerCapabilities,
  isCanvasGeometryCapabilities,
  isCanvasEditingCapabilities,
  isCanvasCapabilities,
  isCanvasDescriptor,
  isCanvasContext,
} from './CanvasContextValidation.generated.js';
export {
  isCanvasSourceTarget,
  isCanvasInputTarget,
  isCanvasPixelRaster,
  isCanvasSourceLayer,
  isCanvasSourceContext,
  isCanvasSourceSnapshot,
  isCanvasSourceCommand,
  isCanvasSourceResult,
  isCanvasSourceRequest,
  isCanvasSourceReply,
  isCanvasSourceChanged,
  isCanvasSourceChange,
  isCanvasSourceQuery,
  isCanvasInputSource,
} from './CanvasSourceValidation.generated.js';
export {
  isCanvasDimensions,
  isCanvasDimensionFieldValues,
  isCanvasSelectionBounds,
  isCanvasDimensionLimits,
  isCanvasScaleOutput,
  isCanvasGeometryCommand,
  isCanvasGeometryResult,
  isCanvasGeometryRequest,
  isCanvasGeometryReply,
} from './CanvasGeometryValidation.generated.js';
export {
  isCanvasToolId,
  isCanvasBrushOptions,
  isCanvasEditCommand,
  isCanvasEditTool,
  isCanvasEditToolOptions,
  isCanvasEditSelectionBounds,
  isCanvasEditSelection,
  isCanvasEditHistory,
  isCanvasEditSession,
  isCanvasEditState,
  isCanvasEditResult,
  isCanvasEditRequest,
  isCanvasEditReply,
  isCanvasEditChanged,
} from './CanvasEditValidation.generated.js';
export {
  isCanvasWorkflowSurfaceBase,
  isCanvasWorkflowMask,
  isCanvasWorkflowSurface,
  isCanvasWorkflowAppearance,
  isCanvasWorkflowResizeMode,
  isCanvasWorkflowCommand,
  isCanvasWorkflowMaskLayer,
  isCanvasWorkflowSyncedSurface,
  isCanvasWorkflowResult,
  isCanvasWorkflowRequest,
  isCanvasWorkflowReply,
} from './CanvasWorkflowValidation.generated.js';
