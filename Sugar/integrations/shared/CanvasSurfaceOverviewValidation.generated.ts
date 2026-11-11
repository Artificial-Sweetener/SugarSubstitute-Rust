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
import {
  isCanvasSurfaceFileReference,
  isCanvasSurfaceAddress,
} from './CanvasSurfaceGeometryValidation.generated.js';
import { isRecord } from './ContractRecord.js';
/** Validate a bounded CanvasSurfaceOverviewRect payload before dispatch. */
export function isCanvasSurfaceOverviewRect(
  value: unknown,
): value is Contract.CanvasSurfaceOverviewRect {
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
/** Validate a bounded CanvasSurfaceOverviewCell payload before dispatch. */
export function isCanvasSurfaceOverviewCell(
  value: unknown,
): value is Contract.CanvasSurfaceOverviewCell {
  return (
    isRecord(value) &&
    Object.keys(value).length === 3 &&
    Object.hasOwn(value, 'itemId') &&
    typeof value['itemId'] === 'string' &&
    value['itemId'].length <= 2048 &&
    Array.from(value['itemId']).length <= 1024 &&
    new RegExp(
      '^result:(?:[^\\u0000-\\u001F\\u007F-\\u009F\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])+(?![\\s\\S])',
    ).test(value['itemId']) &&
    Object.hasOwn(value, 'file') &&
    isCanvasSurfaceFileReference(value['file']) &&
    Object.hasOwn(value, 'rect') &&
    isCanvasSurfaceOverviewRect(value['rect'])
  );
}
/** Validate a bounded CanvasSurfaceOverviewRequest payload before dispatch. */
export function isCanvasSurfaceOverviewRequest(
  value: unknown,
): value is Contract.CanvasSurfaceOverviewRequest {
  return (
    isRecord(value) &&
    Object.keys(value).length === 8 &&
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
    value['type'] === 'surface.overview' &&
    Object.hasOwn(value, 'address') &&
    isCanvasSurfaceAddress(value['address']) &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'number' &&
    Number.isSafeInteger(value['revision']) &&
    value['revision'] >= 1 &&
    value['revision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'cells') &&
    Array.isArray(value['cells']) &&
    value['cells'].length >= 1 &&
    value['cells'].length <= 8 &&
    value['cells'].every((item: unknown) => isCanvasSurfaceOverviewCell(item))
  );
}
