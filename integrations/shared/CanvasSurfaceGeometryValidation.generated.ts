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
import type * as Contract from '../contracts/CanvasSurfaceProtocol.generated.js';

import { isRecord } from './ContractRecord.js';
/** Validate a bounded CanvasSurfaceAddress payload before dispatch. */
export function isCanvasSurfaceAddress(value: unknown): value is Contract.CanvasSurfaceAddress {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'sessionId') &&
    typeof value['sessionId'] === 'string' &&
    value['sessionId'].length <= 512 &&
    Array.from(value['sessionId']).length <= 256 &&
    new RegExp(
      '^canvas:(?:[^\\u0000-\\u001F\\u007F-\\u009F\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])+(?![\\s\\S])',
    ).test(value['sessionId']) &&
    Object.hasOwn(value, 'viewId') &&
    typeof value['viewId'] === 'string' &&
    value['viewId'].length <= 256 &&
    Array.from(value['viewId']).length <= 128 &&
    new RegExp(
      '^view:(?:[^\\u0000-\\u001F\\u007F-\\u009F\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])+(?![\\s\\S])',
    ).test(value['viewId']) &&
    Object.hasOwn(value, 'windowId') &&
    typeof value['windowId'] === 'string' &&
    value['windowId'].length <= 256 &&
    Array.from(value['windowId']).length <= 128 &&
    new RegExp(
      '^window:(?:[^\\u0000-\\u001F\\u007F-\\u009F\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])+(?![\\s\\S])',
    ).test(value['windowId']) &&
    Object.hasOwn(value, 'attachmentId') &&
    typeof value['attachmentId'] === 'string' &&
    value['attachmentId'].length <= 256 &&
    Array.from(value['attachmentId']).length <= 128 &&
    new RegExp(
      '^attachment:(?:[^\\u0000-\\u001F\\u007F-\\u009F\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])+(?![\\s\\S])',
    ).test(value['attachmentId']) &&
    Object.hasOwn(value, 'generation') &&
    typeof value['generation'] === 'number' &&
    Number.isSafeInteger(value['generation']) &&
    value['generation'] >= 1 &&
    value['generation'] <= 9007199254740991
  );
}
/** Validate a bounded CanvasSurfaceFileReference payload before dispatch. */
export function isCanvasSurfaceFileReference(
  value: unknown,
): value is Contract.CanvasSurfaceFileReference {
  return (
    isRecord(value) &&
    Object.keys(value).length === 3 &&
    Object.hasOwn(value, 'filename') &&
    typeof value['filename'] === 'string' &&
    value['filename'].length <= 8192 &&
    Array.from(value['filename']).length <= 4096 &&
    new RegExp('^(?:[^\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])*$').test(
      value['filename'],
    ) &&
    Object.hasOwn(value, 'subfolder') &&
    typeof value['subfolder'] === 'string' &&
    value['subfolder'].length <= 8192 &&
    Array.from(value['subfolder']).length <= 4096 &&
    new RegExp('^(?:[^\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])*$').test(
      value['subfolder'],
    ) &&
    Object.hasOwn(value, 'type') &&
    (value['type'] === 'input' || value['type'] === 'output' || value['type'] === 'temp')
  );
}
/** Validate a bounded CanvasSurfacePoint payload before dispatch. */
export function isCanvasSurfacePoint(value: unknown): value is Contract.CanvasSurfacePoint {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
    Object.hasOwn(value, 'x') &&
    typeof value['x'] === 'number' &&
    Number.isFinite(value['x']) &&
    value['x'] >= -1000000 &&
    value['x'] <= 1000000 &&
    Object.hasOwn(value, 'y') &&
    typeof value['y'] === 'number' &&
    Number.isFinite(value['y']) &&
    value['y'] >= -1000000 &&
    value['y'] <= 1000000
  );
}
/** Validate a bounded CanvasSurfaceSize payload before dispatch. */
export function isCanvasSurfaceSize(value: unknown): value is Contract.CanvasSurfaceSize {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
    Object.hasOwn(value, 'width') &&
    typeof value['width'] === 'number' &&
    Number.isFinite(value['width']) &&
    value['width'] >= 0 &&
    value['width'] <= 1000000 &&
    Object.hasOwn(value, 'height') &&
    typeof value['height'] === 'number' &&
    Number.isFinite(value['height']) &&
    value['height'] >= 0 &&
    value['height'] <= 1000000
  );
}
/** Validate a bounded CanvasSurfaceClip payload before dispatch. */
export function isCanvasSurfaceClip(value: unknown): value is Contract.CanvasSurfaceClip {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'x') &&
    typeof value['x'] === 'number' &&
    Number.isFinite(value['x']) &&
    value['x'] >= -1000000 &&
    value['x'] <= 1000000 &&
    Object.hasOwn(value, 'y') &&
    typeof value['y'] === 'number' &&
    Number.isFinite(value['y']) &&
    value['y'] >= -1000000 &&
    value['y'] <= 1000000 &&
    Object.hasOwn(value, 'width') &&
    typeof value['width'] === 'number' &&
    Number.isFinite(value['width']) &&
    value['width'] >= 0 &&
    value['width'] <= 1000000 &&
    Object.hasOwn(value, 'height') &&
    typeof value['height'] === 'number' &&
    Number.isFinite(value['height']) &&
    value['height'] >= 0 &&
    value['height'] <= 1000000
  );
}
/** Validate a bounded CanvasSurfaceOcclusion payload before dispatch. */
export function isCanvasSurfaceOcclusion(value: unknown): value is Contract.CanvasSurfaceOcclusion {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'x') &&
    typeof value['x'] === 'number' &&
    Number.isFinite(value['x']) &&
    value['x'] >= -1000000 &&
    value['x'] <= 1000000 &&
    Object.hasOwn(value, 'y') &&
    typeof value['y'] === 'number' &&
    Number.isFinite(value['y']) &&
    value['y'] >= -1000000 &&
    value['y'] <= 1000000 &&
    Object.hasOwn(value, 'width') &&
    typeof value['width'] === 'number' &&
    Number.isFinite(value['width']) &&
    value['width'] > 0 &&
    value['width'] <= 1000000 &&
    Object.hasOwn(value, 'height') &&
    typeof value['height'] === 'number' &&
    Number.isFinite(value['height']) &&
    value['height'] > 0 &&
    value['height'] <= 1000000
  );
}
/** Validate a bounded CanvasSurfaceGeometry payload before dispatch. */
export function isCanvasSurfaceGeometry(value: unknown): value is Contract.CanvasSurfaceGeometry {
  return (
    isRecord(value) &&
    Object.keys(value).length ===
      8 + (Object.hasOwn(value, 'occlusions') ? 1 : 0) + (Object.hasOwn(value, 'modal') ? 1 : 0) &&
    Object.hasOwn(value, 'coordinateSpace') &&
    value['coordinateSpace'] === 'client' &&
    Object.hasOwn(value, 'units') &&
    value['units'] === 'css-pixels' &&
    Object.hasOwn(value, 'origin') &&
    isCanvasSurfacePoint(value['origin']) &&
    Object.hasOwn(value, 'viewport') &&
    isCanvasSurfaceSize(value['viewport']) &&
    Object.hasOwn(value, 'clip') &&
    isCanvasSurfaceClip(value['clip']) &&
    Object.hasOwn(value, 'physicalPixelsPerCssPixel') &&
    typeof value['physicalPixelsPerCssPixel'] === 'number' &&
    Number.isFinite(value['physicalPixelsPerCssPixel']) &&
    value['physicalPixelsPerCssPixel'] > 0 &&
    value['physicalPixelsPerCssPixel'] <= 64 &&
    Object.hasOwn(value, 'visible') &&
    typeof value['visible'] === 'boolean' &&
    Object.hasOwn(value, 'focused') &&
    typeof value['focused'] === 'boolean' &&
    (!Object.hasOwn(value, 'occlusions') ||
      (Array.isArray(value['occlusions']) &&
        value['occlusions'].length <= 64 &&
        value['occlusions'].every((item: unknown) => isCanvasSurfaceOcclusion(item)))) &&
    (!Object.hasOwn(value, 'modal') || typeof value['modal'] === 'boolean')
  );
}
