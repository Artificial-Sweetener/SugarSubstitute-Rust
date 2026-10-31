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
import { isCanvasSurfaceAddress } from './CanvasSurfaceGeometryValidation.generated.js';
import { isRecord } from './ContractRecord.js';
/** Validate a bounded CanvasSurfaceViewportCommand payload before dispatch. */
export function isCanvasSurfaceViewportCommand(
  value: unknown,
): value is Contract.CanvasSurfaceViewportCommand {
  return (
    (isRecord(value) &&
      Object.keys(value).length === 1 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'read') ||
    (isRecord(value) &&
      Object.keys(value).length === 1 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'fit') ||
    (isRecord(value) &&
      Object.keys(value).length === 1 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'native') ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'zoom' &&
      Object.hasOwn(value, 'factor') &&
      typeof value['factor'] === 'number' &&
      Number.isFinite(value['factor']) &&
      value['factor'] >= 0.01 &&
      value['factor'] <= 100) ||
    (isRecord(value) &&
      Object.keys(value).length === 4 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'pan' &&
      Object.hasOwn(value, 'x') &&
      typeof value['x'] === 'number' &&
      Number.isFinite(value['x']) &&
      value['x'] >= -10 &&
      value['x'] <= 10 &&
      Object.hasOwn(value, 'y') &&
      typeof value['y'] === 'number' &&
      Number.isFinite(value['y']) &&
      value['y'] >= -10 &&
      value['y'] <= 10 &&
      Object.hasOwn(value, 'units') &&
      value['units'] === 'viewport') ||
    (isRecord(value) &&
      Object.keys(value).length === 3 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'wipe' &&
      Object.hasOwn(value, 'orientation') &&
      (value['orientation'] === 'vertical' || value['orientation'] === 'horizontal') &&
      Object.hasOwn(value, 'split') &&
      typeof value['split'] === 'number' &&
      Number.isFinite(value['split']) &&
      value['split'] >= 0 &&
      value['split'] <= 1) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'inspect' &&
      Object.hasOwn(value, 'inspection') &&
      isCanvasSurfaceInspection(value['inspection']))
  );
}
/** Validate a bounded CanvasSurfaceImageDimensions payload before dispatch. */
export function isCanvasSurfaceImageDimensions(
  value: unknown,
): value is Contract.CanvasSurfaceImageDimensions {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
    Object.hasOwn(value, 'width') &&
    typeof value['width'] === 'number' &&
    Number.isSafeInteger(value['width']) &&
    value['width'] >= 1 &&
    value['width'] <= 2147483647 &&
    Object.hasOwn(value, 'height') &&
    typeof value['height'] === 'number' &&
    Number.isSafeInteger(value['height']) &&
    value['height'] >= 1 &&
    value['height'] <= 2147483647
  );
}
/** Validate a bounded CanvasSurfaceViewportCenter payload before dispatch. */
export function isCanvasSurfaceViewportCenter(
  value: unknown,
): value is Contract.CanvasSurfaceViewportCenter {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
    Object.hasOwn(value, 'x') &&
    typeof value['x'] === 'number' &&
    Number.isFinite(value['x']) &&
    value['x'] >= 0 &&
    value['x'] <= 1 &&
    Object.hasOwn(value, 'y') &&
    typeof value['y'] === 'number' &&
    Number.isFinite(value['y']) &&
    value['y'] >= 0 &&
    value['y'] <= 1
  );
}
/** Validate a bounded CanvasSurfaceInspectionSpan payload before dispatch. */
export function isCanvasSurfaceInspectionSpan(
  value: unknown,
): value is Contract.CanvasSurfaceInspectionSpan {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
    Object.hasOwn(value, 'x') &&
    typeof value['x'] === 'number' &&
    Number.isFinite(value['x']) &&
    value['x'] > 0 &&
    value['x'] <= 1000000000 &&
    Object.hasOwn(value, 'y') &&
    typeof value['y'] === 'number' &&
    Number.isFinite(value['y']) &&
    value['y'] > 0 &&
    value['y'] <= 1000000000
  );
}
/** Validate a bounded CanvasSurfaceInspection payload before dispatch. */
export function isCanvasSurfaceInspection(
  value: unknown,
): value is Contract.CanvasSurfaceInspection {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
    Object.hasOwn(value, 'center') &&
    isCanvasSurfaceViewportCenter(value['center']) &&
    Object.hasOwn(value, 'span') &&
    isCanvasSurfaceInspectionSpan(value['span'])
  );
}
/** Validate a bounded CanvasSurfaceViewportState payload before dispatch. */
export function isCanvasSurfaceViewportState(
  value: unknown,
): value is Contract.CanvasSurfaceViewportState {
  return (
    isRecord(value) &&
    Object.keys(value).length ===
      3 +
        (Object.hasOwn(value, 'secondaryDimensions') ? 1 : 0) +
        (Object.hasOwn(value, 'center') ? 1 : 0) &&
    Object.hasOwn(value, 'mode') &&
    (value['mode'] === 'fit' || value['mode'] === 'native' || value['mode'] === 'custom') &&
    Object.hasOwn(value, 'physicalScale') &&
    typeof value['physicalScale'] === 'number' &&
    Number.isFinite(value['physicalScale']) &&
    value['physicalScale'] >= 1e-9 &&
    value['physicalScale'] <= 16384 &&
    Object.hasOwn(value, 'dimensions') &&
    isCanvasSurfaceImageDimensions(value['dimensions']) &&
    (!Object.hasOwn(value, 'secondaryDimensions') ||
      isCanvasSurfaceImageDimensions(value['secondaryDimensions'])) &&
    (!Object.hasOwn(value, 'center') || isCanvasSurfaceViewportCenter(value['center']))
  );
}
/** Validate a bounded CanvasSurfaceViewportRequest payload before dispatch. */
export function isCanvasSurfaceViewportRequest(
  value: unknown,
): value is Contract.CanvasSurfaceViewportRequest {
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
    value['type'] === 'surface.viewport.request' &&
    Object.hasOwn(value, 'address') &&
    isCanvasSurfaceAddress(value['address']) &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'number' &&
    Number.isSafeInteger(value['revision']) &&
    value['revision'] >= 1 &&
    value['revision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'command') &&
    isCanvasSurfaceViewportCommand(value['command'])
  );
}
/** Validate a bounded CanvasSurfaceViewportReply payload before dispatch. */
export function isCanvasSurfaceViewportReply(
  value: unknown,
): value is Contract.CanvasSurfaceViewportReply {
  return (
    isRecord(value) &&
    Object.keys(value).length === 10 &&
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
    value['type'] === 'surface.viewport.reply' &&
    Object.hasOwn(value, 'address') &&
    isCanvasSurfaceAddress(value['address']) &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'number' &&
    Number.isSafeInteger(value['revision']) &&
    value['revision'] >= 1 &&
    value['revision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'status') &&
    value['status'] === 'applied' &&
    Object.hasOwn(value, 'message') &&
    typeof value['message'] === 'string' &&
    value['message'].length <= 1024 &&
    Array.from(value['message']).length <= 512 &&
    new RegExp('^(?:[^\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])*$').test(
      value['message'],
    ) &&
    Object.hasOwn(value, 'viewport') &&
    isCanvasSurfaceViewportState(value['viewport'])
  );
}
