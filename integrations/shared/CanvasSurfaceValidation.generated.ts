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
  isCanvasSurfaceAddress,
  isCanvasSurfaceGeometry,
  isCanvasSurfaceFileReference,
} from './CanvasSurfaceGeometryValidation.generated.js';
import {
  isCanvasSurfacePreviewBeginRequest,
  isCanvasSurfacePreviewChunkRequest,
  isCanvasSurfacePreviewEndRequest,
  isCanvasSurfacePreviewWithdrawRequest,
  isCanvasSurfacePresentationRequest,
  isCanvasSurfacePresentationReply,
  isCanvasSurfacePresentationChanged,
} from './CanvasSurfacePreviewValidation.generated.js';
import {
  isCanvasSurfaceViewportRequest,
  isCanvasSurfaceViewportReply,
} from './CanvasSurfaceViewportValidation.generated.js';
import { isRecord } from './ContractRecord.js';
/** Validate a bounded CanvasSurfaceCapabilities payload before dispatch. */
export function isCanvasSurfaceCapabilities(
  value: unknown,
): value is Contract.CanvasSurfaceCapabilities {
  return (
    isRecord(value) &&
    Object.keys(value).length === 7 &&
    Object.hasOwn(value, 'schema') &&
    value['schema'] === 1 &&
    Object.hasOwn(value, 'imageFormats') &&
    Array.isArray(value['imageFormats']) &&
    value['imageFormats'].length <= 3 &&
    value['imageFormats'].every(
      (item: unknown) => item === 'png' || item === 'jpeg' || item === 'webp',
    ) &&
    Object.hasOwn(value, 'viewportCommands') &&
    Array.isArray(value['viewportCommands']) &&
    value['viewportCommands'].length <= 4 &&
    value['viewportCommands'].every(
      (item: unknown) => item === 'fit' || item === 'native' || item === 'zoom' || item === 'pan',
    ) &&
    Object.hasOwn(value, 'viewportState') &&
    typeof value['viewportState'] === 'boolean' &&
    Object.hasOwn(value, 'previewFormats') &&
    Array.isArray(value['previewFormats']) &&
    value['previewFormats'].length <= 2 &&
    value['previewFormats'].every((item: unknown) => item === 'png' || item === 'jpeg') &&
    Object.hasOwn(value, 'comparison') &&
    typeof value['comparison'] === 'boolean' &&
    Object.hasOwn(value, 'overview') &&
    typeof value['overview'] === 'boolean'
  );
}
/** Validate a bounded CanvasSurfaceCapabilitiesRequest payload before dispatch. */
export function isCanvasSurfaceCapabilitiesRequest(
  value: unknown,
): value is Contract.CanvasSurfaceCapabilitiesRequest {
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
    value['type'] === 'surface.capabilities.request' &&
    Object.hasOwn(value, 'schema') &&
    value['schema'] === 1
  );
}
/** Validate a bounded CanvasSurfaceCapabilitiesReply payload before dispatch. */
export function isCanvasSurfaceCapabilitiesReply(
  value: unknown,
): value is Contract.CanvasSurfaceCapabilitiesReply {
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
    value['type'] === 'surface.capabilities.reply' &&
    Object.hasOwn(value, 'status') &&
    (value['status'] === 'available' ||
      value['status'] === 'unsupported' ||
      value['status'] === 'unavailable' ||
      value['status'] === 'error') &&
    Object.hasOwn(value, 'capabilities') &&
    isCanvasSurfaceCapabilities(value['capabilities']) &&
    Object.hasOwn(value, 'message') &&
    typeof value['message'] === 'string' &&
    value['message'].length <= 1024 &&
    Array.from(value['message']).length <= 512 &&
    new RegExp('^(?:[^\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])*$').test(
      value['message'],
    )
  );
}
/** Validate a bounded CanvasSurfacePrepareRequest payload before dispatch. */
export function isCanvasSurfacePrepareRequest(
  value: unknown,
): value is Contract.CanvasSurfacePrepareRequest {
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
    value['type'] === 'surface.prepare' &&
    Object.hasOwn(value, 'address') &&
    isCanvasSurfaceAddress(value['address']) &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'number' &&
    Number.isSafeInteger(value['revision']) &&
    value['revision'] >= 1 &&
    value['revision'] <= 9007199254740991
  );
}
/** Validate a bounded CanvasSurfaceGeometryRequest payload before dispatch. */
export function isCanvasSurfaceGeometryRequest(
  value: unknown,
): value is Contract.CanvasSurfaceGeometryRequest {
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
    value['type'] === 'surface.geometry' &&
    Object.hasOwn(value, 'address') &&
    isCanvasSurfaceAddress(value['address']) &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'number' &&
    Number.isSafeInteger(value['revision']) &&
    value['revision'] >= 1 &&
    value['revision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'geometry') &&
    isCanvasSurfaceGeometry(value['geometry'])
  );
}
/** Validate a bounded CanvasSurfaceSelectRequest payload before dispatch. */
export function isCanvasSurfaceSelectRequest(
  value: unknown,
): value is Contract.CanvasSurfaceSelectRequest {
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
    value['type'] === 'surface.select' &&
    Object.hasOwn(value, 'address') &&
    isCanvasSurfaceAddress(value['address']) &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'number' &&
    Number.isSafeInteger(value['revision']) &&
    value['revision'] >= 1 &&
    value['revision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'itemId') &&
    typeof value['itemId'] === 'string' &&
    value['itemId'].length <= 8192 &&
    Array.from(value['itemId']).length <= 4096 &&
    new RegExp(
      '^result:(?:[^\\u0000-\\u001F\\u007F-\\u009F\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])+(?![\\s\\S])',
    ).test(value['itemId']) &&
    Object.hasOwn(value, 'file') &&
    isCanvasSurfaceFileReference(value['file'])
  );
}
/** Validate a bounded CanvasSurfaceCommitRequest payload before dispatch. */
export function isCanvasSurfaceCommitRequest(
  value: unknown,
): value is Contract.CanvasSurfaceCommitRequest {
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
    value['type'] === 'surface.commit' &&
    Object.hasOwn(value, 'address') &&
    isCanvasSurfaceAddress(value['address']) &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'number' &&
    Number.isSafeInteger(value['revision']) &&
    value['revision'] >= 1 &&
    value['revision'] <= 9007199254740991
  );
}
/** Validate a bounded CanvasSurfaceReleaseRequest payload before dispatch. */
export function isCanvasSurfaceReleaseRequest(
  value: unknown,
): value is Contract.CanvasSurfaceReleaseRequest {
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
    value['type'] === 'surface.release' &&
    Object.hasOwn(value, 'address') &&
    isCanvasSurfaceAddress(value['address']) &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'number' &&
    Number.isSafeInteger(value['revision']) &&
    value['revision'] >= 1 &&
    value['revision'] <= 9007199254740991
  );
}
/** Validate a bounded CanvasSurfaceCancelRequest payload before dispatch. */
export function isCanvasSurfaceCancelRequest(
  value: unknown,
): value is Contract.CanvasSurfaceCancelRequest {
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
/** Validate a bounded CanvasSurfaceReply payload before dispatch. */
export function isCanvasSurfaceReply(value: unknown): value is Contract.CanvasSurfaceReply {
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
    value['type'] === 'surface.reply' &&
    Object.hasOwn(value, 'address') &&
    isCanvasSurfaceAddress(value['address']) &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'number' &&
    Number.isSafeInteger(value['revision']) &&
    value['revision'] >= 1 &&
    value['revision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'status') &&
    (value['status'] === 'ready' ||
      value['status'] === 'applied' ||
      value['status'] === 'stale' ||
      value['status'] === 'cancelled' ||
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
/** Validate a bounded CanvasSurfaceMessage payload before dispatch. */
export function isCanvasSurfaceMessage(value: unknown): value is Contract.CanvasSurfaceMessage {
  return (
    isCanvasSurfacePreviewBeginRequest(value) ||
    isCanvasSurfacePreviewChunkRequest(value) ||
    isCanvasSurfacePreviewEndRequest(value) ||
    isCanvasSurfacePreviewWithdrawRequest(value) ||
    isCanvasSurfacePresentationRequest(value) ||
    isCanvasSurfacePresentationReply(value) ||
    isCanvasSurfacePresentationChanged(value) ||
    isCanvasSurfaceCapabilitiesRequest(value) ||
    isCanvasSurfaceCapabilitiesReply(value) ||
    isCanvasSurfacePrepareRequest(value) ||
    isCanvasSurfaceGeometryRequest(value) ||
    isCanvasSurfaceSelectRequest(value) ||
    isCanvasSurfaceCommitRequest(value) ||
    isCanvasSurfaceReleaseRequest(value) ||
    isCanvasSurfaceReply(value) ||
    isCanvasSurfaceViewportRequest(value) ||
    isCanvasSurfaceViewportReply(value)
  );
}
export {
  isCanvasSurfacePreviewIdentity,
  isCanvasSurfacePreviewBeginRequest,
  isCanvasSurfacePreviewChunkRequest,
  isCanvasSurfacePreviewEndRequest,
  isCanvasSurfacePreviewWithdrawRequest,
  isCanvasSurfacePresentationRequest,
  isCanvasSurfacePresentationReply,
  isCanvasSurfacePresentationDisplay,
  isCanvasSurfacePresentationChange,
  isCanvasSurfacePresentationChanged,
} from './CanvasSurfacePreviewValidation.generated.js';
export {
  isCanvasSurfaceAddress,
  isCanvasSurfaceFileReference,
  isCanvasSurfacePoint,
  isCanvasSurfaceSize,
  isCanvasSurfaceClip,
  isCanvasSurfaceGeometry,
} from './CanvasSurfaceGeometryValidation.generated.js';
export {
  isCanvasSurfaceViewportCommand,
  isCanvasSurfaceImageDimensions,
  isCanvasSurfaceViewportState,
  isCanvasSurfaceViewportRequest,
  isCanvasSurfaceViewportReply,
} from './CanvasSurfaceViewportValidation.generated.js';
