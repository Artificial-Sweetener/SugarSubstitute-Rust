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
import { isCanvasSurfaceViewportState } from './CanvasSurfaceViewportValidation.generated.js';
import { isRecord } from './ContractRecord.js';
/** Validate a bounded CanvasSurfacePreviewIdentity payload before dispatch. */
export function isCanvasSurfacePreviewIdentity(
  value: unknown,
): value is Contract.CanvasSurfacePreviewIdentity {
  return (
    isRecord(value) &&
    Object.keys(value).length === 6 &&
    Object.hasOwn(value, 'generation') &&
    typeof value['generation'] === 'number' &&
    Number.isSafeInteger(value['generation']) &&
    value['generation'] >= 0 &&
    value['generation'] <= 9007199254740991 &&
    Object.hasOwn(value, 'promptId') &&
    typeof value['promptId'] === 'string' &&
    value['promptId'].length <= 512 &&
    Array.from(value['promptId']).length <= 256 &&
    new RegExp('^(?:[^\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])*$').test(
      value['promptId'],
    ) &&
    Object.hasOwn(value, 'sourceId') &&
    typeof value['sourceId'] === 'string' &&
    value['sourceId'].length <= 8192 &&
    Array.from(value['sourceId']).length <= 4096 &&
    new RegExp('^(?:[^\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])*$').test(
      value['sourceId'],
    ) &&
    Object.hasOwn(value, 'runOrder') &&
    typeof value['runOrder'] === 'number' &&
    Number.isSafeInteger(value['runOrder']) &&
    value['runOrder'] >= 1 &&
    value['runOrder'] <= 9007199254740991 &&
    Object.hasOwn(value, 'sequence') &&
    typeof value['sequence'] === 'number' &&
    Number.isSafeInteger(value['sequence']) &&
    value['sequence'] >= 1 &&
    value['sequence'] <= 9007199254740991 &&
    Object.hasOwn(value, 'token') &&
    typeof value['token'] === 'string' &&
    value['token'].length <= 48 &&
    Array.from(value['token']).length <= 24 &&
    new RegExp('^(?:[^\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])*$').test(value['token'])
  );
}
/** Validate a bounded CanvasSurfacePreviewBeginRequest payload before dispatch. */
export function isCanvasSurfacePreviewBeginRequest(
  value: unknown,
): value is Contract.CanvasSurfacePreviewBeginRequest {
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
    value['type'] === 'surface.preview.begin' &&
    Object.hasOwn(value, 'address') &&
    isCanvasSurfaceAddress(value['address']) &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'number' &&
    Number.isSafeInteger(value['revision']) &&
    value['revision'] >= 1 &&
    value['revision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'preview') &&
    isCanvasSurfacePreviewIdentity(value['preview']) &&
    Object.hasOwn(value, 'byteLength') &&
    typeof value['byteLength'] === 'number' &&
    Number.isSafeInteger(value['byteLength']) &&
    value['byteLength'] >= 1 &&
    value['byteLength'] <= 67108864 &&
    Object.hasOwn(value, 'format') &&
    (value['format'] === 'png' || value['format'] === 'jpeg')
  );
}
/** Validate a bounded CanvasSurfacePreviewChunkRequest payload before dispatch. */
export function isCanvasSurfacePreviewChunkRequest(
  value: unknown,
): value is Contract.CanvasSurfacePreviewChunkRequest {
  return (
    isRecord(value) &&
    Object.keys(value).length === 11 &&
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
    value['type'] === 'surface.preview.chunk' &&
    Object.hasOwn(value, 'address') &&
    isCanvasSurfaceAddress(value['address']) &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'number' &&
    Number.isSafeInteger(value['revision']) &&
    value['revision'] >= 1 &&
    value['revision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'token') &&
    typeof value['token'] === 'string' &&
    value['token'].length <= 48 &&
    Array.from(value['token']).length <= 24 &&
    new RegExp('^(?:[^\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])*$').test(
      value['token'],
    ) &&
    Object.hasOwn(value, 'leaseRevision') &&
    typeof value['leaseRevision'] === 'number' &&
    Number.isSafeInteger(value['leaseRevision']) &&
    value['leaseRevision'] >= 1 &&
    value['leaseRevision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'offset') &&
    typeof value['offset'] === 'number' &&
    Number.isSafeInteger(value['offset']) &&
    value['offset'] >= 0 &&
    value['offset'] <= 67108863 &&
    Object.hasOwn(value, 'data') &&
    typeof value['data'] === 'string' &&
    value['data'].length <= 10924 &&
    new RegExp('^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?(?![\\s\\S])').test(
      value['data'],
    )
  );
}
/** Validate a bounded CanvasSurfacePreviewEndRequest payload before dispatch. */
export function isCanvasSurfacePreviewEndRequest(
  value: unknown,
): value is Contract.CanvasSurfacePreviewEndRequest {
  return (
    isRecord(value) &&
    Object.keys(value).length === 9 &&
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
    value['type'] === 'surface.preview.end' &&
    Object.hasOwn(value, 'address') &&
    isCanvasSurfaceAddress(value['address']) &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'number' &&
    Number.isSafeInteger(value['revision']) &&
    value['revision'] >= 1 &&
    value['revision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'token') &&
    typeof value['token'] === 'string' &&
    value['token'].length <= 48 &&
    Array.from(value['token']).length <= 24 &&
    new RegExp('^(?:[^\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])*$').test(
      value['token'],
    ) &&
    Object.hasOwn(value, 'leaseRevision') &&
    typeof value['leaseRevision'] === 'number' &&
    Number.isSafeInteger(value['leaseRevision']) &&
    value['leaseRevision'] >= 1 &&
    value['leaseRevision'] <= 9007199254740991
  );
}
/** Validate a bounded CanvasSurfacePreviewWithdrawRequest payload before dispatch. */
export function isCanvasSurfacePreviewWithdrawRequest(
  value: unknown,
): value is Contract.CanvasSurfacePreviewWithdrawRequest {
  return (
    isRecord(value) &&
    Object.keys(value).length === 9 &&
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
    value['type'] === 'surface.preview.withdraw' &&
    Object.hasOwn(value, 'address') &&
    isCanvasSurfaceAddress(value['address']) &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'number' &&
    Number.isSafeInteger(value['revision']) &&
    value['revision'] >= 1 &&
    value['revision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'token') &&
    typeof value['token'] === 'string' &&
    value['token'].length <= 48 &&
    Array.from(value['token']).length <= 24 &&
    new RegExp('^(?:[^\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])*$').test(
      value['token'],
    ) &&
    Object.hasOwn(value, 'leaseRevision') &&
    typeof value['leaseRevision'] === 'number' &&
    Number.isSafeInteger(value['leaseRevision']) &&
    value['leaseRevision'] >= 1 &&
    value['leaseRevision'] <= 9007199254740991
  );
}
/** Validate a bounded CanvasSurfacePresentationRequest payload before dispatch. */
export function isCanvasSurfacePresentationRequest(
  value: unknown,
): value is Contract.CanvasSurfacePresentationRequest {
  return (
    isRecord(value) &&
    Object.keys(value).length === 7 &&
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
    value['type'] === 'surface.presentation.request' &&
    Object.hasOwn(value, 'schema') &&
    value['schema'] === 1 &&
    Object.hasOwn(value, 'enabled') &&
    typeof value['enabled'] === 'boolean'
  );
}
/** Validate a bounded CanvasSurfacePresentationReply payload before dispatch. */
export function isCanvasSurfacePresentationReply(
  value: unknown,
): value is Contract.CanvasSurfacePresentationReply {
  return (
    isRecord(value) &&
    Object.keys(value).length === 9 &&
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
    value['type'] === 'surface.presentation.reply' &&
    Object.hasOwn(value, 'schema') &&
    value['schema'] === 1 &&
    Object.hasOwn(value, 'enabled') &&
    typeof value['enabled'] === 'boolean' &&
    Object.hasOwn(value, 'status') &&
    (value['status'] === 'available' ||
      value['status'] === 'unsupported' ||
      value['status'] === 'unavailable' ||
      value['status'] === 'error') &&
    Object.hasOwn(value, 'message') &&
    typeof value['message'] === 'string' &&
    value['message'].length <= 1024 &&
    Array.from(value['message']).length <= 512 &&
    new RegExp('^(?:[^\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])*$').test(
      value['message'],
    )
  );
}
/** Validate a bounded CanvasSurfacePresentationDisplay payload before dispatch. */
export function isCanvasSurfacePresentationDisplay(
  value: unknown,
): value is Contract.CanvasSurfacePresentationDisplay {
  return (
    (isRecord(value) &&
      Object.keys(value).length === 1 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'empty') ||
    (isRecord(value) &&
      Object.keys(value).length === 4 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'preview' &&
      Object.hasOwn(value, 'token') &&
      typeof value['token'] === 'string' &&
      value['token'].length <= 48 &&
      Array.from(value['token']).length <= 24 &&
      new RegExp('^(?:[^\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])*$').test(
        value['token'],
      ) &&
      Object.hasOwn(value, 'leaseRevision') &&
      typeof value['leaseRevision'] === 'number' &&
      Number.isSafeInteger(value['leaseRevision']) &&
      value['leaseRevision'] >= 1 &&
      value['leaseRevision'] <= 9007199254740991 &&
      Object.hasOwn(value, 'viewport') &&
      isCanvasSurfaceViewportState(value['viewport'])) ||
    (isRecord(value) &&
      Object.keys(value).length === 3 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'final' &&
      Object.hasOwn(value, 'selectionRevision') &&
      typeof value['selectionRevision'] === 'number' &&
      Number.isSafeInteger(value['selectionRevision']) &&
      value['selectionRevision'] >= 1 &&
      value['selectionRevision'] <= 9007199254740991 &&
      Object.hasOwn(value, 'viewport') &&
      isCanvasSurfaceViewportState(value['viewport']))
  );
}
/** Validate a bounded CanvasSurfacePresentationChange payload before dispatch. */
export function isCanvasSurfacePresentationChange(
  value: unknown,
): value is Contract.CanvasSurfacePresentationChange {
  return (
    isRecord(value) &&
    Object.keys(value).length === 7 &&
    Object.hasOwn(value, 'address') &&
    isCanvasSurfaceAddress(value['address']) &&
    Object.hasOwn(value, 'sequence') &&
    typeof value['sequence'] === 'number' &&
    Number.isSafeInteger(value['sequence']) &&
    value['sequence'] >= 1 &&
    value['sequence'] <= 9007199254740991 &&
    Object.hasOwn(value, 'token') &&
    typeof value['token'] === 'string' &&
    value['token'].length <= 48 &&
    Array.from(value['token']).length <= 24 &&
    new RegExp('^(?:[^\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])*$').test(
      value['token'],
    ) &&
    Object.hasOwn(value, 'leaseRevision') &&
    typeof value['leaseRevision'] === 'number' &&
    Number.isSafeInteger(value['leaseRevision']) &&
    value['leaseRevision'] >= 1 &&
    value['leaseRevision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'status') &&
    (value['status'] === 'presented' ||
      value['status'] === 'failed' ||
      value['status'] === 'cancelled') &&
    Object.hasOwn(value, 'displayed') &&
    isCanvasSurfacePresentationDisplay(value['displayed']) &&
    Object.hasOwn(value, 'message') &&
    typeof value['message'] === 'string' &&
    value['message'].length <= 1024 &&
    Array.from(value['message']).length <= 512 &&
    new RegExp('^(?:[^\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])*$').test(
      value['message'],
    )
  );
}
/** Validate a bounded CanvasSurfacePresentationChanged payload before dispatch. */
export function isCanvasSurfacePresentationChanged(
  value: unknown,
): value is Contract.CanvasSurfacePresentationChanged {
  return (
    isRecord(value) &&
    Object.keys(value).length === 7 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.canvas' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 3 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'requestId') &&
    value['requestId'] === 0 &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'surface.presentation.changed' &&
    Object.hasOwn(value, 'schema') &&
    value['schema'] === 1 &&
    Object.hasOwn(value, 'change') &&
    isCanvasSurfacePresentationChange(value['change'])
  );
}
