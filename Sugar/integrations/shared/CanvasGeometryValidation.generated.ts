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
import { isRecord } from './ContractRecord.js';
/** Validate a bounded CanvasDimensions payload before dispatch. */
export function isCanvasDimensions(value: unknown): value is Contract.CanvasDimensions {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
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
/** Validate a bounded CanvasDimensionFieldValues payload before dispatch. */
export function isCanvasDimensionFieldValues(
  value: unknown,
): value is Contract.CanvasDimensionFieldValues {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
    Object.hasOwn(value, 'width') &&
    typeof value['width'] === 'number' &&
    Number.isSafeInteger(value['width']) &&
    value['width'] >= 0 &&
    value['width'] <= 300000 &&
    Object.hasOwn(value, 'height') &&
    typeof value['height'] === 'number' &&
    Number.isSafeInteger(value['height']) &&
    value['height'] >= 0 &&
    value['height'] <= 300000
  );
}
/** Validate a bounded CanvasSelectionBounds payload before dispatch. */
export function isCanvasSelectionBounds(value: unknown): value is Contract.CanvasSelectionBounds {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
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
/** Validate a bounded CanvasDimensionLimits payload before dispatch. */
export function isCanvasDimensionLimits(value: unknown): value is Contract.CanvasDimensionLimits {
  return (
    isRecord(value) &&
    Object.keys(value).length === 3 &&
    Object.hasOwn(value, 'minimum') &&
    typeof value['minimum'] === 'number' &&
    Number.isSafeInteger(value['minimum']) &&
    value['minimum'] >= 1 &&
    value['minimum'] <= 300000 &&
    Object.hasOwn(value, 'maximum') &&
    typeof value['maximum'] === 'number' &&
    Number.isSafeInteger(value['maximum']) &&
    value['maximum'] >= 1 &&
    value['maximum'] <= 300000 &&
    Object.hasOwn(value, 'multiple') &&
    typeof value['multiple'] === 'number' &&
    Number.isSafeInteger(value['multiple']) &&
    value['multiple'] >= 1 &&
    value['multiple'] <= 300000
  );
}
/** Validate a bounded CanvasScaleOutput payload before dispatch. */
export function isCanvasScaleOutput(value: unknown): value is Contract.CanvasScaleOutput {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
    Object.hasOwn(value, 'id') &&
    typeof value['id'] === 'string' &&
    value['id'].length <= 512 &&
    Object.hasOwn(value, 'label') &&
    typeof value['label'] === 'string' &&
    value['label'].length <= 256
  );
}
/** Validate a bounded CanvasGeometryCommand payload before dispatch. */
export function isCanvasGeometryCommand(value: unknown): value is Contract.CanvasGeometryCommand {
  return (
    (isRecord(value) &&
      Object.keys(value).length === 1 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'context') ||
    (isRecord(value) &&
      Object.keys(value).length === 3 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'resize' &&
      Object.hasOwn(value, 'document') &&
      isCanvasDocument(value['document']) &&
      Object.hasOwn(value, 'size') &&
      isCanvasDimensions(value['size']))
  );
}
/** Validate a bounded CanvasGeometryResult payload before dispatch. */
export function isCanvasGeometryResult(value: unknown): value is Contract.CanvasGeometryResult {
  return (
    (isRecord(value) &&
      Object.keys(value).length === 6 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'context' &&
      Object.hasOwn(value, 'supported') &&
      typeof value['supported'] === 'boolean' &&
      Object.hasOwn(value, 'resize') &&
      typeof value['resize'] === 'boolean' &&
      Object.hasOwn(value, 'selectionSupported') &&
      typeof value['selectionSupported'] === 'boolean' &&
      Object.hasOwn(value, 'document') &&
      (isCanvasDocument(value['document']) || value['document'] === null) &&
      Object.hasOwn(value, 'selection') &&
      (isCanvasSelectionBounds(value['selection']) || value['selection'] === null)) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'resized' &&
      Object.hasOwn(value, 'document') &&
      isCanvasDocument(value['document'])) ||
    (isRecord(value) &&
      Object.keys(value).length === 3 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'failure' &&
      Object.hasOwn(value, 'status') &&
      (value['status'] === 'unavailable' ||
        value['status'] === 'unsupported' ||
        value['status'] === 'stale' ||
        value['status'] === 'cancelled' ||
        value['status'] === 'error') &&
      Object.hasOwn(value, 'message') &&
      typeof value['message'] === 'string' &&
      value['message'].length <= 512)
  );
}
/** Validate a bounded CanvasGeometryRequest payload before dispatch. */
export function isCanvasGeometryRequest(value: unknown): value is Contract.CanvasGeometryRequest {
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
    value['type'] === 'geometry.request' &&
    Object.hasOwn(value, 'command') &&
    isCanvasGeometryCommand(value['command'])
  );
}
/** Validate a bounded CanvasGeometryReply payload before dispatch. */
export function isCanvasGeometryReply(value: unknown): value is Contract.CanvasGeometryReply {
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
    value['type'] === 'geometry.reply' &&
    Object.hasOwn(value, 'result') &&
    isCanvasGeometryResult(value['result'])
  );
}
